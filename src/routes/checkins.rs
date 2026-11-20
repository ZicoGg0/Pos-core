use crate::{auth::Authed, error::AppError, AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, FromRow)]
pub struct Checkin {
    day: NaiveDate,
    kind: String,
    mood: Option<i16>,
    energy: Option<i16>,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct RangeQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

pub async fn list(
    _: Authed,
    State(s): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<Json<Vec<Checkin>>, AppError> {
    let rows = sqlx::query_as::<_, Checkin>(
        "SELECT * FROM checkins
         WHERE ($1::date IS NULL OR day >= $1) AND ($2::date IS NULL OR day <= $2)
         ORDER BY day DESC, kind
         LIMIT 400",
    )
    .bind(q.from)
    .bind(q.to)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewCheckin {
    day: NaiveDate,
    kind: String,
    mood: Option<i16>,
    energy: Option<i16>,
    note: Option<String>,
}

pub async fn upsert(
    _: Authed,
    State(s): State<AppState>,
    Json(b): Json<NewCheckin>,
) -> Result<Json<Checkin>, AppError> {
    let row = sqlx::query_as::<_, Checkin>(
        "INSERT INTO checkins (day, kind, mood, energy, note)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (day, kind) DO UPDATE SET
            mood   = EXCLUDED.mood,
            energy = EXCLUDED.energy,
            note   = EXCLUDED.note
         RETURNING *",
    )
    .bind(b.day)
    .bind(b.kind)
    .bind(b.mood)
    .bind(b.energy)
    .bind(b.note)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(row))
}
