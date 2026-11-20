//! Communication: pairing codes, 1:1 conversations, messages, media.
//! Every handler uses `Member` (JWT only) and checks conversation membership itself.
use crate::{auth::Member, error::AppError, AppState};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use uuid::Uuid;

type R = Result<Json<Value>, AppError>;

const MAX_MEDIA: usize = 10 * 1024 * 1024;
const CODE_TTL_SECS: i64 = 900;
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no 0/O/1/I
const MIMES: &[&str] = &[
    "image/jpeg", "image/png", "image/webp", "image/gif",
    "audio/webm", "audio/ogg", "audio/mp4", "audio/mpeg", "audio/wav", "audio/x-m4a", "audio/aac",
    "video/mp4", "video/webm", "application/pdf", "text/plain",
];

async fn require_member(s: &AppState, conv: Uuid, user: Uuid) -> Result<(), AppError> {
    let ok: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM conversations WHERE id = $1 AND (user_a = $2 OR user_b = $2)")
        .bind(conv)
        .bind(user)
        .fetch_optional(&s.db)
        .await?;
    // 404 rather than 403 so ids of other people's chats can't be probed.
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

fn random_code() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    let more = Uuid::new_v4().into_bytes();
    (0..8).map(|i| ALPHABET[(if i < 4 { bytes[i] } else { more[i] }) as usize % ALPHABET.len()] as char).collect()
}

pub async fn pair_code(m: Member, State(s): State<AppState>) -> R {
    sqlx::query("DELETE FROM pair_codes WHERE user_id = $1 OR expires_at < now()").bind(m.id).execute(&s.db).await?;
    for _ in 0..5 {
        let code = random_code();
        let r = sqlx::query("INSERT INTO pair_codes (code, user_id, expires_at) VALUES ($1, $2, now() + make_interval(secs => $3))")
            .bind(&code)
            .bind(m.id)
            .bind(CODE_TTL_SECS as f64)
            .execute(&s.db)
            .await;
        match r {
            Ok(_) => return Ok(Json(json!({ "code": code, "expires_in": CODE_TTL_SECS }))),
            Err(sqlx::Error::Database(d)) if d.code().as_deref() == Some("23505") => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::Internal("could not allocate a pairing code".into()))
}

#[derive(Deserialize)]
pub struct JoinIn { code: String }

pub async fn pair_join(m: Member, State(s): State<AppState>, Json(b): Json<JoinIn>) -> R {
    // Guess limiter: 10 wrong codes per 15 minutes per account.
    {
        let mut f = s.join_fails.lock().unwrap();
        if let Some((n, since)) = f.get(&m.id).copied() {
            if since.elapsed() > Duration::from_secs(CODE_TTL_SECS as u64) {
                f.remove(&m.id);
            } else if n >= 10 {
                return Err(AppError::BadRequest("too many wrong codes, try again in a few minutes".into()));
            }
        }
    }
    let code = b.code.trim().to_uppercase().replace([' ', '-'], "");
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT user_id FROM pair_codes WHERE code = $1 AND expires_at > now()")
        .bind(&code)
        .fetch_optional(&s.db)
        .await?;
    let Some((other,)) = row else {
        let mut f = s.join_fails.lock().unwrap();
        let e = f.entry(m.id).or_insert((0, Instant::now()));
        e.0 += 1;
        return Err(AppError::BadRequest("that code is wrong or has expired".into()));
    };
    if other == m.id {
        return Err(AppError::BadRequest("that is your own code; share it with the other person".into()));
    }
    sqlx::query("INSERT INTO conversations (user_a, user_b) VALUES (LEAST($1::uuid, $2::uuid), GREATEST($1::uuid, $2::uuid)) ON CONFLICT DO NOTHING")
        .bind(m.id)
        .bind(other)
        .execute(&s.db)
        .await?;
    sqlx::query("DELETE FROM pair_codes WHERE code = $1").bind(&code).execute(&s.db).await?;
    let v: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', c.id, 'peer', jsonb_build_object('id', u.id, 'name', u.name))
         FROM conversations c JOIN users u ON u.id = $2
         WHERE c.user_a = LEAST($1::uuid, $2::uuid) AND c.user_b = GREATEST($1::uuid, $2::uuid)",
    )
    .bind(m.id)
    .bind(other)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(v))
}

pub async fn conversations(m: Member, State(s): State<AppState>) -> R {
    let v: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(x ORDER BY x->>'sort' DESC), '[]'::jsonb) FROM (
           SELECT jsonb_build_object(
                    'id', c.id,
                    'peer', jsonb_build_object('id', u.id, 'name', u.name),
                    'last', (SELECT jsonb_build_object('seq', m.seq, 'kind', m.kind, 'body', m.body, 'sender', m.sender, 'created_at', m.created_at)
                             FROM messages m WHERE m.conversation_id = c.id ORDER BY m.seq DESC LIMIT 1),
                    'sort', coalesce((SELECT to_char(max(m.created_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US') FROM messages m WHERE m.conversation_id = c.id),
                                     to_char(c.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US'))
                  ) AS x
           FROM conversations c
           JOIN users u ON u.id = CASE WHEN c.user_a = $1 THEN c.user_b ELSE c.user_a END
           WHERE c.user_a = $1 OR c.user_b = $1
         ) t",
    )
    .bind(m.id)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct AfterQ { after_seq: Option<i64> }

pub async fn messages_list(m: Member, State(s): State<AppState>, Path(conv): Path<Uuid>, Query(q): Query<AfterQ>) -> R {
    require_member(&s, conv, m.id).await?;
    let v: Value = match q.after_seq {
        Some(after) => sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.seq), '[]'::jsonb)
             FROM (SELECT * FROM messages WHERE conversation_id = $1 AND seq > $2 ORDER BY seq LIMIT 200) x",
        ).bind(conv).bind(after).fetch_one(&s.db).await?,
        None => sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(x) ORDER BY x.seq), '[]'::jsonb)
             FROM (SELECT * FROM messages WHERE conversation_id = $1 ORDER BY seq DESC LIMIT 100) x",
        ).bind(conv).fetch_one(&s.db).await?,
    };
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct MsgIn {
    id: Uuid,
    kind: String,
    body: Option<String>,
    media_id: Option<Uuid>,
    duration_ms: Option<i32>,
}

pub async fn messages_create(m: Member, State(s): State<AppState>, Path(conv): Path<Uuid>, Json(b): Json<MsgIn>) -> Result<(StatusCode, Json<Value>), AppError> {
    require_member(&s, conv, m.id).await?;
    let body = b.body.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    if body.as_ref().map(|x| x.chars().count() > 4000).unwrap_or(false) {
        return Err(AppError::BadRequest("message is too long (4000 characters max)".into()));
    }
    match b.kind.as_str() {
        "text" => {
            if body.is_none() { return Err(AppError::BadRequest("message is empty".into())); }
            if b.media_id.is_some() { return Err(AppError::BadRequest("text messages cannot carry media".into())); }
        }
        "image" | "audio" | "file" => {
            let Some(mid) = b.media_id else { return Err(AppError::BadRequest("this message needs its media uploaded first".into())) };
            let ok: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM media WHERE id = $1 AND conversation_id = $2")
                .bind(mid).bind(conv).fetch_optional(&s.db).await?;
            if ok.is_none() { return Err(AppError::BadRequest("media not found in this conversation".into())); }
        }
        _ => return Err(AppError::BadRequest("unknown message kind".into())),
    }
    let ins = sqlx::query(
        "INSERT INTO messages (id, conversation_id, sender, kind, body, media_id, duration_ms) VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(b.id).bind(conv).bind(m.id).bind(&b.kind).bind(&body).bind(b.media_id).bind(b.duration_ms.map(|d| d.clamp(0, 3_600_000)))
    .execute(&s.db).await?;
    // A retried send (same client id) returns the stored message; it must be this sender's, in this chat.
    let v: Option<Value> = sqlx::query_scalar("SELECT to_jsonb(x) FROM messages x WHERE id = $1 AND conversation_id = $2 AND sender = $3")
        .bind(b.id).bind(conv).bind(m.id).fetch_optional(&s.db).await?;
    let v = v.ok_or_else(|| AppError::BadRequest("message id already used".into()))?;
    let code = if ins.rows_affected() == 1 { StatusCode::CREATED } else { StatusCode::OK };
    Ok((code, Json(v)))
}

fn clean_mime(h: &HeaderMap) -> String {
    let raw = h.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let base = raw.split(';').next().unwrap_or("").trim().to_lowercase();
    if MIMES.contains(&base.as_str()) { base } else { "application/octet-stream".into() }
}

pub async fn media_put(m: Member, State(s): State<AppState>, Path(id): Path<Uuid>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, AppError> {
    let conv: Uuid = headers.get("x-conversation").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError::BadRequest("x-conversation header is required".into()))?;
    require_member(&s, conv, m.id).await?;
    if body.is_empty() { return Err(AppError::BadRequest("empty file".into())); }
    if body.len() > MAX_MEDIA { return Err(AppError::BadRequest("file is larger than 10 MB".into())); }
    let r = sqlx::query("INSERT INTO media (id, conversation_id, uploader, mime, size, data) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO NOTHING")
        .bind(id).bind(conv).bind(m.id).bind(clean_mime(&headers)).bind(body.len() as i32).bind(body.to_vec())
        .execute(&s.db).await?;
    if r.rows_affected() == 0 {
        // Retried upload is fine only if it is the same uploader in the same chat.
        let same: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM media WHERE id = $1 AND uploader = $2 AND conversation_id = $3")
            .bind(id).bind(m.id).bind(conv).fetch_optional(&s.db).await?;
        if same.is_none() { return Err(AppError::BadRequest("media id already used".into())); }
        return Ok(StatusCode::OK);
    }
    Ok(StatusCode::CREATED)
}

pub async fn media_get(m: Member, State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, AppError> {
    let row: Option<(String, Vec<u8>, Uuid)> = sqlx::query_as("SELECT mime, data, conversation_id FROM media WHERE id = $1")
        .bind(id).fetch_optional(&s.db).await?;
    let (mime, data, conv) = row.ok_or(AppError::NotFound)?;
    require_member(&s, conv, m.id).await?;
    let mut r = (StatusCode::OK, data).into_response();
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("content-security-policy", HeaderValue::from_static("sandbox; default-src 'none'"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=31536000, immutable"));
    Ok(r)
}
