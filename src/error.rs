use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest(String),
    Internal(String),
    TooManyRequests(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "not allowed for this account".to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m),
            AppError::Internal(m) => {
                tracing::error!("internal error: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            // Check-constraint violations (bad area/status/priority) are the caller's fault.
            sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23514") => {
                AppError::BadRequest("one of the values is not allowed for that field".into())
            }
            sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23505") => AppError::BadRequest("that already exists".into()),
            sqlx::Error::Database(ref d) if d.code().as_deref() == Some("23503") => AppError::BadRequest("the subject does not exist".into()),
            other => AppError::Internal(other.to_string()),
        }
    }
}
