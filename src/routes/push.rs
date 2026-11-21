//! Web Push notifications. The browser/TWA service worker subscribes and posts its subscription
//! here; the server sends encrypted push messages (VAPID) when a chat message arrives and on a
//! daily schedule (triggered by n8n). Push is optional: with no VAPID keys configured it is a no-op.
use crate::{auth::{Authed, Member}, error::AppError, AppState};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;
use web_push::{ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder, URL_SAFE_NO_PAD};

/// Send one notification to every device a user has registered. Dead subscriptions (404/410) are
/// pruned. Returns how many were delivered. No-op (0) when VAPID is not configured.
pub async fn send_to_user(s: &AppState, user_id: Uuid, title: &str, body: &str, url: &str) -> i64 {
    if s.cfg.vapid_private.is_empty() {
        return 0;
    }
    let subs: Vec<(String, String, String)> =
        match sqlx::query_as("SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&s.db)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("push: subscription lookup failed: {e}");
                return 0;
            }
        };
    let payload = json!({ "title": title, "body": body, "url": url }).to_string();
    let client = reqwest::Client::new();
    let mut sent = 0i64;
    for (endpoint, p256dh, auth) in subs {
        let info = SubscriptionInfo::new(endpoint.clone(), p256dh, auth);
        let sig = match VapidSignatureBuilder::from_base64(&s.cfg.vapid_private, URL_SAFE_NO_PAD, &info) {
            Ok(mut b) => {
                b.add_claim("sub", s.cfg.vapid_subject.clone());
                match b.build() {
                    Ok(x) => x,
                    Err(e) => { tracing::warn!("push: vapid sign failed: {e}"); continue; }
                }
            }
            Err(e) => { tracing::warn!("push: bad vapid key: {e}"); continue; }
        };
        let mut mb = WebPushMessageBuilder::new(&info);
        mb.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
        mb.set_vapid_signature(sig);
        mb.set_ttl(86_400);
        let msg = match mb.build() {
            Ok(m) => m,
            Err(e) => { tracing::warn!("push: build failed: {e}"); continue; }
        };
        let Some(pl) = msg.payload else { continue };
        let mut req = client
            .post(msg.endpoint.to_string())
            .header("TTL", msg.ttl.to_string())
            .header("Content-Encoding", pl.content_encoding.to_str())
            .header("Content-Type", "application/octet-stream");
        for (k, v) in pl.crypto_headers {
            req = req.header(k, v);
        }
        match req.body(pl.content).timeout(Duration::from_secs(10)).send().await {
            Ok(r) if r.status().is_success() => sent += 1,
            Ok(r) if matches!(r.status().as_u16(), 404 | 410) => {
                let _ = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1")
                    .bind(&endpoint).execute(&s.db).await;
            }
            Ok(r) => tracing::warn!("push: endpoint answered {}", r.status()),
            Err(e) => tracing::warn!("push: send failed: {e}"),
        }
    }
    sent
}

/// The VAPID public key the browser needs to subscribe. Any signed-in account may read it.
pub async fn key(_m: Member, State(s): State<AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({ "key": s.cfg.vapid_public })))
}

#[derive(Deserialize)]
pub struct Keys { p256dh: String, auth: String }
#[derive(Deserialize)]
pub struct SubIn { endpoint: String, keys: Keys }

/// Store (or refresh) this account's push subscription for one device.
pub async fn subscribe(m: Member, State(s): State<AppState>, Json(b): Json<SubIn>) -> Result<Json<Value>, AppError> {
    if b.endpoint.trim().is_empty() || b.keys.p256dh.trim().is_empty() || b.keys.auth.trim().is_empty() {
        return Err(AppError::BadRequest("incomplete push subscription".into()));
    }
    sqlx::query(
        "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth) VALUES ($1, $2, $3, $4)
         ON CONFLICT (endpoint) DO UPDATE SET user_id = $1, p256dh = $3, auth = $4",
    )
    .bind(m.id).bind(b.endpoint.trim()).bind(b.keys.p256dh.trim()).bind(b.keys.auth.trim())
    .execute(&s.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct UnsubIn { endpoint: String }

pub async fn unsubscribe(m: Member, State(s): State<AppState>, Json(b): Json<UnsubIn>) -> Result<Json<Value>, AppError> {
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1 AND user_id = $2")
        .bind(&b.endpoint).bind(m.id).execute(&s.db).await?;
    Ok(Json(json!({ "ok": true })))
}

/// Send a test notification to the caller's own devices (used by the Settings toggle).
pub async fn test(m: Member, State(s): State<AppState>) -> Result<Json<Value>, AppError> {
    let n = send_to_user(&s, m.id, "Personal OS", "Notifications are on.", "/").await;
    Ok(Json(json!({ "sent": n })))
}

#[derive(Deserialize)]
pub struct NotifyIn { title: String, body: String, url: Option<String> }

/// Send a notification to the owner. Owner JWT or the n8n service key (Authed). Used for the daily
/// notification: an n8n morning schedule POSTs here.
pub async fn notify(_: Authed, State(s): State<AppState>, Json(b): Json<NotifyIn>) -> Result<Json<Value>, AppError> {
    if b.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    let owner: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE role = 'owner' ORDER BY created_at LIMIT 1")
            .fetch_optional(&s.db).await?;
    let Some((oid,)) = owner else { return Ok(Json(json!({ "sent": 0 }))) };
    let url = b.url.unwrap_or_else(|| "/".into());
    let n = send_to_user(&s, oid, b.title.trim(), b.body.trim(), &url).await;
    Ok(Json(json!({ "sent": n })))
}
