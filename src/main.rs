mod auth;
mod error;
mod routes;

use axum::http::{header, Method};
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashMap, env, net::SocketAddr, sync::{Arc, Mutex}, time::Instant};
use tower_http::{cors::{Any, CorsLayer}, services::ServeDir, trace::TraceLayer};

pub struct Config {
    pub jwt_secret: String,
    pub service_key: String,
    // Pay floors, compared within the listing's own pay period.
    pub min_hourly: i32,
    pub min_monthly: i32,
    pub min_yearly: i32,
    // Where n8n lives on the docker network; the API calls its webhooks to trigger AI picks.
    pub n8n_base: String,
    // Open sign-up for Communication members (they never see the owner's data).
    pub allow_signup: bool,
    pub max_users: i32,
    // Web Push (VAPID). Empty private key disables push.
    pub vapid_public: String,
    pub vapid_private: String,
    pub vapid_subject: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub cfg: Arc<Config>,
    // Wrong pairing-code guesses per user: (count, first failure).
    pub join_fails: Arc<Mutex<HashMap<uuid::Uuid, (u32, Instant)>>>,
    // Failed sign-in attempts per email: (count, first failure).
    pub login_fails: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}

fn require(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required env var {name}"))
}

fn env_int(name: &str, default: i32) -> i32 {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into()))
        .init();

    let cfg = Arc::new(Config {
        jwt_secret: require("JWT_SECRET")?,
        service_key: require("SERVICE_API_KEY")?,
        min_hourly: env_int("MIN_HOURLY", 15),
        min_monthly: env_int("MIN_MONTHLY", 5000),
        min_yearly: env_int("MIN_YEARLY", 50000),
        n8n_base: env::var("N8N_BASE_URL").unwrap_or_else(|_| "http://n8n:5678".into()),
        allow_signup: !matches!(env::var("ALLOW_SIGNUP").unwrap_or_default().to_lowercase().as_str(), "false" | "0" | "no"),
        max_users: env_int("MAX_USERS", 50),
        vapid_public: env::var("VAPID_PUBLIC_KEY").unwrap_or_default(),
        vapid_private: env::var("VAPID_PRIVATE_KEY").unwrap_or_default(),
        vapid_subject: env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:owner@localhost".into()),
    });
    if cfg.jwt_secret.len() < 32 || cfg.service_key.len() < 32 {
        anyhow::bail!("JWT_SECRET and SERVICE_API_KEY must each be at least 32 characters");
    }

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&require("DATABASE_URL")?)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    // First run: create the single owner account from env.
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users").fetch_one(&db).await?;
    if users == 0 {
        let email = require("OWNER_EMAIL")?.trim().to_lowercase();
        let hash = auth::hash_password(&require("OWNER_PASSWORD")?)?;
        let name = env::var("OWNER_NAME").unwrap_or_else(|_| email.split('@').next().unwrap_or("Owner").to_string());
        sqlx::query("INSERT INTO users (email, password_hash, name, role) VALUES ($1, $2, $3, 'owner')")
            .bind(&email)
            .bind(hash)
            .bind(name)
            .execute(&db)
            .await?;
        tracing::info!("created owner account {email}");
    }
    // Accounts that existed before sign-up got a display name from their email.
    sqlx::query("UPDATE users SET name = split_part(email, '@', 1) WHERE name = ''").execute(&db).await?;

    // CORS: the PWA / Tauri webview calls this API from another origin.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::HeaderName::from_static("x-conversation")]);

    // The PWA is served from the same origin as the API (one container, no CORS fuss).
    let web_dir = env::var("WEB_DIR").unwrap_or_else(|_| "web".into());
    let app = routes::router()
        .fallback_service(ServeDir::new(web_dir))
        .with_state(AppState { db, cfg, join_fails: Arc::new(Mutex::new(HashMap::new())), login_fails: Arc::new(Mutex::new(HashMap::new())) })
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()).parse()?;
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
