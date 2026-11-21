use crate::{auth::CurrentUser, error::AppError, AppState};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

type R = Result<Json<Value>, AppError>;
type RC = Result<(StatusCode, Json<Value>), AppError>;

async fn owns_subject(s: &AppState, user: Uuid, subject: Uuid) -> Result<(), AppError> {
    let ok: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM subjects WHERE id = $1 AND user_id = $2")
        .bind(subject).bind(user).fetch_optional(&s.db).await?;
    ok.map(|_| ()).ok_or_else(|| AppError::BadRequest("that subject does not exist".into()))
}

pub async fn subjects_list(u: CurrentUser, State(s): State<AppState>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM (SELECT * FROM subjects WHERE user_id = $1 ORDER BY archived, name) x",
    ).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct SubjectIn { name: String, goal: Option<String>, weekly_minutes: Option<i32> }

pub async fn subjects_create(u: CurrentUser, State(s): State<AppState>, Json(b): Json<SubjectIn>) -> RC {
    if b.name.trim().is_empty() { return Err(AppError::BadRequest("name is required".into())); }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO subjects (user_id, name, goal, weekly_minutes) VALUES ($1, $2, $3, $4) RETURNING to_jsonb(subjects)",
    ).bind(u.id).bind(b.name.trim()).bind(b.goal).bind(b.weekly_minutes).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct SubjectPatch { name: Option<String>, goal: Option<String>, weekly_minutes: Option<i32>, archived: Option<bool> }

pub async fn subjects_patch(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<SubjectPatch>) -> R {
    let v: Value = sqlx::query_scalar(
        "UPDATE subjects SET name = COALESCE($2, name), goal = COALESCE($3, goal),
            weekly_minutes = COALESCE($4, weekly_minutes), archived = COALESCE($5, archived)
         WHERE id = $1 AND user_id = $6 RETURNING to_jsonb(subjects)",
    ).bind(id).bind(b.name.map(|n| n.trim().to_string())).bind(b.goal).bind(b.weekly_minutes).bind(b.archived).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct SinceQ { since: Option<NaiveDate> }

pub async fn sessions_list(u: CurrentUser, State(s): State<AppState>, Query(q): Query<SinceQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM study_sessions WHERE user_id = $2 AND day >= COALESCE($1, current_date - 7) ORDER BY day DESC, created_at DESC LIMIT 500) x",
    ).bind(q.since).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct SessionIn { subject_id: Uuid, day: Option<NaiveDate>, minutes: i32, notes: Option<String> }

pub async fn sessions_create(u: CurrentUser, State(s): State<AppState>, Json(b): Json<SessionIn>) -> RC {
    owns_subject(&s, u.id, b.subject_id).await?;
    let v: Value = sqlx::query_scalar(
        "INSERT INTO study_sessions (user_id, subject_id, day, minutes, notes) VALUES ($1, $2, COALESCE($3, current_date), $4, $5) RETURNING to_jsonb(study_sessions)",
    ).bind(u.id).bind(b.subject_id).bind(b.day).bind(b.minutes).bind(b.notes).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

pub async fn sessions_del(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM study_sessions WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ReviewQ { due: Option<bool>, on: Option<NaiveDate> }

pub async fn reviews_list(u: CurrentUser, State(s): State<AppState>, Query(q): Query<ReviewQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.due_on, x.created_at), '[]'::jsonb) FROM
         (SELECT r.*, s.name AS subject FROM reviews r JOIN subjects s ON s.id = r.subject_id
          WHERE r.user_id = $3 AND ($1::bool IS NOT TRUE OR r.due_on <= COALESCE($2, current_date))
          ORDER BY r.due_on, r.created_at LIMIT 200) x",
    ).bind(q.due).bind(q.on).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct ReviewIn { subject_id: Uuid, prompt: String, answer: Option<String> }

pub async fn reviews_create(u: CurrentUser, State(s): State<AppState>, Json(b): Json<ReviewIn>) -> RC {
    if b.prompt.trim().is_empty() { return Err(AppError::BadRequest("prompt is required".into())); }
    owns_subject(&s, u.id, b.subject_id).await?;
    let v: Value = sqlx::query_scalar(
        "INSERT INTO reviews (user_id, subject_id, prompt, answer) VALUES ($1, $2, $3, $4) RETURNING to_jsonb(reviews)",
    ).bind(u.id).bind(b.subject_id).bind(b.prompt.trim()).bind(b.answer).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct AnswerIn { remembered: bool, on: Option<NaiveDate> }

pub async fn reviews_answer(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<AnswerIn>) -> R {
    let (cur,): (i16,) = sqlx::query_as("SELECT box FROM reviews WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).fetch_one(&s.db).await?;
    let nb: i16 = if b.remembered { (cur + 1).min(5) } else { 0 };
    let days: i32 = [1, 3, 7, 14, 30, 60][nb as usize];
    let v: Value = sqlx::query_scalar(
        "UPDATE reviews SET box = $2, due_on = COALESCE($3, current_date) + $4::int, last_reviewed = COALESCE($3, current_date)
         WHERE id = $1 AND user_id = $5 RETURNING to_jsonb(reviews)",
    ).bind(id).bind(nb).bind(b.on).bind(days).bind(u.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

pub async fn reviews_del(u: CurrentUser, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM reviews WHERE id = $1 AND user_id = $2").bind(id).bind(u.id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}
