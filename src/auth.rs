use std::sync::Arc;

use auth_mini_axum::{AuthMiniError, AuthMiniLayer, AuthMiniPrincipal, JwksCachePolicy};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::{Row, SqlitePool};
use tokio::sync::RwLock;

use crate::{
    AppState,
    crypto::{hash_token, now},
    error::{AppError, AppResult},
};

/// A signed-in Auth Mini user, resolved on every browser request.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub id: String,
    pub display_name: Option<String>,
}

/// Which side of a Ticket an agent token speaks for.
#[derive(Clone, Debug)]
pub enum AgentScope {
    /// Bound to exactly one Ticket: the reporter's investigation Thread.
    Reporter {
        ticket_id: String,
        reporter_user_id: String,
    },
    /// Bound to one project: the resolver Ultimate Machine.
    Resolver { project_id: String },
}

#[derive(Clone, Debug)]
pub struct AgentIdentity {
    pub scope: AgentScope,
    pub label: String,
}

/// Lazily-built Auth Mini verifier.
///
/// The JWKS is fetched on first use rather than at boot, so a momentary Auth
/// Mini outage cannot stop the process from starting or a deploy from passing.
#[derive(Clone)]
pub struct AuthGate {
    issuer: String,
    audience: String,
    layer: Arc<RwLock<Option<AuthMiniLayer>>>,
}

impl AuthGate {
    pub fn new(issuer: impl Into<String>, audience: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            layer: Arc::new(RwLock::new(None)),
        }
    }

    async fn layer(&self) -> AppResult<AuthMiniLayer> {
        if let Some(layer) = self.layer.read().await.clone() {
            return Ok(layer);
        }
        let layer = AuthMiniLayer::from_issuer(
            &self.issuer,
            self.audience.clone(),
            JwksCachePolicy::default(),
        )
        .await
        .map_err(|error| match error {
            AuthMiniError::JwksUnavailable => AppError::unavailable("Auth Mini 暂时不可用"),
            AuthMiniError::InvalidIssuer => AppError::internal("Auth Mini issuer 配置错误"),
            AuthMiniError::InvalidToken => AppError::unauthorized("登录已失效"),
        })?;
        *self.layer.write().await = Some(layer.clone());
        Ok(layer)
    }
}

pub fn bearer(headers: &HeaderMap) -> AppResult<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::unauthorized("缺少访问令牌"))
}

fn linkit_name(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("x-linkit-username")?.to_str().ok()?.trim();
    if raw.is_empty() || raw.chars().count() > 64 {
        return None;
    }
    Some(raw.to_owned())
}

/// Browser authentication. `--dev-user` replaces Auth Mini entirely so the app
/// can be exercised locally against a throwaway database.
pub async fn require_user(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let display_name = linkit_name(request.headers());
    let user_id = match &state.dev_user {
        Some(user_id) => user_id.clone(),
        None => {
            let token = match bearer(request.headers()) {
                Ok(token) => token,
                Err(error) => return error.into_response(),
            };
            let layer = match state.auth.layer().await {
                Ok(layer) => layer,
                Err(error) => return error.into_response(),
            };
            let principal = match layer.verifier().verify(&token).await {
                Ok(principal) => principal,
                Err(error) => {
                    let error = match error {
                        AuthMiniError::JwksUnavailable => {
                            AppError::unavailable("Auth Mini 暂时不可用")
                        }
                        _ => AppError::unauthorized("登录已失效，请重新登录"),
                    };
                    return error.into_response();
                }
            };
            principal.subject
        }
    };

    let user = match upsert_user(&state.db, &user_id, display_name.as_deref()).await {
        Ok(user) => user,
        Err(error) => return error.into_response(),
    };
    request.extensions_mut().insert(user);
    next.run(request).await
}

pub async fn upsert_user(
    pool: &SqlitePool,
    user_id: &str,
    display_name: Option<&str>,
) -> AppResult<CurrentUser> {
    sqlx::query(
        "INSERT INTO users(id,display_name,created_at) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING",
    )
    .bind(user_id)
    .bind(display_name)
    .bind(now())
    .execute(pool)
    .await?;
    if let Some(name) = display_name {
        sqlx::query("UPDATE users SET display_name=? WHERE id=? AND (display_name IS NULL OR display_name<>?)")
            .bind(name)
            .bind(user_id)
            .bind(name)
            .execute(pool)
            .await?;
    }
    let row = sqlx::query("SELECT id,display_name FROM users WHERE id=?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(CurrentUser {
        id: row.get(0),
        display_name: row.get(1),
    })
}

/// Agent authentication. One bearer-token space, two kinds of token: a Ticket's
/// reporter token and a project's resolver key. The token itself is never
/// stored — only its SHA-256 — so a database leak cannot mint agent calls.
pub async fn require_agent(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match bearer(request.headers()) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    if !token.starts_with("tk_") {
        return AppError::unauthorized("无效的 Agent 令牌").into_response();
    }
    let hash = hash_token(&token);
    match resolve_agent(&state, &hash).await {
        Ok(identity) => {
            request.extensions_mut().insert(identity);
            next.run(request).await
        }
        Err(error) => error.into_response(),
    }
}

async fn resolve_agent(state: &AppState, hash: &str) -> AppResult<AgentIdentity> {
    let ticket = sqlx::query(
        "SELECT id,reporter_user_id,number FROM tickets WHERE reporter_agent_token_hash=?",
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;
    if let Some(row) = ticket {
        let number: i64 = row.get(2);
        return Ok(AgentIdentity {
            scope: AgentScope::Reporter {
                ticket_id: row.get(0),
                reporter_user_id: row.get(1),
            },
            label: format!("报障 Agent · #{number}"),
        });
    }
    let project = sqlx::query("SELECT id,name FROM projects WHERE resolver_key_hash=?")
        .bind(hash)
        .fetch_optional(&state.db)
        .await?;
    if let Some(row) = project {
        let name: String = row.get(1);
        return Ok(AgentIdentity {
            scope: AgentScope::Resolver {
                project_id: row.get(0),
            },
            label: format!("修复 Agent · {name}"),
        });
    }
    Err(AppError::unauthorized(
        "Agent 令牌无效或已被轮换。请在工单页重新获取。",
    ))
}

/// Auth Mini's own principal type is only used for the type-level check above;
/// keeping this function forces the dependency to stay wired to the crate's
/// public API.
#[allow(dead_code)]
fn _assert_principal_type(_: AuthMiniPrincipal) {}
