use crate::{error::AppError, AppState};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    // Tokens issued before sign-up existed have no role: they belong to the owner.
    #[serde(default = "owner_role")]
    role: String,
}

fn owner_role() -> String {
    "owner".into()
}

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash error: {e}"))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn issue_token(user_id: &str, role: &str, secret: &str) -> Result<String, AppError> {
    let exp = (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize;
    encode(
        &Header::default(),
        &Claims { sub: user_id.to_string(), exp, role: role.to_string() },
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Extractor that guards the owner's personal data. Accepts either:
///  - the owner's JWT (from /auth/login), or
///  - the static service key (for n8n and other automations).
/// Member accounts (signed up via /auth/signup) get 403 here.
pub struct Authed;

#[async_trait]
impl FromRequestParts<AppState> for Authed {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = header.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;

        if constant_time_eq(token.as_bytes(), state.cfg.service_key.as_bytes()) {
            return Ok(Authed);
        }
        let c = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.cfg.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?
        .claims;
        if c.role == "owner" { Ok(Authed) } else { Err(AppError::Forbidden) }
    }
}

/// Extractor for Communication: any signed-in person (owner or member), by JWT only.
/// The static service key is deliberately not accepted, so automations cannot read chats.
pub struct Member {
    pub id: uuid::Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for Member {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = header.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;
        let c = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.cfg.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?
        .claims;
        let id = c.sub.parse().map_err(|_| AppError::Unauthorized)?;
        // Reject tokens for accounts that no longer exist.
        let exists: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
        if exists.is_none() {
            return Err(AppError::Unauthorized);
        }
        Ok(Member { id })
    }
}
