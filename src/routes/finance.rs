//! Finance: per-user transactions, budgets, debts + repayments, and derived overview/insights.
use crate::{auth::CurrentUser, error::AppError, AppState};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

type R = Result<Json<Value>, AppError>;
type RC = Result<(StatusCode, Json<Value>), AppError>;

fn norm_ccy(c: Option<String>) -> Result<String, AppError> {
    let c = c.unwrap_or_else(|| "NGN".into()).trim().to_uppercase();
    if c.chars().count() != 3 || !c.chars().all(|x| x.is_ascii_alphabetic()) {
        return Err(AppError::BadRequest("currency must be a 3-letter code like NGN".into()));
    }
    Ok(c)
}
fn clean_cat(c: Option<String>) -> String {
    c.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).unwrap_or_else(|| "general".into())
}

pub async fn overview(u: CurrentUser, State(s): State<AppState>) -> R {
    let v: Value = sqlx::query_scalar(
        "WITH ngn AS (
            SELECT
              coalesce(sum(amount) FILTER (WHERE kind='income'),0)  AS inc,
              coalesce(sum(amount) FILTER (WHERE kind='expense'),0) AS exp,
              coalesce(sum(amount) FILTER (WHERE kind='income'  AND date_trunc('month',occurred_on)=date_trunc('month',current_date)),0) AS minc,
              coalesce(sum(amount) FILTER (WHERE kind='expense' AND date_trunc('month',occurred_on)=date_trunc('month',current_date)),0) AS mexp
            FROM finance_transactions WHERE currency='NGN' AND user_id=$1)
         SELECT jsonb_build_object(
           'primary_currency','NGN',
           'balance',          (SELECT inc-exp FROM ngn),
           'income_total',     (SELECT inc  FROM ngn),
           'expense_total',    (SELECT exp  FROM ngn),
           'month_income',     (SELECT minc FROM ngn),
           'month_expense',    (SELECT mexp FROM ngn),
           'month_savings',    (SELECT minc-mexp FROM ngn),
           'debt_outstanding', coalesce((SELECT sum(outstanding) FROM finance_debts WHERE status='open' AND currency='NGN' AND user_id=$1),0),
           'other_currencies', coalesce((
               SELECT jsonb_agg(jsonb_build_object('currency',currency,'balance',bal,'income_total',inc,'expense_total',exp) ORDER BY currency)
               FROM (SELECT currency,
                       coalesce(sum(amount) FILTER (WHERE kind='income'),0) - coalesce(sum(amount) FILTER (WHERE kind='expense'),0) AS bal,
                       coalesce(sum(amount) FILTER (WHERE kind='income'),0)  AS inc,
                       coalesce(sum(amount) FILTER (WHERE kind='expense'),0) AS exp
                     FROM finance_transactions WHERE currency<>'NGN' AND user_id=$1 GROUP BY currency) q), '[]'::jsonb)
         )",
    ).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct TxQuery { kind: Option<String>, category: Option<String>, from: Option<NaiveDate>, to: Option<NaiveDate> }

pub async fn tx_list(u: CurrentUser, State(s): State<AppState>, Query(q): Query<TxQuery>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.occurred_on DESC, x.created_at DESC), '[]'::jsonb) FROM
         (SELECT * FROM finance_transactions
          WHERE user_id=$5 AND ($1::text IS NULL OR kind=$1) AND ($2::text IS NULL OR category=$2)
            AND ($3::date IS NULL OR occurred_on>=$3) AND ($4::date IS NULL OR occurred_on<=$4)
          ORDER BY occurred_on DESC, created_at DESC LIMIT 500) x",
    ).bind(q.kind).bind(q.category).bind(q.from).bind(q.to).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct TxIn { kind: String, amount: f64, currency: Option<String>, category: Option<String>, description: Option<String>, occurred_on: Option<NaiveDate> }

pub async fn tx_create(u: CurrentUser, State(s): State<AppState>, Json(b): Json<TxIn>) -> RC {
    if b.kind != "income" && b.kind != "expense" { return Err(AppError::BadRequest("kind must be income or expense".into())); }
    if !(b.amount > 0.0) { return Err(AppError::BadRequest("amount must be greater than 0".into())); }
    let ccy = norm_ccy(b.currency)?;
    let cat = clean_cat(b.category);
    let v: Value = sqlx::query_scalar(
        "INSERT INTO finance_transactions (user_id, kind, amount, currency, category, description, occurred_on)
         VALUES ($7, $1, ROUND($2::numeric,2), $3, $4, $5, COALESCE($6, current_date)) RETURNING to_jsonb(finance_transactions)",
    ).bind(&b.kind).bind(b.amount).bind(ccy).bind(cat).bind(b.description).bind(b.occurred_on).bind(u.id).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct TxPatch { kind: Option<String>, amount: Option<f64>, currency: Option<String>, category: Option<String>, description: Option<String>, occurred_on: Option<NaiveDate> }

pub async fn tx_update(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<TxPatch>) -> R {
    if let Some(k) = &b.kind { if k != "income" && k != "expense" { return Err(AppError::BadRequest("kind must be income or expense".into())); } }
    if let Some(a) = b.amount { if !(a > 0.0) { return Err(AppError::BadRequest("amount must be greater than 0".into())); } }
    let ccy = match b.currency { Some(c) => Some(norm_ccy(Some(c))?), None => None };
    let cat = b.category.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    let v: Value = sqlx::query_scalar(
        "UPDATE finance_transactions SET
            kind=COALESCE($2,kind), amount=COALESCE(ROUND($3::numeric,2),amount), currency=COALESCE($4,currency),
            category=COALESCE($5,category), description=COALESCE($6,description), occurred_on=COALESCE($7,occurred_on), updated_at=now()
         WHERE id=$1 AND user_id=$8 RETURNING to_jsonb(finance_transactions)",
    ).bind(id).bind(b.kind).bind(b.amount).bind(ccy).bind(cat).bind(b.description).bind(b.occurred_on).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

pub async fn tx_delete(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM finance_transactions WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct BudgetQ { month: Option<NaiveDate> }

pub async fn budgets_list(u: CurrentUser, State(s): State<AppState>, Query(q): Query<BudgetQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(jsonb_build_object(
             'id', b.id, 'month', b.month, 'category', b.category, 'limit_amount', b.limit_amount, 'currency', b.currency,
             'spent', coalesce((SELECT sum(amount) FROM finance_transactions t
                                WHERE t.user_id=b.user_id AND t.kind='expense' AND t.category=b.category AND t.currency=b.currency
                                  AND date_trunc('month',t.occurred_on)=b.month),0)
           ) ORDER BY b.category), '[]'::jsonb)
         FROM finance_budgets b WHERE b.user_id=$2 AND b.month = date_trunc('month', COALESCE($1, current_date))::date",
    ).bind(q.month).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct BudgetIn { month: Option<NaiveDate>, category: String, limit_amount: f64, currency: Option<String> }

pub async fn budgets_upsert(u: CurrentUser, State(s): State<AppState>, Json(b): Json<BudgetIn>) -> RC {
    let cat = b.category.trim().to_string();
    if cat.is_empty() { return Err(AppError::BadRequest("category is required".into())); }
    if b.limit_amount < 0.0 { return Err(AppError::BadRequest("limit cannot be negative".into())); }
    let ccy = norm_ccy(b.currency)?;
    let v: Value = sqlx::query_scalar(
        "INSERT INTO finance_budgets (user_id, month, category, limit_amount, currency)
         VALUES ($5, date_trunc('month', COALESCE($1, current_date))::date, $2, ROUND($3::numeric,2), $4)
         ON CONFLICT (user_id, month, category) DO UPDATE SET limit_amount = ROUND($3::numeric,2), currency = $4
         RETURNING to_jsonb(finance_budgets)",
    ).bind(b.month).bind(cat).bind(b.limit_amount).bind(ccy).bind(u.id).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

pub async fn budgets_del(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM finance_budgets WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn debts_list(u: CurrentUser, State(s): State<AppState>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(jsonb_build_object(
             'id', d.id, 'creditor', d.creditor, 'original_amount', d.original_amount, 'outstanding', d.outstanding,
             'interest_rate', d.interest_rate, 'currency', d.currency, 'status', d.status, 'opened_on', d.opened_on, 'notes', d.notes,
             'total_repaid', coalesce((SELECT sum(amount) FROM finance_repayments r WHERE r.debt_id=d.id),0)
           ) ORDER BY (d.status<>'open'), d.opened_on DESC), '[]'::jsonb)
         FROM finance_debts d WHERE d.user_id=$1",
    ).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct DebtIn { creditor: String, original_amount: f64, outstanding: Option<f64>, interest_rate: Option<f64>, currency: Option<String>, opened_on: Option<NaiveDate>, notes: Option<String> }

pub async fn debts_create(u: CurrentUser, State(s): State<AppState>, Json(b): Json<DebtIn>) -> RC {
    let creditor = b.creditor.trim().to_string();
    if creditor.is_empty() { return Err(AppError::BadRequest("creditor is required".into())); }
    if b.original_amount < 0.0 { return Err(AppError::BadRequest("amount cannot be negative".into())); }
    let ccy = norm_ccy(b.currency)?;
    let v: Value = sqlx::query_scalar(
        "INSERT INTO finance_debts (user_id, creditor, original_amount, outstanding, interest_rate, currency, opened_on, notes)
         VALUES ($8, $1, ROUND($2::numeric,2), ROUND(COALESCE($3,$2)::numeric,2), $4, $5, COALESCE($6, current_date), $7)
         RETURNING to_jsonb(finance_debts)",
    ).bind(creditor).bind(b.original_amount).bind(b.outstanding).bind(b.interest_rate).bind(ccy).bind(b.opened_on).bind(b.notes).bind(u.id).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct DebtPatch { creditor: Option<String>, outstanding: Option<f64>, interest_rate: Option<f64>, status: Option<String>, notes: Option<String> }

pub async fn debts_patch(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<DebtPatch>) -> R {
    if let Some(o) = b.outstanding { if o < 0.0 { return Err(AppError::BadRequest("outstanding cannot be negative".into())); } }
    let v: Value = sqlx::query_scalar(
        "UPDATE finance_debts SET creditor=COALESCE($2,creditor), outstanding=COALESCE(ROUND($3::numeric,2),outstanding),
            interest_rate=COALESCE($4,interest_rate), status=COALESCE($5,status), notes=COALESCE($6,notes)
         WHERE id=$1 AND user_id=$7 RETURNING to_jsonb(finance_debts)",
    ).bind(id).bind(b.creditor.map(|c| c.trim().to_string())).bind(b.outstanding).bind(b.interest_rate).bind(b.status).bind(b.notes).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct RepayIn { amount: f64, paid_on: Option<NaiveDate>, note: Option<String> }

pub async fn debt_repay(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<RepayIn>) -> RC {
    if !(b.amount > 0.0) { return Err(AppError::BadRequest("amount must be greater than 0".into())); }
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM finance_debts WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).fetch_optional(&s.db).await?;
    if exists.is_none() { return Err(AppError::NotFound); }
    let v: Value = sqlx::query_scalar(
        "WITH r AS (
            INSERT INTO finance_repayments (user_id, debt_id, amount, paid_on, note)
            VALUES ($5, $1, ROUND($2::numeric,2), COALESCE($3, current_date), $4) RETURNING amount)
         UPDATE finance_debts d
            SET outstanding = GREATEST(0, d.outstanding - (SELECT amount FROM r)),
                status      = CASE WHEN d.outstanding - (SELECT amount FROM r) <= 0 THEN 'paid' ELSE d.status END
          WHERE d.id = $1 AND d.user_id = $5
          RETURNING to_jsonb(d)",
    ).bind(id).bind(b.amount).bind(b.paid_on).bind(b.note).bind(u.id).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct InsightQ { months: Option<i32> }

pub async fn insights(u: CurrentUser, State(s): State<AppState>, Query(q): Query<InsightQ>) -> R {
    let n = q.months.unwrap_or(6).clamp(1, 24);
    let v: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object(
           'primary_currency','NGN',
           'months', (
             SELECT coalesce(jsonb_agg(jsonb_build_object(
                 'month', to_char(m,'YYYY-MM'),
                 'income',  coalesce((SELECT sum(amount) FROM finance_transactions t WHERE t.user_id=$2 AND t.currency='NGN' AND t.kind='income'  AND date_trunc('month',t.occurred_on)=m),0),
                 'expense', coalesce((SELECT sum(amount) FROM finance_transactions t WHERE t.user_id=$2 AND t.currency='NGN' AND t.kind='expense' AND date_trunc('month',t.occurred_on)=m),0)
               ) ORDER BY m), '[]'::jsonb)
             FROM generate_series(date_trunc('month',current_date) - (($1::text)||' months')::interval + interval '1 month', date_trunc('month',current_date), interval '1 month') m),
           'by_category', (
             SELECT coalesce(jsonb_agg(jsonb_build_object('category',category,'amount',amt) ORDER BY amt DESC), '[]'::jsonb)
             FROM (SELECT category, sum(amount) amt FROM finance_transactions
                   WHERE user_id=$2 AND currency='NGN' AND kind='expense' AND date_trunc('month',occurred_on)=date_trunc('month',current_date)
                   GROUP BY category) c)
         )",
    ).bind(n).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}
