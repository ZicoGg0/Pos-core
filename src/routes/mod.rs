pub mod assistant;
pub mod auth_routes;
pub mod checkins;
pub mod comm;
pub mod jobs;
pub mod lifestyle;
pub mod study;
pub mod tasks;

use crate::AppState;
use axum::{extract::DefaultBodyLimit, routing::{get, patch, post, put}, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/login", post(auth_routes::login))
        .route("/assistant/chat", post(assistant::chat))
        .route("/auth/signup", post(auth_routes::signup))
        .route("/auth/me", get(auth_routes::me))
        .route("/comm/pair/code", post(comm::pair_code))
        .route("/comm/pair/join", post(comm::pair_join))
        .route("/comm/conversations", get(comm::conversations))
        .route("/comm/conversations/:id/messages", get(comm::messages_list).post(comm::messages_create))
        .route("/comm/media/:id", put(comm::media_put).get(comm::media_get).layer(DefaultBodyLimit::max(11 * 1024 * 1024)))
        .route("/tasks", get(tasks::list).post(tasks::create))
        .route("/tasks/:id", patch(tasks::update).delete(tasks::remove))
        .route("/jobs", get(jobs::list).post(jobs::create))
        .route("/jobs/:id", patch(jobs::update))
        .route("/fun/profile", get(lifestyle::profile_get).put(lifestyle::profile_put))
        .route("/fun/picks", get(lifestyle::picks_list).post(lifestyle::picks_create))
        .route("/fun/picks/request", post(lifestyle::picks_request))
        .route("/fun/picks/:id", patch(lifestyle::picks_patch))
        .route("/health/weights", get(lifestyle::weights_list))
        .route("/health/weights/:day", put(lifestyle::weights_put).delete(lifestyle::weights_del))
        .route("/health/workouts", get(lifestyle::workouts_list).post(lifestyle::workouts_create))
        .route("/health/workouts/:id", axum::routing::delete(lifestyle::workouts_del))
        .route("/health/suggestions", get(lifestyle::sugg_list).post(lifestyle::sugg_upsert))
        .route("/health/suggestions/request", post(lifestyle::sugg_request))
        .route("/health/meals", get(lifestyle::meals_list).post(lifestyle::meals_create))
        .route("/health/meals/:id", axum::routing::delete(lifestyle::meals_del))
        .route("/study/subjects", get(study::subjects_list).post(study::subjects_create))
        .route("/study/subjects/:id", patch(study::subjects_patch))
        .route("/study/sessions", get(study::sessions_list).post(study::sessions_create))
        .route("/study/sessions/:id", axum::routing::delete(study::sessions_del))
        .route("/study/reviews", get(study::reviews_list).post(study::reviews_create))
        .route("/study/reviews/:id/answer", post(study::reviews_answer))
        .route("/study/reviews/:id", axum::routing::delete(study::reviews_del))
        .route("/checkins", get(checkins::list).post(checkins::upsert))
}
