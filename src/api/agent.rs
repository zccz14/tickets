//! The Agent API: read, evidence, resolve.
//!
//! Both token kinds are bound to exactly one Ticket, so authorization is a
//! comparison rather than a query. There is no queue to poll and no comment
//! channel: the reporter Agent writes what it measured, the resolver Agent
//! writes what it did, and the Ticket is closed by that write.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::{Value, json};

use crate::{
    AppState,
    api::tickets::{self, TicketRow},
    auth::{AgentIdentity, AgentScope},
    crypto::now,
    error::{AppError, AppResult},
    model::{AgentEvidence, AgentResolve},
};

async fn authorize(agent: &AgentIdentity, ticket: &TicketRow) -> AppResult<()> {
    let token_ticket = match &agent.scope {
        AgentScope::Reporter { ticket_id, .. } | AgentScope::Resolver { ticket_id } => ticket_id,
    };
    if token_ticket != &ticket.id {
        return Err(AppError::forbidden("该令牌只对创建它的工单有效"));
    }
    if matches!(agent.scope, AgentScope::Resolver { .. }) && ticket.status == "draft" {
        return Err(AppError::conflict("草稿工单还没有提交"));
    }
    Ok(())
}

pub async fn whoami(Extension(agent): Extension<AgentIdentity>) -> Json<Value> {
    Json(match &agent.scope {
        AgentScope::Reporter {
            ticket_id,
            reporter_user_id,
        } => json!({
            "role": "reporter",
            "label": agent.label,
            "ticket_id": ticket_id,
            "user_id": reporter_user_id,
        }),
        AgentScope::Resolver { ticket_id } => json!({
            "role": "resolver",
            "label": agent.label,
            "ticket_id": ticket_id,
        }),
    })
}

pub async fn read(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&agent, &ticket).await?;
    Ok(Json(tickets::agent_view(&state, &id).await?))
}

/// The reporter Agent's only write: what it found on the reporter's machine.
/// Re-writing replaces the previous report, so the Ticket always carries the
/// latest measurement rather than a pile of them.
pub async fn evidence(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
    Json(input): Json<AgentEvidence>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&agent, &ticket).await?;
    if !matches!(agent.scope, AgentScope::Reporter { .. }) {
        return Err(AppError::forbidden("只有报障 Agent 可以写入现场证据"));
    }
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("现场证据不能为空"));
    }
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("现场证据不能超过 20000 个字符"));
    }
    let timestamp = now();
    sqlx::query("UPDATE tickets SET evidence=?, evidence_at=?, updated_at=? WHERE id=?")
        .bind(body)
        .bind(timestamp)
        .bind(timestamp)
        .bind(&ticket.id)
        .execute(&state.db)
        .await?;

    // New evidence on a Ticket the resolver already owns is worth a push: the
    // Thread keeps its history, so it only needs to know something changed.
    if ticket.status == "open" && ticket.resolver_thread_id.is_some() {
        let note = crate::prompt::resolver_evidence_note(ticket.number, body);
        crate::spawn_resolver_notice(state.clone(), ticket.id.clone(), note);
    }
    Ok(Json(json!({ "evidence_at": timestamp })))
}

/// The resolver Agent's only write: its conclusion, in the same call that
/// closes the Ticket. Requiring the text is deliberate — a Ticket is closed
/// with a reason, never silently.
pub async fn resolve(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
    Json(input): Json<AgentResolve>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&agent, &ticket).await?;
    if !matches!(agent.scope, AgentScope::Resolver { .. }) {
        return Err(AppError::forbidden("只有修复 Agent 可以给出处理结论"));
    }
    if ticket.status == "closed" {
        return Ok(Json(json!({ "closed": true, "already": true })));
    }
    let resolution = input.resolution.trim();
    if resolution.is_empty() {
        return Err(AppError::bad_request("处理结论不能为空"));
    }
    if resolution.chars().count() > 20_000 {
        return Err(AppError::bad_request("处理结论不能超过 20000 个字符"));
    }
    let timestamp = now();
    sqlx::query(
        "UPDATE tickets SET status='closed', closed_at=?, closed_by_kind='resolver_agent', closed_by_user_id=NULL, resolution=?, resolution_at=?, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(resolution)
    .bind(timestamp)
    .bind(timestamp)
    .bind(&ticket.id)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "closed": true })))
}
