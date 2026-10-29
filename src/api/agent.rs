use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    api::{projects, tickets},
    auth::{AgentIdentity, AgentScope},
    error::{AppError, AppResult},
    model::{AgentClose, AgentComment, QueueTicket, QueueView, excerpt},
};

/// One agent token, two scopes. Both are checked against the Ticket itself, so
/// a reporter token can only ever touch its own Ticket and a resolver key can
/// only ever touch its own project.
async fn authorize(
    state: &AppState,
    agent: &AgentIdentity,
    ticket: &tickets::TicketRow,
) -> AppResult<()> {
    match &agent.scope {
        AgentScope::Reporter { ticket_id, .. } => {
            if ticket_id != &ticket.id {
                return Err(AppError::forbidden("该令牌只对创建它的工单有效"));
            }
        }
        AgentScope::Resolver { project_id } => {
            if project_id != &ticket.project_id {
                return Err(AppError::forbidden("该令牌只对所属项目有效"));
            }
            if ticket.status == "draft" {
                return Err(AppError::conflict("草稿工单不在修复队列里"));
            }
        }
    }
    let _ = state;
    Ok(())
}

fn author_kind(agent: &AgentIdentity) -> &'static str {
    match agent.scope {
        AgentScope::Reporter { .. } => "reporter_agent",
        AgentScope::Resolver { .. } => "resolver_agent",
    }
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
        AgentScope::Resolver { project_id } => json!({
            "role": "resolver",
            "label": agent.label,
            "project_id": project_id,
        }),
    })
}

pub async fn queue(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
) -> AppResult<Json<QueueView>> {
    let AgentScope::Resolver { project_id } = &agent.scope else {
        return Err(AppError::forbidden("只有修复 Agent 可以读取队列"));
    };
    let project = projects::load_project(&state, project_id).await?;
    let total_open: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tickets WHERE project_id=? AND status='open'")
            .bind(project_id)
            .fetch_one(&state.db)
            .await?;
    let rows = sqlx::query(
        "SELECT t.id,t.number,t.title,t.body,t.updated_at,t.reporter_user_id,
                (SELECT COUNT(*) FROM comments c WHERE c.ticket_id=t.id AND c.kind<>'system') AS comments
         FROM tickets t
         WHERE t.project_id=? AND t.status='open' AND t.last_reporter_at > t.last_resolver_at
         ORDER BY t.last_reporter_at ASC
         LIMIT 5",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    let mut pending = Vec::with_capacity(rows.len());
    for row in &rows {
        let ticket_id: String = row.get(0);
        let evidence = sqlx::query_scalar::<_, String>(
            "SELECT body FROM comments WHERE ticket_id=? AND kind='evidence' ORDER BY created_at DESC, rowid DESC LIMIT 1",
        )
        .bind(&ticket_id)
        .fetch_optional(&state.db)
        .await?;
        let reporter_id: String = row.get(5);
        pending.push(QueueTicket {
            url: format!("{}/#/tickets/{}", state.public_base_url, ticket_id),
            summary: excerpt(&row.get::<String, _>(3), 600),
            evidence: evidence.map(|body| excerpt(&body, 600)),
            reporter: tickets::display_name(&state, &reporter_id)
                .await?
                .unwrap_or_else(|| reporter_id.chars().take(8).collect()),
            id: ticket_id,
            number: row.get(1),
            title: row.get(2),
            project: project.name.clone(),
            updated_at: row.get(4),
            comments: row.get(6),
        });
    }

    Ok(Json(QueueView {
        project: project.name,
        total_open,
        pending,
    }))
}

pub async fn read(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&state, &agent, &ticket).await?;
    Ok(Json(tickets::agent_view(&state, &id).await?))
}

pub async fn comment(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
    Json(input): Json<AgentComment>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&state, &agent, &ticket).await?;
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("评论不能为空"));
    }
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("评论不能超过 20000 个字符"));
    }
    let kind = match input.kind.as_deref() {
        Some("evidence") => {
            if !matches!(agent.scope, AgentScope::Reporter { .. }) {
                return Err(AppError::forbidden("只有报障 Agent 可以写入证据"));
            }
            "evidence"
        }
        Some("comment") | None => "comment",
        Some(other) => return Err(AppError::bad_request(format!("未知评论类型: {other}"))),
    };
    let id = tickets::insert_comment(
        &state,
        &ticket.id,
        kind,
        author_kind(&agent),
        None,
        Some(&agent.label),
        body,
    )
    .await?;
    Ok(Json(json!({ "id": id, "kind": kind })))
}

pub async fn close(
    State(state): State<AppState>,
    Extension(agent): Extension<AgentIdentity>,
    Path(id): Path<String>,
    Json(input): Json<AgentClose>,
) -> AppResult<Json<Value>> {
    let ticket = tickets::load_ticket(&state, &id).await?;
    authorize(&state, &agent, &ticket).await?;
    if ticket.status == "closed" {
        return Ok(Json(json!({ "closed": true, "already": true })));
    }
    if let Some(body) = input
        .body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
    {
        tickets::insert_comment(
            &state,
            &ticket.id,
            "comment",
            author_kind(&agent),
            None,
            Some(&agent.label),
            body,
        )
        .await?;
    }
    tickets::close_ticket(&state, &ticket, author_kind(&agent), None, &agent.label).await?;
    Ok(Json(json!({ "closed": true })))
}
