use crate::{auth::{self, Member}, error::AppError, AppState};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Login {
    email: String,
    password: String,
}

pub async fn login(State(s): State<AppState>, Json(b): Json<Login>) -> Result<Json<Value>, AppError> {
    let row: Option<(Uuid, String, String, String)> =
        sqlx::query_as("SELECT id, password_hash, role, name FROM users WHERE email = $1")
            .bind(b.email.trim().to_lowercase())
            .fetch_optional(&s.db)
            .await?;

    match row {
        Some((id, hash, role, name)) if auth::verify_password(&b.password, &hash) => {
            let token = auth::issue_token(&id.to_string(), &role, &s.cfg.jwt_secret)?;
            Ok(Json(json!({ "token": token, "role": role, "name": name, "id": id })))
        }
        _ => Err(AppError::Unauthorized),
    }
}

#[derive(Deserialize)]
pub struct Signup {
    email: String,
    password: String,
    name: String,
}

/// Open sign-up creates a *member*: they can use Communication only, never the owner's data.
pub async fn signup(State(s): State<AppState>, Json(b): Json<Signup>) -> Result<(StatusCode, Json<Value>), AppError> {
    if !s.cfg.allow_signup {
        return Err(AppError::BadRequest("sign-up is closed on this server".into()));
    }
    let email = b.email.trim().to_lowercase();
    let name = b.name.trim().to_string();
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(AppError::BadRequest("enter a valid email address".into()));
    }
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::BadRequest("name must be 1 to 40 characters".into()));
    }
    if b.password.len() < 8 || b.password.len() > 128 {
        return Err(AppError::BadRequest("password must be 8 to 128 characters".into()));
    }
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM users").fetch_one(&s.db).await?;
    if count >= s.cfg.max_users as i64 {
        return Err(AppError::BadRequest("this server is full".into()));
    }
    let hash = auth::hash_password(&b.password).map_err(|e| AppError::Internal(e.to_string()))?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name, role) VALUES ($1, $2, $3, 'member') RETURNING id",
    )
    .bind(&email)
    .bind(hash)
    .bind(&name)
    .fetch_one(&s.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23505") => {
            AppError::BadRequest("an account with that email already exists".into())
        }
        other => other.into(),
    })?;
    let token = auth::issue_token(&id.to_string(), "member", &s.cfg.jwt_secret)?;
    Ok((StatusCode::CREATED, Json(json!({ "token": token, "role": "member", "name": name, "id": id }))))
}

pub async fn me(m: Member, State(s): State<AppState>) -> Result<Json<Value>, AppError> {
    let v: Value = sqlx::query_scalar("SELECT jsonb_build_object('id', id, 'email', email, 'name', name, 'role', role) FROM users WHERE id = $1")
        .bind(m.id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct PwChange { current: String, new_password: String }

/// Change your own password. Any signed-in account (owner or member). A wrong current password is
/// a 400, not a 401, so the app does not treat it as an expired session and log the user out.
pub async fn change_password(m: Member, State(s): State<AppState>, Json(b): Json<PwChange>) -> Result<Json<Value>, AppError> {
    if b.new_password.len() < 8 || b.new_password.len() > 128 {
        return Err(AppError::BadRequest("new password must be 8 to 128 characters".into()));
    }
    let (hash,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(m.id).fetch_one(&s.db).await?;
    if !auth::verify_password(&b.current, &hash) {
        return Err(AppError::BadRequest("your current password is incorrect".into()));
    }
    let nh = auth::hash_password(&b.new_password).map_err(|e| AppError::Internal(e.to_string()))?;
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1").bind(m.id).bind(nh).execute(&s.db).await?;
    Ok(Json(json!({ "ok": true })))
}
