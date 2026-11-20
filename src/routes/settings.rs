//! Settings: per-user preferences (currency, date format, timezone, theme). Self-scoped via the
//! `Member` extractor (each account reads/writes only its own row), so no owner data is exposed.
use crate::{auth::Member, error::AppError, AppState};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::Value;

type R = Result<Json<Value>, AppError>;

pub async fn prefs_get(m: Member, State(s): State<AppState>) -> R {
    // Get-or-create the row with defaults.
    let v: Value = sqlx::query_scalar(
        "INSERT INTO user_prefs (user_id) VALUES ($1)
         ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
         RETURNING to_jsonb(user_prefs)",
    ).bind(m.id).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct PrefsIn {
    currency: Option<String>,
    date_format: Option<String>,
    timezone: Option<String>,
    theme: Option<String>,
}

pub async fn prefs_put(m: Member, State(s): State<AppState>, Json(b): Json<PrefsIn>) -> R {
    let currency = match b.currency {
        Some(c) => {
            let c = c.trim().to_uppercase();
            if c.chars().count() != 3 || !c.chars().all(|x| x.is_ascii_alphabetic()) {
                return Err(AppError::BadRequest("currency must be a 3-letter code like NGN".into()));
            }
            Some(c)
        }
        None => None,
    };
    let tz = b.timezone.map(|t| t.trim().to_string()).filter(|t| !t.is_empty() && t.len() <= 64);
    let v: Value = sqlx::query_scalar(
        "INSERT INTO user_prefs (user_id, currency, date_format, timezone, theme)
         VALUES ($1, COALESCE($2,'NGN'), COALESCE($3,'DMY'), COALESCE($4,'Africa/Lagos'), $5)
         ON CONFLICT (user_id) DO UPDATE SET
            currency    = COALESCE($2, user_prefs.currency),
            date_format = COALESCE($3, user_prefs.date_format),
            timezone    = COALESCE($4, user_prefs.timezone),
            theme       = COALESCE($5, user_prefs.theme),
            updated_at  = now()
         RETURNING to_jsonb(user_prefs)",
    ).bind(m.id).bind(currency).bind(b.date_format).bind(tz).bind(b.theme).fetch_one(&s.db).await?;
    Ok(Json(v))
}
