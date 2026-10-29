//! The resolver side, managed as a Cybion Ultimate Machine.
//!
//! The project owner never pastes a key or a command: Tickets mints the
//! resolver key, builds the check command, and asks Cybion to create the
//! machine on a Worker the owner picks. Cybion keeps the schedule; this module
//! keeps the link between the project and that machine.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    api::projects::{ProjectRow, load_project},
    auth::CurrentUser,
    crypto::{hash_token, new_token, now, token_hint},
    cybion::NewMachine,
    error::{AppError, AppResult},
};

const DEFAULT_INTERVAL_SECONDS: i64 = 120;
const MIN_INTERVAL_SECONDS: i64 = 10;
const MAX_INTERVAL_SECONDS: i64 = 30 * 24 * 60 * 60;
const MACHINE_NAME_MAX_CHARS: usize = 120;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectMachine {
    pub worker_id: String,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateMachine {
    pub enabled: bool,
}

fn browser_token(user: &CurrentUser) -> AppResult<&str> {
    user.token
        .as_deref()
        .ok_or_else(|| AppError::unauthorized("缺少 Auth Mini 访问令牌"))
}

async fn owned_project(state: &AppState, user: &CurrentUser, id: &str) -> AppResult<ProjectRow> {
    let project = load_project(state, id).await?;
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以管理修复 Agent"));
    }
    Ok(project)
}

fn machine_name(project: &str) -> String {
    let prefix = "工单修复 · ";
    let room = MACHINE_NAME_MAX_CHARS - prefix.chars().count();
    let trimmed: String = project.chars().take(room).collect();
    format!("{prefix}{trimmed}")
}

/// The Workers this person can run the machine on.
pub async fn workers(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Value>> {
    let token = browser_token(&user)?;
    let workers = state
        .cybion
        .list_workers(token)
        .await
        .map_err(AppError::unavailable)?;
    Ok(Json(json!({ "workers": workers })))
}

/// The machine watching this project, read live from Cybion.
pub async fn read(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let project = owned_project(&state, &user, &id).await?;
    let Some(machine_id) = project.resolver_machine_id.clone() else {
        return Ok(Json(json!({ "machine": null })));
    };
    let token = browser_token(&user)?;
    let machine = state
        .cybion
        .read_machine(token, &machine_id)
        .await
        .map_err(AppError::unavailable)?;
    if machine.is_none() {
        sqlx::query("UPDATE projects SET resolver_machine_id=NULL WHERE id=?")
            .bind(&project.id)
            .execute(&state.db)
            .await?;
    }
    Ok(Json(json!({ "machine": machine })))
}

/// Creates the resolver key and the Ultimate Machine in one step. A failed
/// machine leaves no half-connected key behind.
pub async fn connect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<ConnectMachine>,
) -> AppResult<Json<Value>> {
    let project = owned_project(&state, &user, &id).await?;
    let worker_id = input.worker_id.trim();
    if worker_id.is_empty() || worker_id.chars().count() > 128 {
        return Err(AppError::bad_request("请选择一个 Cybion Worker"));
    }
    let interval = input.interval_seconds.unwrap_or(DEFAULT_INTERVAL_SECONDS);
    if !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&interval) {
        return Err(AppError::bad_request("检查间隔需要在 10 秒到 30 天之间"));
    }
    if project.resolver_machine_id.is_some() {
        return Err(AppError::conflict("这个项目已经接入了终极机器"));
    }
    let token = browser_token(&user)?;

    let key = new_token();
    sqlx::query(
        "UPDATE projects SET resolver_key_hash=?, resolver_key_hint=?, resolver_key_created_at=?, updated_at=? WHERE id=?",
    )
    .bind(hash_token(&key))
    .bind(token_hint(&key))
    .bind(now())
    .bind(now())
    .bind(&project.id)
    .execute(&state.db)
    .await?;

    let command = crate::machine::resolver_command(&state.public_base_url, &key, &project.name);
    let name = machine_name(&project.name);
    let intent = crate::prompt::resolver_intent(&project.name);
    let created = state
        .cybion
        .create_machine(
            token,
            &NewMachine {
                name: &name,
                worker_id,
                command: &command,
                intent: Some(&intent),
                interval_seconds: interval,
            },
        )
        .await;

    let machine = match created {
        Ok(machine) => machine,
        Err(error) => {
            sqlx::query(
                "UPDATE projects SET resolver_key_hash=NULL, resolver_key_hint=NULL, resolver_key_created_at=NULL WHERE id=?",
            )
            .bind(&project.id)
            .execute(&state.db)
            .await?;
            return Err(AppError::unavailable(format!(
                "密钥已生成，但接入终极机器失败：{error}"
            )));
        }
    };

    let thread_url = if machine.thread_id.is_empty() {
        None
    } else {
        Some(state.cybion.thread_url(&machine.thread_id))
    };
    sqlx::query(
        "UPDATE projects SET resolver_machine_id=?, resolver_thread_url=COALESCE(?, resolver_thread_url), updated_at=? WHERE id=?",
    )
    .bind(&machine.id)
    .bind(thread_url.as_deref())
    .bind(now())
    .bind(&project.id)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "machine": machine })))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<UpdateMachine>,
) -> AppResult<Json<Value>> {
    let project = owned_project(&state, &user, &id).await?;
    let machine_id = project
        .resolver_machine_id
        .clone()
        .ok_or_else(|| AppError::conflict("这个项目还没有接入终极机器"))?;
    let token = browser_token(&user)?;
    let machine = state
        .cybion
        .update_machine(token, &machine_id, json!({ "enabled": input.enabled }))
        .await
        .map_err(AppError::unavailable)?;
    Ok(Json(json!({ "machine": machine })))
}

pub async fn run(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let project = owned_project(&state, &user, &id).await?;
    let machine_id = project
        .resolver_machine_id
        .clone()
        .ok_or_else(|| AppError::conflict("这个项目还没有接入终极机器"))?;
    let token = browser_token(&user)?;
    state
        .cybion
        .run_machine(token, &machine_id)
        .await
        .map_err(AppError::unavailable)?;
    Ok(Json(json!({ "started": true })))
}

/// Deletes the machine at Cybion and revokes the resolver key here, so nothing
/// keeps polling a project that is no longer connected.
pub async fn disconnect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let project = owned_project(&state, &user, &id).await?;
    let token = browser_token(&user)?;
    if let Some(machine_id) = project.resolver_machine_id.clone() {
        state
            .cybion
            .delete_machine(token, &machine_id)
            .await
            .map_err(AppError::unavailable)?;
    }
    sqlx::query(
        "UPDATE projects SET resolver_key_hash=NULL, resolver_key_hint=NULL, resolver_key_created_at=NULL, resolver_machine_id=NULL, updated_at=? WHERE id=?",
    )
    .bind(now())
    .bind(&project.id)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "disconnected": true })))
}
