use crate::{auth::Authed, error::AppError, AppState};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

type R = Result<Json<Value>, AppError>;
type RC = Result<(StatusCode, Json<Value>), AppError>;

// ---------- Fun ----------
pub async fn profile_get(_: Authed, State(s): State<AppState>) -> R {
    let v: Option<Value> = sqlx::query_scalar("SELECT to_jsonb(p) FROM fun_profile p WHERE id = 1").fetch_optional(&s.db).await?;
    Ok(Json(v.unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
pub struct ProfileIn {
    age: Option<i16>,
    about: Option<String>,
    social: Option<String>,
    hobbies: Vec<String>,
    consumes: Vec<String>,
    has_partner: bool,
    partner_interests: Option<String>,
}

pub async fn profile_put(_: Authed, State(s): State<AppState>, Json(b): Json<ProfileIn>) -> R {
    if b.hobbies.is_empty() || b.hobbies.len() > 30 || b.consumes.len() > 30 {
        return Err(AppError::BadRequest("add 1 to 30 hobbies and at most 30 consumed items".into()));
    }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO fun_profile (id, age, about, social, hobbies, consumes, has_partner, partner_interests, completed_at)
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, now())
         ON CONFLICT (id) DO UPDATE SET age = $1, about = $2, social = $3, hobbies = $4, consumes = $5,
            has_partner = $6, partner_interests = $7,
            completed_at = COALESCE(fun_profile.completed_at, now()), updated_at = now()
         RETURNING to_jsonb(fun_profile)",
    )
    .bind(b.age).bind(b.about).bind(b.social).bind(b.hobbies).bind(b.consumes).bind(b.has_partner).bind(b.partner_interests)
    .fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct StatusQ { status: Option<String> }

pub async fn picks_list(_: Authed, State(s): State<AppState>, Query(q): Query<StatusQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM fun_picks WHERE ($1::text IS NULL OR status = $1) ORDER BY created_at DESC LIMIT 200) x",
    ).bind(q.status).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct PickIn { kind: String, title: String, why: Option<String>, url: Option<String>, source: Option<String> }

pub async fn picks_create(_: Authed, State(s): State<AppState>, Json(b): Json<PickIn>) -> RC {
    if b.title.trim().is_empty() { return Err(AppError::BadRequest("title is required".into())); }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO fun_picks (kind, title, why, url, source) VALUES ($1, $2, $3, $4, COALESCE($5, 'app')) RETURNING to_jsonb(fun_picks)",
    ).bind(b.kind).bind(b.title.trim()).bind(b.why).bind(b.url).bind(b.source).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

#[derive(Deserialize)]
pub struct PickPatch { status: Option<String>, feedback: Option<String> }

/// Fire-and-forget call to an n8n webhook (same bearer key as the API's service key).
fn fire(s: &AppState, path: &str, body: Value) {
    let url = format!("{}/webhook/{}", s.cfg.n8n_base.trim_end_matches('/'), path);
    let key = s.cfg.service_key.clone();
    tokio::spawn(async move {
        let r = reqwest::Client::new().post(&url).bearer_auth(key).json(&body).timeout(std::time::Duration::from_secs(10)).send().await;
        match r {
            Ok(x) if x.status().is_success() => tracing::info!("triggered {url}"),
            Ok(x) => tracing::warn!("n8n webhook {url} answered {}", x.status()),
            Err(e) => tracing::warn!("n8n webhook {url} failed: {e}"),
        }
    });
}

pub async fn picks_request(_: Authed, State(s): State<AppState>) -> RC {
    fire(&s, "fun-pick", json!({ "mode": "weekly" }));
    Ok((StatusCode::ACCEPTED, Json(json!({ "requested": true }))))
}

pub async fn picks_patch(_: Authed, State(s): State<AppState>, Path(id): Path<Uuid>, Json(b): Json<PickPatch>) -> R {
    let rejected = b.status.as_deref() == Some("rejected");
    let v: Value = sqlx::query_scalar(
        "UPDATE fun_picks SET status = COALESCE($2, status), feedback = COALESCE($3, feedback) WHERE id = $1 RETURNING to_jsonb(fun_picks)",
    ).bind(id).bind(b.status).bind(b.feedback).fetch_one(&s.db).await?;
    if rejected {
        // Instant swap: ask n8n to pick a replacement using the note you left.
        fire(&s, "fun-pick", json!({ "mode": "replace", "pick_id": id }));
    }
    Ok(Json(v))
}

// ---------- Health ----------
#[derive(Deserialize)]
pub struct DaysQ { days: Option<i32> }

pub async fn weights_list(_: Authed, State(s): State<AppState>, Query(q): Query<DaysQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM weights WHERE day >= current_date - $1::int ORDER BY day DESC) x",
    ).bind(q.days.unwrap_or(90).clamp(1, 3650)).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct WeightIn { kg: f64, note: Option<String> }

pub async fn weights_put(_: Authed, State(s): State<AppState>, Path(day): Path<NaiveDate>, Json(b): Json<WeightIn>) -> R {
    let v: Value = sqlx::query_scalar(
        "INSERT INTO weights (day, kg, note) VALUES ($1, $2, $3)
         ON CONFLICT (day) DO UPDATE SET kg = $2, note = $3 RETURNING to_jsonb(weights)",
    ).bind(day).bind(b.kg).bind(b.note).fetch_one(&s.db).await?;
    Ok(Json(v))
}

pub async fn weights_del(_: Authed, State(s): State<AppState>, Path(day): Path<NaiveDate>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM weights WHERE day = $1").bind(day).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DayQ { day: Option<NaiveDate> }

pub async fn workouts_list(_: Authed, State(s): State<AppState>, Query(q): Query<DayQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM workouts WHERE ($1::date IS NULL OR day = $1) ORDER BY day DESC, created_at LIMIT 200) x",
    ).bind(q.day).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct WorkoutIn { day: Option<NaiveDate>, name: String, minutes: Option<i32>, notes: Option<String> }

pub async fn workouts_create(_: Authed, State(s): State<AppState>, Json(b): Json<WorkoutIn>) -> RC {
    if b.name.trim().is_empty() { return Err(AppError::BadRequest("name is required".into())); }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO workouts (day, name, minutes, notes) VALUES (COALESCE($1, current_date), $2, $3, $4) RETURNING to_jsonb(workouts)",
    ).bind(b.day).bind(b.name.trim()).bind(b.minutes).bind(b.notes).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

pub async fn workouts_del(_: Authed, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM workouts WHERE id = $1").bind(id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn meals_list(_: Authed, State(s): State<AppState>, Query(q): Query<DayQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM meals WHERE ($1::date IS NULL OR day = $1) ORDER BY day DESC, created_at LIMIT 200) x",
    ).bind(q.day).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct MealIn { day: Option<NaiveDate>, slot: String, description: String, source: Option<String> }

pub async fn meals_create(_: Authed, State(s): State<AppState>, Json(b): Json<MealIn>) -> RC {
    if b.description.trim().is_empty() { return Err(AppError::BadRequest("description is required".into())); }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO meals (day, slot, description, source) VALUES (COALESCE($1, current_date), $2, $3, COALESCE($4, 'me')) RETURNING to_jsonb(meals)",
    ).bind(b.day).bind(b.slot).bind(b.description.trim()).bind(b.source).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

pub async fn meals_del(_: Authed, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    let r = sqlx::query("DELETE FROM meals WHERE id = $1").bind(id).execute(&s.db).await?;
    if r.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Daily health suggestions (written by n8n) ----------
pub async fn sugg_list(_: Authed, State(s): State<AppState>, Query(q): Query<DayQ>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(x)), '[]'::jsonb) FROM
         (SELECT * FROM health_suggestions WHERE day = COALESCE($1, current_date) ORDER BY kind DESC, slot) x",
    ).bind(q.day).fetch_one(&s.db).await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct SuggIn { day: Option<NaiveDate>, kind: String, slot: Option<String>, text: String }

pub async fn sugg_upsert(_: Authed, State(s): State<AppState>, Json(b): Json<SuggIn>) -> RC {
    if b.text.trim().is_empty() { return Err(AppError::BadRequest("text is required".into())); }
    let v: Value = sqlx::query_scalar(
        "INSERT INTO health_suggestions (day, kind, slot, text) VALUES (COALESCE($1, current_date), $2, COALESCE($3, ''), $4)
         ON CONFLICT (day, kind, slot) DO UPDATE SET text = $4, created_at = now() RETURNING to_jsonb(health_suggestions)",
    ).bind(b.day).bind(b.kind).bind(b.slot).bind(b.text.trim()).fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

pub async fn sugg_request(_: Authed, State(s): State<AppState>) -> RC {
    fire(&s, "health-suggest", json!({}));
    Ok((StatusCode::ACCEPTED, Json(json!({ "requested": true }))))
}
