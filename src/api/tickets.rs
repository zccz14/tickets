use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    api::{account, projects},
    auth::CurrentUser,
    crypto::{hash_token, new_id, new_token, now, token_hint},
    error::{AppError, AppResult},
    model::{
        AgentEndpoint, CreateTicket, ThreadInput, ThreadView, TicketSummary, TicketView,
        UpdateTicket,
    },
};

#[derive(Clone)]
pub struct TicketRow {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub reporter_user_id: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub submitted_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub closed_by_kind: Option<String>,
    pub closed_by_user_id: Option<String>,
    pub evidence: Option<String>,
    pub evidence_at: Option<i64>,
    pub resolution: Option<String>,
    pub resolution_at: Option<i64>,
    pub reporter_thread_id: Option<String>,
    pub reporter_thread_owner: Option<String>,
    pub reporter_thread_error: Option<String>,
    pub resolver_thread_id: Option<String>,
    pub resolver_thread_owner: Option<String>,
    pub resolver_thread_error: Option<String>,
    pub resolver_dispatched_at: Option<i64>,
}

const TICKET_COLUMNS: &str = "SELECT id,project_id,number,reporter_user_id,title,body,status,created_at,updated_at,submitted_at,closed_at,closed_by_kind,closed_by_user_id,evidence,evidence_at,resolution,resolution_at,reporter_thread_id,reporter_thread_owner,reporter_thread_error,resolver_thread_id,resolver_thread_owner,resolver_thread_error,resolver_dispatched_at FROM tickets";

impl TicketRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get(0),
            project_id: row.get(1),
            number: row.get(2),
            reporter_user_id: row.get(3),
            title: row.get(4),
            body: row.get(5),
            status: row.get(6),
            created_at: row.get(7),
            updated_at: row.get(8),
            submitted_at: row.get(9),
            closed_at: row.get(10),
            closed_by_kind: row.get(11),
            closed_by_user_id: row.get(12),
            evidence: row.get(13),
            evidence_at: row.get(14),
            resolution: row.get(15),
            resolution_at: row.get(16),
            reporter_thread_id: row.get(17),
            reporter_thread_owner: row.get(18),
            reporter_thread_error: row.get(19),
            resolver_thread_id: row.get(20),
            resolver_thread_owner: row.get(21),
            resolver_thread_error: row.get(22),
            resolver_dispatched_at: row.get(23),
        }
    }
}

pub async fn load_ticket(state: &AppState, id: &str) -> AppResult<TicketRow> {
    let row = sqlx::query(&format!("{TICKET_COLUMNS} WHERE id=?"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("工单不存在"))?;
    Ok(TicketRow::from_row(&row))
}

pub async fn display_name(state: &AppState, user_id: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar::<_, Option<String>>("SELECT display_name FROM users WHERE id=?")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .flatten(),
    )
}

#[derive(Deserialize)]
pub struct StatusFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
}

fn summary_from_row(row: &sqlx::sqlite::SqliteRow) -> TicketSummary {
    TicketSummary {
        id: row.get(0),
        project_id: row.get(1),
        project_name: row.get(2),
        number: row.get(3),
        title: row.get(4),
        status: row.get(5),
        reporter_user_id: row.get(6),
        reporter_name: row.get(7),
        created_at: row.get(8),
        updated_at: row.get(9),
        has_evidence: row.get::<i64, _>(10) != 0,
        has_resolution: row.get::<i64, _>(11) != 0,
        thread_id: row.get(12),
    }
}

const SUMMARY_SELECT: &str = "
SELECT t.id,t.project_id,p.name,t.number,t.title,t.status,t.reporter_user_id,u.display_name,
       t.created_at,t.updated_at,
       (t.evidence IS NOT NULL AND t.evidence<>'') AS has_evidence,
       (t.resolution IS NOT NULL AND t.resolution<>'') AS has_resolution,
       t.reporter_thread_id
FROM tickets t
JOIN projects p ON p.id=t.project_id
LEFT JOIN users u ON u.id=t.reporter_user_id
";

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(project_id): Path<String>,
    Query(filter): Query<StatusFilter>,
) -> AppResult<Json<Vec<TicketSummary>>> {
    let project = projects::load_project(&state, &project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    let status = filter.status.unwrap_or_else(|| "open".to_owned());
    let mut sql = String::from(SUMMARY_SELECT);
    let mut args: Vec<String> = vec![project_id.clone()];
    match status.as_str() {
        "all" | "" => {
            sql.push_str(" WHERE t.project_id=? AND (t.status<>'draft' OR t.reporter_user_id=?)");
            args.push(user.id.clone());
        }
        "draft" => {
            sql.push_str(" WHERE t.project_id=? AND t.status='draft' AND t.reporter_user_id=?");
            args.push(user.id.clone());
        }
        "open" | "closed" => {
            sql.push_str(" WHERE t.project_id=? AND t.status=?");
            args.push(status.clone());
        }
        other => return Err(AppError::bad_request(format!("未知状态: {other}"))),
    }
    sql.push_str(" ORDER BY t.updated_at DESC LIMIT 200");
    let mut query = sqlx::query(&sql);
    for arg in &args {
        query = query.bind(arg);
    }
    let rows = query.fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(summary_from_row).collect()))
}

pub async fn my_tickets(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Vec<TicketSummary>>> {
    let rows = sqlx::query(&format!(
        "{SUMMARY_SELECT}
         WHERE t.reporter_user_id=?
           AND (p.visibility='public' OR p.owner_user_id=?
                OR EXISTS(SELECT 1 FROM project_grants g WHERE g.project_id=p.id AND g.user_id=?))
         ORDER BY t.updated_at DESC LIMIT 200"
    ))
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.iter().map(summary_from_row).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(project_id): Path<String>,
    Json(input): Json<CreateTicket>,
) -> AppResult<Json<TicketView>> {
    let project = projects::load_project(&state, &project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    let body = input.body.trim();
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("工单正文不能超过 20000 个字符"));
    }
    // A conversation has no title field: the first message names the Ticket.
    // Agents that already have a title still send one and keep it verbatim.
    let title = match input.title.trim() {
        "" => crate::model::title_from_body(body),
        title => title.to_owned(),
    };
    if title.is_empty() {
        return Err(AppError::bad_request("工单标题不能为空"));
    }
    if title.chars().count() > 200 {
        return Err(AppError::bad_request("工单标题不能超过 200 个字符"));
    }
    let status = if input.submit { "open" } else { "draft" };
    let timestamp = now();
    let id = new_id();

    let mut tx = state.db.begin().await?;
    let number: i64 = sqlx::query_scalar(
        "UPDATE projects SET ticket_counter=ticket_counter+1, updated_at=? WHERE id=? RETURNING ticket_counter",
    )
    .bind(timestamp)
    .bind(&project.id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO tickets(id,project_id,number,reporter_user_id,title,body,status,created_at,updated_at,submitted_at)
         VALUES(?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&project.id)
    .bind(number)
    .bind(&user.id)
    .bind(title)
    .bind(body)
    .bind(status)
    .bind(timestamp)
    .bind(timestamp)
    .bind(if input.submit { Some(timestamp) } else { None })
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    if input.submit {
        crate::spawn_thread_creation(state.clone(), id.clone());
        crate::spawn_resolver_dispatch(state.clone(), id.clone());
    }
    let ticket = load_ticket(&state, &id).await?;
    read(State(state), Extension(user), Path(ticket.id)).await
}

pub async fn read(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    let reported_by_name = display_name(&state, &ticket.reporter_user_id).await?;
    let closed_by_name = match (
        ticket.closed_by_kind.as_deref(),
        ticket.closed_by_user_id.as_deref(),
    ) {
        (Some("user"), Some(uid)) => display_name(&state, uid).await?,
        (Some(kind), _) => Some(crate::model::closer_label(kind)),
        _ => None,
    };
    let is_reporter = ticket.reporter_user_id == user.id;
    let is_project_owner = project.owner_user_id == user.id;
    let reporter_thread = ThreadView {
        thread_id: ticket.reporter_thread_id.clone(),
        owner_user_id: ticket.reporter_thread_owner.clone(),
        title: None,
        status: None,
        display_status: None,
        context_tokens: None,
        url: ticket
            .reporter_thread_id
            .as_ref()
            .filter(|_| is_reporter)
            .map(|thread_id| state.cybion.thread_url(thread_id)),
        error: ticket.reporter_thread_error.clone(),
    };
    let resolver_thread = ThreadView {
        thread_id: ticket.resolver_thread_id.clone(),
        owner_user_id: ticket.resolver_thread_owner.clone(),
        title: None,
        status: None,
        display_status: None,
        context_tokens: None,
        url: ticket
            .resolver_thread_id
            .as_ref()
            .filter(|_| is_project_owner)
            .map(|thread_id| state.cybion.thread_url(thread_id)),
        error: ticket.resolver_thread_error.clone(),
    };
    Ok(Json(TicketView {
        agent: AgentEndpoint {
            ticket_url: format!("{}/api/agent/tickets/{}", state.public_base_url, ticket.id),
            evidence_url: format!(
                "{}/api/agent/tickets/{}/evidence",
                state.public_base_url, ticket.id
            ),
            resolve_url: format!(
                "{}/api/agent/tickets/{}/resolve",
                state.public_base_url, ticket.id
            ),
        },
        id: ticket.id.clone(),
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        number: ticket.number,
        title: ticket.title.clone(),
        body: ticket.body.clone(),
        status: ticket.status.clone(),
        reporter_user_id: ticket.reporter_user_id.clone(),
        reporter_name: reported_by_name,
        created_at: ticket.created_at,
        updated_at: ticket.updated_at,
        submitted_at: ticket.submitted_at,
        closed_at: ticket.closed_at,
        closed_by_kind: ticket.closed_by_kind.clone(),
        closed_by_name,
        evidence: ticket.evidence.clone(),
        evidence_at: ticket.evidence_at,
        resolution: ticket.resolution.clone(),
        resolution_at: ticket.resolution_at,
        is_reporter,
        is_project_owner,
        reporter_thread,
        resolver_thread,
    }))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTicket>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以修改工单内容"));
    }
    if ticket.status == "closed" {
        return Err(AppError::conflict("已关闭的工单不能修改，请先重新打开"));
    }
    if let Some(title) = input.title.as_deref() {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::bad_request("工单标题不能为空"));
        }
        if title.chars().count() > 200 {
            return Err(AppError::bad_request("工单标题不能超过 200 个字符"));
        }
        sqlx::query("UPDATE tickets SET title=? WHERE id=?")
            .bind(title)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(body) = input.body.as_deref() {
        if body.chars().count() > 20_000 {
            return Err(AppError::bad_request("工单正文不能超过 20000 个字符"));
        }
        sqlx::query("UPDATE tickets SET body=? WHERE id=?")
            .bind(body.trim())
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    sqlx::query("UPDATE tickets SET updated_at=? WHERE id=?")
        .bind(now())
        .bind(&id)
        .execute(&state.db)
        .await?;
    read(State(state), Extension(user), Path(id)).await
}

/// Submitting is the hand-off: the Ticket enters the resolver's world, and the
/// push happens right after the response (a slow Cybion must not hold it up).
pub async fn submit(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以提交工单"));
    }
    if ticket.status != "draft" {
        return Err(AppError::conflict("只有草稿可以提交"));
    }
    let timestamp = now();
    sqlx::query("UPDATE tickets SET status='open', submitted_at=?, updated_at=? WHERE id=?")
        .bind(timestamp)
        .bind(timestamp)
        .bind(&id)
        .execute(&state.db)
        .await?;
    if ticket.reporter_thread_id.is_none() {
        crate::spawn_thread_creation(state.clone(), ticket.id.clone());
    }
    crate::spawn_resolver_dispatch(state.clone(), ticket.id.clone());
    read(State(state), Extension(user), Path(id)).await
}

pub async fn close(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    let allowed = ticket.reporter_user_id == user.id || project.owner_user_id == user.id;
    if !allowed {
        return Err(AppError::forbidden("只有报障人或项目所有者可以关闭工单"));
    }
    close_ticket(&state, &ticket, "user", Some(&user.id)).await?;
    read(State(state), Extension(user), Path(id)).await
}

pub async fn close_ticket(
    state: &AppState,
    ticket: &TicketRow,
    kind: &str,
    user_id: Option<&str>,
) -> AppResult<()> {
    if ticket.status == "closed" {
        return Ok(());
    }
    let timestamp = now();
    sqlx::query(
        "UPDATE tickets SET status='closed', closed_at=?, closed_by_kind=?, closed_by_user_id=?, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(kind)
    .bind(user_id)
    .bind(timestamp)
    .bind(&ticket.id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Reopening puts the Ticket back in front of the same resolver Thread: its
/// history is the context, so it only needs to be told what changed. A Ticket
/// that never got a Thread (the push failed) is dispatched from scratch.
pub async fn reopen(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    let allowed = ticket.reporter_user_id == user.id || project.owner_user_id == user.id;
    if !allowed {
        return Err(AppError::forbidden(
            "只有报障人或项目所有者可以重新打开工单",
        ));
    }
    if ticket.status == "draft" {
        return Err(AppError::conflict("草稿请先提交"));
    }
    let timestamp = now();
    sqlx::query(
        "UPDATE tickets SET status='open', closed_at=NULL, closed_by_kind=NULL, closed_by_user_id=NULL, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(&id)
    .execute(&state.db)
    .await?;
    if ticket.resolver_thread_id.is_some() && ticket.resolver_thread_error.is_none() {
        let note = crate::prompt::resolver_reopen_note(
            ticket.number,
            ticket.resolution.as_deref().unwrap_or_default(),
        );
        crate::spawn_resolver_notice(state.clone(), ticket.id.clone(), note);
    } else {
        crate::spawn_resolver_dispatch(state.clone(), ticket.id.clone());
    }
    read(State(state), Extension(user), Path(id)).await
}

/// Lets the project owner retry the push after fixing the reason it failed
/// (no Worker selected, Cybion unreachable, key revoked).
pub async fn dispatch(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    // A draft is nobody else's business: it answers 404 before ownership does.
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以重新派发工单"));
    }
    if ticket.status == "draft" {
        return Err(AppError::conflict("草稿还没有提交，不能派发"));
    }
    if ticket.status == "closed" {
        return Err(AppError::conflict("已关闭的工单不能派发"));
    }
    dispatch_resolver(&state, &ticket.id).await?;
    read(State(state), Extension(user), Path(id)).await
}

/// Turns a Thread title into the Ticket title. Threads created by Tickets used
/// to carry a `[#12] ` prefix; strip it so the Ticket keeps the plain name the
/// reporter sees in Cybion.
fn title_from_thread(title: &str, number: i64) -> String {
    let title = title.trim();
    let prefix = format!("[#{number}] ");
    let title = title.strip_prefix(&prefix).unwrap_or(title).trim();
    title.chars().take(200).collect()
}

pub async fn read_thread(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<ThreadView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    let is_reporter = ticket.reporter_user_id == user.id;
    let Some(thread_id) = ticket.reporter_thread_id.clone() else {
        return Ok(Json(ThreadView {
            thread_id: None,
            owner_user_id: ticket.reporter_thread_owner.clone(),
            title: None,
            status: None,
            display_status: None,
            context_tokens: None,
            url: None,
            error: ticket.reporter_thread_error.clone(),
        }));
    };
    let live = match account::key_for(&state, &ticket.reporter_user_id).await? {
        Some(key) => state.cybion.read_thread(&key, &thread_id).await,
        None => Err("报障人还没有连接 Cybion 账号".to_owned()),
    };
    let (title, status, display_status, context_tokens, error) = match live {
        Ok(info) => {
            // The Thread names the Ticket. A rename in Cybion should reach the
            // Ticket without a second edit here.
            let synced = title_from_thread(&info.title, ticket.number);
            if !synced.is_empty() && synced != "Untitled thread" && synced != ticket.title {
                sqlx::query("UPDATE tickets SET title=? WHERE id=?")
                    .bind(&synced)
                    .bind(&ticket.id)
                    .execute(&state.db)
                    .await?;
            }
            (
                Some(info.title),
                Some(info.status),
                Some(info.display_status),
                info.context_tokens,
                None,
            )
        }
        Err(error) => (None, None, None, None, Some(error)),
    };
    Ok(Json(ThreadView {
        thread_id: Some(thread_id.clone()),
        owner_user_id: ticket.reporter_thread_owner.clone(),
        title,
        status,
        display_status,
        context_tokens,
        url: is_reporter.then(|| state.cybion.thread_url(&thread_id)),
        error,
    }))
}

pub async fn retry_thread(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<ThreadView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以创建现场调查 Thread"));
    }
    if ticket.reporter_thread_id.is_some() {
        return Err(AppError::conflict("该工单已经有 Cybion Thread"));
    }
    ensure_reporter_thread(&state, &ticket.id).await?;
    read_thread(State(state), Extension(user), Path(id)).await
}

/// The ticket page polls this with the newest history id it has seen, so the
/// Thread is read incrementally and a long investigation never re-transfers
/// old evidence.
#[derive(Deserialize)]
pub struct LiveThreadQuery {
    #[serde(default)]
    pub after: i64,
}

fn browser_token(user: &CurrentUser) -> AppResult<&str> {
    user.token
        .as_deref()
        .ok_or_else(|| AppError::unauthorized("缺少 Auth Mini 访问令牌，请重新登录"))
}

/// The Thread as Tickets knows it, without a second Cybion round-trip: the
/// sidebar query fills in live status separately.
fn live_thread_view(state: &AppState, ticket: &TicketRow) -> ThreadView {
    ThreadView {
        thread_id: ticket.reporter_thread_id.clone(),
        owner_user_id: ticket.reporter_thread_owner.clone(),
        title: None,
        status: None,
        display_status: None,
        context_tokens: None,
        url: ticket
            .reporter_thread_id
            .as_ref()
            .map(|thread_id| state.cybion.thread_url(thread_id)),
        error: ticket.reporter_thread_error.clone(),
    }
}

/// Reporting is the conversation: the reporter's own Thread is read through
/// Tickets, with that person's Auth Mini token. Only the reporter may see it —
/// the Thread runs on their machine and stays theirs.
pub async fn live_thread(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Query(query): Query<LiveThreadQuery>,
) -> AppResult<Json<Value>> {
    let mut ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以查看现场调查 Thread"));
    }
    // A draft has no Thread until its reporter looks at it. Starting the
    // investigation here is the product: no separate "create Thread" click.
    // A failed creation records its reason, so this never retries in a loop.
    if ticket.status == "draft"
        && ticket.reporter_thread_id.is_none()
        && ticket.reporter_thread_error.is_none()
    {
        let _ = ensure_reporter_thread(&state, &ticket.id).await;
        ticket = load_ticket(&state, &id).await?;
    }
    let thread = live_thread_view(&state, &ticket);
    let Some(thread_id) = ticket.reporter_thread_id.clone() else {
        return Ok(Json(json!({
            "thread": thread,
            "records": [],
            "response": null,
        })));
    };
    let token = browser_token(&user)?;
    let records = state
        .cybion
        .browser_history(token, &thread_id, query.after)
        .await
        .map_err(AppError::unavailable)?;
    // The in-flight reply is a preview, not the deliverable: a transient
    // failure here must not blank a conversation that history already has.
    let response = state
        .cybion
        .browser_response(token, &thread_id)
        .await
        .unwrap_or(None);
    Ok(Json(json!({
        "thread": thread,
        "records": records,
        "response": response,
    })))
}

/// A message the reporter types into the conversation. It goes straight into
/// the Thread with their token, so the Agent answers in the same turn.
pub async fn live_thread_input(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<ThreadInput>,
) -> AppResult<Json<Value>> {
    let mut ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    projects::require_view(&state, &project, &user.id).await?;
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以推进现场调查 Thread"));
    }
    if ticket.status == "closed" {
        return Err(AppError::conflict("工单已关闭，不能再推进现场调查 Thread"));
    }
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("消息不能为空"));
    }
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("消息不能超过 20000 个字符"));
    }
    if ticket.reporter_thread_id.is_none() {
        ensure_reporter_thread(&state, &ticket.id).await?;
        ticket = load_ticket(&state, &id).await?;
    }
    let Some(thread_id) = ticket.reporter_thread_id.clone() else {
        return Err(AppError::conflict(
            "现场调查 Thread 还没有创建完成，请稍后重试",
        ));
    };
    let token = browser_token(&user)?;
    state
        .cybion
        .browser_input(token, &thread_id, body)
        .await
        .map_err(AppError::unavailable)?;
    // A successful turn proves the Thread is healthy again.
    set_reporter_error(&state, &ticket.id, None).await?;
    Ok(Json(json!({ "accepted": true })))
}

/// Creates the reporter's Cybion Thread and hands it the investigation
/// briefing. Safe to retry: it records failures on the Ticket instead of
/// failing the caller's request.
pub async fn ensure_reporter_thread(state: &AppState, ticket_id: &str) -> AppResult<()> {
    let ticket = load_ticket(state, ticket_id).await?;
    if ticket.reporter_thread_id.is_some() {
        return Ok(());
    }
    let project = projects::load_project(state, &ticket.project_id).await?;
    let Some(key) = account::key_for(state, &ticket.reporter_user_id).await? else {
        set_reporter_error(
            state,
            ticket_id,
            Some("报障人还没有连接 Cybion 账号，无法创建现场调查 Thread"),
        )
        .await?;
        return Ok(());
    };
    let reporter = display_name(state, &ticket.reporter_user_id)
        .await?
        .unwrap_or_else(|| {
            format!(
                "用户 {}",
                &ticket.reporter_user_id[..8.min(ticket.reporter_user_id.len())]
            )
        });
    let title = format!(
        "[#{number}] {title}",
        number = ticket.number,
        title = crate::model::excerpt(&ticket.title, 80)
    );
    let external_ref = format!("{}/#/tickets/{}", state.public_base_url, ticket.id);

    let thread = match state
        .cybion
        .create_thread(&key, &title, &external_ref)
        .await
    {
        Ok(thread) => thread,
        Err(error) => {
            set_reporter_error(state, ticket_id, Some(&error)).await?;
            return Err(AppError::unavailable(error));
        }
    };

    let token = new_token();
    sqlx::query(
        "UPDATE tickets SET reporter_thread_id=?, reporter_thread_owner=?, reporter_agent_token_hash=?, reporter_agent_token_hint=?, reporter_thread_error=NULL WHERE id=?",
    )
    .bind(&thread.id)
    .bind(&ticket.reporter_user_id)
    .bind(hash_token(&token))
    .bind(token_hint(&token))
    .bind(ticket_id)
    .execute(&state.db)
    .await?;

    let briefing = crate::prompt::reporter_briefing(&crate::prompt::ReporterBriefing {
        cybion_base: state.cybion.base().to_owned(),
        ticket_number: ticket.number,
        project_name: &project.name,
        project_description: &project.description,
        title: &ticket.title,
        reporter: &reporter,
        draft: ticket.status == "draft",
        api_base: &state.public_base_url,
        ticket_id: &ticket.id,
        token: &token,
    });
    // The briefing is folded context, not something the reporter typed: the
    // panel must show their own words, and the Ticket body already sits right
    // above it. The short opening line is what actually starts the run.
    if let Err(error) = state
        .cybion
        .append_context(&key, &thread.id, &briefing)
        .await
    {
        set_reporter_error(
            state,
            ticket_id,
            Some(&format!("Thread 已创建，但调查简报写入失败：{error}")),
        )
        .await?;
        return Ok(());
    }
    if let Err(error) = state
        .cybion
        .post_input(
            &key,
            &thread.id,
            crate::prompt::first_message(&ticket.body, &ticket.title),
        )
        .await
    {
        set_reporter_error(
            state,
            ticket_id,
            Some(&format!("Thread 已创建，但首条用户消息发送失败：{error}")),
        )
        .await?;
    }
    Ok(())
}

async fn set_reporter_error(
    state: &AppState,
    ticket_id: &str,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET reporter_thread_error=? WHERE id=?")
        .bind(error)
        .bind(ticket_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

async fn set_resolver_error(
    state: &AppState,
    ticket_id: &str,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET resolver_thread_error=? WHERE id=?")
        .bind(error)
        .bind(ticket_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Pushes a submitted Ticket to the project owner's resolver Agent: one fresh
/// Cybion Thread on the owner's account, opened with the full briefing. This is
/// the whole hand-off — no queue, no schedule, no machine.
///
/// Failures are recorded on the Ticket (`resolver_thread_error`) so the owner
/// can fix the cause and retry from the ticket page; the reporter's submit has
/// already succeeded by then.
pub async fn dispatch_resolver(state: &AppState, ticket_id: &str) -> AppResult<()> {
    let ticket = load_ticket(state, ticket_id).await?;
    if ticket.status != "open" {
        return Ok(());
    }
    let project = projects::load_project(state, &ticket.project_id).await?;
    // The only prerequisite: the owner's own Cybion account. No Worker to pick,
    // no schedule to tune — the Thread uses whatever machine that account has,
    // exactly like a Thread created from the Cybion UI.
    let Some(key) = account::key_for(state, &project.owner_user_id).await? else {
        set_resolver_error(
            state,
            ticket_id,
            Some("项目所有者还没有连接 Cybion 账号，工单没有派发出去"),
        )
        .await?;
        return Ok(());
    };
    let owner = display_name(state, &project.owner_user_id)
        .await?
        .unwrap_or_else(|| "项目所有者".to_owned());
    let reporter = display_name(state, &ticket.reporter_user_id)
        .await?
        .unwrap_or_else(|| {
            format!(
                "用户 {}",
                &ticket.reporter_user_id[..8.min(ticket.reporter_user_id.len())]
            )
        });
    let ticket_url = format!("{}/#/tickets/{}", state.public_base_url, ticket.id);

    // One Thread per Ticket: a previous attempt may have created it and failed
    // to brief it, in which case the same Thread is reused and its token is
    // rotated. A Thread that is already briefed is left alone.
    let token = new_token();
    let thread_id = match ticket.resolver_thread_id.clone() {
        Some(thread_id) if ticket.resolver_thread_error.is_some() => {
            sqlx::query(
                "UPDATE tickets SET resolver_agent_token_hash=?, resolver_agent_token_hint=?, resolver_dispatched_at=? WHERE id=?",
            )
            .bind(hash_token(&token))
            .bind(token_hint(&token))
            .bind(now())
            .bind(ticket_id)
            .execute(&state.db)
            .await?;
            thread_id
        }
        Some(_) => return Ok(()),
        None => {
            let title = format!(
                "[#{number}] {title}",
                number = ticket.number,
                title = crate::model::excerpt(&ticket.title, 80)
            );
            let created = match state.cybion.create_thread(&key, &title, &ticket_url).await {
                Ok(thread) => thread,
                Err(error) => {
                    set_resolver_error(state, ticket_id, Some(&error)).await?;
                    return Err(AppError::unavailable(error));
                }
            };
            sqlx::query(
                "UPDATE tickets SET resolver_thread_id=?, resolver_thread_owner=?, resolver_agent_token_hash=?, resolver_agent_token_hint=?, resolver_dispatched_at=? WHERE id=?",
            )
            .bind(&created.id)
            .bind(&project.owner_user_id)
            .bind(hash_token(&token))
            .bind(token_hint(&token))
            .bind(now())
            .bind(ticket_id)
            .execute(&state.db)
            .await?;
            created.id
        }
    };

    let briefing = crate::prompt::resolver_briefing(&crate::prompt::ResolverBriefing {
        ticket_number: ticket.number,
        project_name: &project.name,
        project_description: &project.description,
        owner: &owner,
        title: &ticket.title,
        reporter: &reporter,
        evidence: ticket.evidence.as_deref().unwrap_or_default(),
        api_base: &state.public_base_url,
        ticket_id: &ticket.id,
        ticket_url: &ticket_url,
        token: &token,
    });
    // Same split as the reporter side: the dispatch itself is folded context,
    // and one short owner line starts the run.
    if let Err(error) = state
        .cybion
        .append_context(&key, &thread_id, &briefing)
        .await
    {
        set_resolver_error(
            state,
            ticket_id,
            Some(&format!("修复 Thread 已就绪，但派单简报写入失败：{error}")),
        )
        .await?;
        return Err(AppError::unavailable(error));
    }
    match state
        .cybion
        .post_input(
            &key,
            &thread_id,
            crate::prompt::first_message(&ticket.body, &ticket.title),
        )
        .await
    {
        Ok(()) => {
            set_resolver_error(state, ticket_id, None).await?;
            Ok(())
        }
        Err(error) => {
            set_resolver_error(
                state,
                ticket_id,
                Some(&format!(
                    "修复 Thread 已就绪，但首条用户消息发送失败：{error}"
                )),
            )
            .await?;
            Err(AppError::unavailable(error))
        }
    }
}

/// Sends one more message into the resolver Thread Tickets already opened — on
/// a reopen, or when new evidence lands. The Thread keeps its history, so this
/// is a nudge rather than a new assignment.
pub async fn notify_resolver(state: &AppState, ticket_id: &str, note: &str) -> AppResult<()> {
    let ticket = load_ticket(state, ticket_id).await?;
    let Some(thread_id) = ticket.resolver_thread_id.clone() else {
        return Ok(());
    };
    let project = projects::load_project(state, &ticket.project_id).await?;
    let Some(key) = account::key_for(state, &project.owner_user_id).await? else {
        set_resolver_error(
            state,
            ticket_id,
            Some("项目所有者还没有连接 Cybion 账号，无法推进修复 Thread"),
        )
        .await?;
        return Ok(());
    };
    match state.cybion.post_input(&key, &thread_id, note).await {
        Ok(()) => {
            sqlx::query("UPDATE tickets SET resolver_thread_error=NULL WHERE id=?")
                .bind(ticket_id)
                .execute(&state.db)
                .await?;
            Ok(())
        }
        Err(error) => {
            set_resolver_error(
                state,
                ticket_id,
                Some(&format!("推进修复 Thread 失败：{error}")),
            )
            .await?;
            Err(AppError::unavailable(error))
        }
    }
}

/// Shared by the browser UI and the agent API so both sides describe a Ticket
/// the same way.
pub async fn agent_view(state: &AppState, ticket_id: &str) -> AppResult<Value> {
    let ticket = load_ticket(state, ticket_id).await?;
    let project = projects::load_project(state, &ticket.project_id).await?;
    Ok(json!({
        "id": ticket.id,
        "number": ticket.number,
        "title": ticket.title,
        "body": ticket.body,
        "status": ticket.status,
        "project": project.name,
        "reporter": display_name(state, &ticket.reporter_user_id).await?,
        "url": format!("{}/#/tickets/{}", state.public_base_url, ticket.id),
        "evidence": ticket.evidence,
        "evidence_at": ticket.evidence_at,
        "resolution": ticket.resolution,
        "resolution_at": ticket.resolution_at,
        "created_at": ticket.created_at,
        "updated_at": ticket.updated_at,
    }))
}
