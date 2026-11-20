//! In-app AI assistant. The browser sends the chat; this route forwards it to the n8n
//! "Assistant" workflow (which holds the AI keys and the tools) and returns the reply.
//! Owner only: the assistant can read and change the owner's data.
use crate::{auth::Authed, error::AppError, AppState};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Deserialize)]
pub struct Turn { role: String, content: String }

#[derive(Deserialize)]
pub struct ChatIn { message: String, history: Option<Vec<Turn>> }

pub async fn chat(_: Authed, State(s): State<AppState>, Json(b): Json<ChatIn>) -> Result<Json<Value>, AppError> {
    let message = b.message.trim().to_string();
    if message.is_empty() { return Err(AppError::BadRequest("write a message first".into())); }
    if message.chars().count() > 2000 { return Err(AppError::BadRequest("message is too long (2000 characters max)".into())); }
    // Keep the last 12 turns, each capped, so a long chat cannot grow the prompt without limit.
    let history: Vec<Value> = b.history.unwrap_or_default().into_iter().rev().take(12).collect::<Vec<_>>().into_iter().rev()
        .filter(|t| t.role == "user" || t.role == "assistant")
        .map(|t| json!({ "role": t.role, "content": t.content.chars().take(2000).collect::<String>() }))
        .collect();

    let url = format!("{}/webhook/assistant", s.cfg.n8n_base.trim_end_matches('/'));
    let res = reqwest::Client::new()
        .post(&url)
        .bearer_auth(&s.cfg.service_key)
        .json(&json!({ "message": message, "history": history }))
        .timeout(Duration::from_secs(120))
        .send()
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return Err(AppError::BadRequest("the assistant took too long to answer, try again".into())),
        Err(e) => {
            tracing::warn!("assistant webhook unreachable: {e}");
            return Err(AppError::BadRequest("the assistant is not reachable right now".into()));
        }
    };
    if !res.status().is_success() {
        tracing::warn!("assistant webhook answered {}", res.status());
        let msg = if res.status().as_u16() == 404 { "the assistant workflow is not published in n8n yet" } else { "the assistant could not answer, try again" };
        return Err(AppError::BadRequest(msg.into()));
    }
    let v: Value = res.json().await.unwrap_or(Value::Null);
    let reply = v.get("reply").and_then(|r| r.as_str()).unwrap_or("").trim().to_string();
    if reply.is_empty() {
        return Err(AppError::BadRequest("the assistant returned no answer, try again".into()));
    }
    Ok(Json(json!({ "reply": reply })))
}
