pub mod account;
pub mod agent;
pub mod projects;
pub mod tickets;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{AppState, error::AppResult, model::Meta};

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    crate::health(&state).await?;
    Ok(Json(json!({ "status": "ok" })))
}

pub async fn meta(State(state): State<AppState>) -> Json<Meta> {
    Json(Meta {
        auth_base_url: state.auth_base_url.clone(),
        linkit_base_url: state.linkit_base_url.clone(),
        cybion_base_url: state.cybion.base().to_owned(),
        public_base_url: state.public_base_url.clone(),
    })
}

pub fn browser_routes() -> Router<AppState> {
    Router::new()
        .route("/api/me", get(account::me))
        .route("/api/me/tickets", get(tickets::my_tickets))
        .route(
            "/api/account/cybion",
            get(account::me)
                .put(account::connect)
                .delete(account::disconnect),
        )
        .route("/api/projects", get(projects::list).post(projects::create))
        .route(
            "/api/projects/{id}",
            get(projects::read).patch(projects::update),
        )
        .route(
            "/api/projects/{id}/grants",
            get(projects::list_grants).post(projects::add_grant),
        )
        .route(
            "/api/projects/{id}/grants/{user_id}",
            axum::routing::delete(projects::remove_grant),
        )
        .route(
            "/api/projects/{id}/tickets",
            get(tickets::list).post(tickets::create),
        )
        .route(
            "/api/tickets/{id}",
            get(tickets::read).patch(tickets::update),
        )
        .route("/api/tickets/{id}/submit", post(tickets::submit))
        .route("/api/tickets/{id}/close", post(tickets::close))
        .route("/api/tickets/{id}/reopen", post(tickets::reopen))
        .route("/api/tickets/{id}/dispatch", post(tickets::dispatch))
        .route(
            "/api/tickets/{id}/thread",
            get(tickets::read_thread).post(tickets::retry_thread),
        )
        .route(
            "/api/tickets/{id}/thread/live",
            get(tickets::live_thread).post(tickets::live_thread_input),
        )
}

pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/whoami", get(agent::whoami))
        .route("/api/agent/tickets/{id}", get(agent::read))
        .route("/api/agent/tickets/{id}/evidence", post(agent::evidence))
        .route("/api/agent/tickets/{id}/resolve", post(agent::resolve))
}
