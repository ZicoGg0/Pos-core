use crate::{auth::Authed, error::AppError, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Serialize, FromRow)]
pub struct Job {
    id: Uuid,
    source: String,
    title: String,
    company: Option<String>,
    url: String,
    description: Option<String>,
    category: Option<String>,
    job_type: Option<String>,
    location: Option<String>,
    pay_min: Option<i32>,
    pay_max: Option<i32>,
    pay_period: Option<String>,
    pay_text: Option<String>,
    score: Option<i16>,
    score_reason: Option<String>,
    status: String,
    skip_reason: Option<String>,
    notes: Option<String>,
    posted_at: Option<DateTime<Utc>>,
    found_at: DateTime<Utc>,
    applied_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    status: Option<String>,
    min_score: Option<i16>,
}

pub async fn list(
    _: Authed,
    State(s): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<Job>>, AppError> {
    let rows = sqlx::query_as::<_, Job>(
        "SELECT * FROM jobs
         WHERE ($1::text IS NULL OR status = $1)
           AND ($2::smallint IS NULL OR score >= $2)
         ORDER BY score DESC NULLS LAST, found_at DESC
         LIMIT 200",
    )
    .bind(q.status)
    .bind(q.min_score)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewJob {
    source: String,
    title: String,
    company: Option<String>,
    url: String,
    description: Option<String>,
    category: Option<String>,
    job_type: Option<String>,
    location: Option<String>,
    pay_min: Option<i32>,
    pay_max: Option<i32>,
    pay_period: Option<String>,
    pay_text: Option<String>,
    posted_at: Option<DateTime<Utc>>,
    status: Option<String>, // only 'queued' is honoured; anything else means 'new'
}

/// n8n posts every listing it finds. The URL is the dedup key, and listings whose published pay
/// is below your floor (same pay period only) are filed straight away as 'skipped'.
pub async fn create(
    _: Authed,
    State(s): State<AppState>,
    Json(b): Json<NewJob>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let url = b.url.trim().to_string();
    if b.title.trim().is_empty() || url.is_empty() {
        return Err(AppError::BadRequest("title and url are required".into()));
    }
    let floor = match b.pay_period.as_deref() {
        Some("hour") => Some(s.cfg.min_hourly),
        Some("month") => Some(s.cfg.min_monthly),
        Some("year") => Some(s.cfg.min_yearly),
        _ => None,
    };
    let below = matches!((b.pay_max.or(b.pay_min), floor), (Some(p), Some(f)) if p < f);
    let want = if b.status.as_deref() == Some("queued") { "queued" } else { "new" };
    let (status, skip) = if below { ("skipped", Some("below_pay_floor")) } else { (want, None) };

    let job = sqlx::query_as::<_, Job>(
        "INSERT INTO jobs (source, title, company, url, description, category, job_type, location,
                           pay_min, pay_max, pay_period, pay_text, posted_at, status, skip_reason)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         ON CONFLICT (url) DO NOTHING
         RETURNING *",
    )
    .bind(b.source)
    .bind(b.title.trim())
    .bind(b.company)
    .bind(&url)
    .bind(b.description)
    .bind(b.category)
    .bind(b.job_type)
    .bind(b.location)
    .bind(b.pay_min)
    .bind(b.pay_max)
    .bind(b.pay_period)
    .bind(b.pay_text)
    .bind(b.posted_at)
    .bind(status)
    .bind(skip)
    .fetch_optional(&s.db)
    .await?;

    match job {
        Some(j) => Ok((StatusCode::CREATED, Json(serde_json::to_value(j).unwrap_or(Value::Null)))),
        None => Ok((StatusCode::OK, Json(json!({ "duplicate": true })))),
    }
}

#[derive(Deserialize)]
pub struct UpdateJob {
    status: Option<String>,
    score: Option<i16>,
    score_reason: Option<String>,
    notes: Option<String>,
}

pub async fn update(
    _: Authed,
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(b): Json<UpdateJob>,
) -> Result<Json<Job>, AppError> {
    let (prev,): (String,) = sqlx::query_as("SELECT status FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_one(&s.db)
        .await?;

    let job = sqlx::query_as::<_, Job>(
        "UPDATE jobs SET
            status       = COALESCE($2, status),
            score        = COALESCE($3, score),
            score_reason = COALESCE($4, score_reason),
            notes        = COALESCE($5, notes),
            applied_at   = CASE WHEN $2::text = 'applied' THEN COALESCE(applied_at, now()) ELSE applied_at END
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&b.status)
    .bind(b.score)
    .bind(b.score_reason)
    .bind(b.notes)
    .fetch_one(&s.db)
    .await?;

    // Rule: the moment a job is marked applied, schedule a follow-up task for 5 days later.
    if b.status.as_deref() == Some("applied") && prev != "applied" {
        sqlx::query(
            "INSERT INTO tasks (title, area, priority, due_at, source)
             VALUES ($1, 'jobs', 2, now() + interval '5 days', 'system')",
        )
        .bind(format!(
            "Follow up: {} @ {}",
            job.title,
            job.company.as_deref().unwrap_or("?")
        ))
        .execute(&s.db)
        .await?;
    }
    Ok(Json(job))
}
