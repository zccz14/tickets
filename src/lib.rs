pub mod api;
pub mod auth;
pub mod crypto;
pub mod cybion;
pub mod db;
pub mod error;
pub mod machine;
pub mod model;
pub mod prompt;
pub mod web;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{Router, middleware::from_fn_with_state, routing::get};
use sqlx::SqlitePool;

use crate::{auth::AuthGate, crypto::SecretBox, cybion::CybionClient, error::AppResult};

pub const DEFAULT_AUTH_BASE_URL: &str = "https://auth.ntnl.io";
pub const DEFAULT_LINKIT_BASE_URL: &str = "https://linkit.ntnl.io";
pub const DEFAULT_CYBION_BASE_URL: &str = "https://cybion.ntnl.io";
pub const DEFAULT_PUBLIC_BASE_URL: &str = "https://tickets.ntnl.io";
/// The Auth Mini audience this deployment accepts. Auth Mini mints one token
/// per business hostname, so this must match the host the browser talks to.
pub const AUTH_AUDIENCE: &str = "tickets.ntnl.io";

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub secrets: Arc<SecretBox>,
    pub auth: AuthGate,
    pub cybion: CybionClient,
    pub auth_base_url: String,
    pub linkit_base_url: String,
    pub public_base_url: String,
    /// When set, every browser request is treated as this user. Local
    /// development only; it is announced loudly at boot.
    pub dev_user: Option<String>,
}

pub struct Options {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub public_base_url: Option<String>,
    pub dev_user: Option<String>,
}

impl Options {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            listen: "0.0.0.0:8080".parse().expect("默认监听地址"),
            data_dir,
            public_base_url: None,
            dev_user: None,
        }
    }
}

pub async fn bootstrap(options: Options) -> anyhow::Result<AppState> {
    std::fs::create_dir_all(&options.data_dir)?;
    let database_path = options.data_dir.join("tickets.db");
    let db = db::connect(&database_path).await?;
    let secrets = Arc::new(SecretBox::load_or_create(&options.data_dir)?);

    let cybion_base = db::setting(&db, "cybion_base_url", DEFAULT_CYBION_BASE_URL).await;
    let auth_base = db::setting(&db, "auth_base_url", DEFAULT_AUTH_BASE_URL).await;
    let linkit_base = db::setting(&db, "linkit_base_url", DEFAULT_LINKIT_BASE_URL).await;
    let stored_public = db::setting(&db, "public_base_url", DEFAULT_PUBLIC_BASE_URL).await;
    let public_base_url = options
        .public_base_url
        .clone()
        .unwrap_or(stored_public)
        .trim_end_matches('/')
        .to_owned();
    db::put_setting(&db, "public_base_url", &public_base_url).await?;

    if options.dev_user.is_some() {
        tracing::warn!(
            "以 --dev-user 启动：所有请求都被视为同一个用户，Auth Mini 校验已关闭。请勿在生产环境使用。"
        );
    }

    Ok(AppState {
        db,
        secrets,
        auth: AuthGate::new(auth_base.clone(), AUTH_AUDIENCE),
        cybion: CybionClient::new(cybion_base),
        auth_base_url: auth_base,
        linkit_base_url: linkit_base,
        public_base_url,
        dev_user: options.dev_user,
    })
}

pub fn router(state: AppState) -> Router {
    let browser =
        api::browser_routes().route_layer(from_fn_with_state(state.clone(), auth::require_user));
    let agent =
        api::agent_routes().route_layer(from_fn_with_state(state.clone(), auth::require_agent));

    Router::new()
        .route("/api/health", get(api::health))
        .route("/api/meta", get(api::meta))
        .merge(browser)
        .merge(agent)
        .fallback(web::static_handler)
        .with_state(state)
}

/// Hands a comment to the reporter's Thread without blocking the HTTP response.
pub fn spawn_reporter_progress(state: AppState, ticket_id: String, nudge: String) {
    tokio::spawn(async move {
        if let Err(error) = api::tickets::progress_reporter_thread(&state, &ticket_id, &nudge).await
        {
            tracing::warn!("推进报障 Thread 失败: {error}");
        }
    });
}

pub fn spawn_thread_creation(state: AppState, ticket_id: String) {
    tokio::spawn(async move {
        if let Err(error) = api::tickets::ensure_reporter_thread(&state, &ticket_id).await {
            tracing::warn!("创建报障 Thread 失败: {error}");
        }
    });
}

pub async fn health(state: &AppState) -> AppResult<()> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(())
}
