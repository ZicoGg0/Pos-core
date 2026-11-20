use crate::{auth::Authed, error::AppError, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Serialize, FromRow)]
pub struct Task {
    id: Uuid,
    title: String,
    notes: Option<String>,
    status: String,
    priority: i16,
    area: String,
    due_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    source: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    status: Option<String>,
    area: Option<String>,
}

pub async fn list(
    _: Authed,
    State(s): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<Task>>, AppError> {
    let rows = sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks
         WHERE ($1::text IS NULL OR status = $1)
           AND ($2::text IS NULL OR area = $2)
         ORDER BY (status = 'done'), due_at NULLS LAST, priority, created_at DESC
         LIMIT 500",
    )
    .bind(q.status)
    .bind(q.area)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewTask {
    title: String,
    notes: Option<String>,
    priority: Option<i16>,
    area: Option<String>,
    due_at: Option<DateTime<Utc>>,
    source: Option<String>,
}

pub async fn create(
    _: Authed,
    State(s): State<AppState>,
    Json(b): Json<NewTask>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    if b.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (title, notes, priority, area, due_at, source)
         VALUES ($1, $2, COALESCE($3, 3), COALESCE($4, 'daily'), $5, COALESCE($6, 'app'))
         RETURNING *",
    )
    .bind(b.title.trim())
    .bind(b.notes)
    .bind(b.priority)
    .bind(b.area)
    .bind(b.due_at)
    .bind(b.source)
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(task)))
}

#[derive(Deserialize)]
pub struct UpdateTask {
    title: Option<String>,
    notes: Option<String>,
    status: Option<String>,
    priority: Option<i16>,
    area: Option<String>,
    due_at: Option<DateTime<Utc>>,
}

pub async fn update(
    _: Authed,
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(b): Json<UpdateTask>,
) -> Result<Json<Task>, AppError> {
    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET
            title        = COALESCE($2, title),
            notes        = COALESCE($3, notes),
            status       = COALESCE($4, status),
            priority     = COALESCE($5, priority),
            area         = COALESCE($6, area),
            due_at       = COALESCE($7, due_at),
            completed_at = CASE WHEN $4::text = 'done' THEN now()
                                WHEN $4::text IS NOT NULL THEN NULL
                                ELSE completed_at END,
            updated_at   = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(b.title)
    .bind(b.notes)
    .bind(b.status)
    .bind(b.priority)
    .bind(b.area)
    .bind(b.due_at)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(task))
}

pub async fn remove(
    _: Authed,
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let res = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
