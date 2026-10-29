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
    crypto::{hash_token, new_id, new_token, now},
    error::{AppError, AppResult},
    model::{
        AgentEndpoint, CommentView, CreateComment, CreateTicket, ThreadView, TicketSummary,
        TicketView, UpdateTicket,
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
    pub last_reporter_at: i64,
    pub last_resolver_at: i64,
    pub reporter_thread_id: Option<String>,
    pub reporter_thread_owner: Option<String>,
    pub reporter_thread_error: Option<String>,
}

const TICKET_COLUMNS: &str = "SELECT id,project_id,number,reporter_user_id,title,body,status,created_at,updated_at,submitted_at,closed_at,closed_by_kind,closed_by_user_id,last_reporter_at,last_resolver_at,reporter_thread_id,reporter_thread_owner,reporter_thread_error FROM tickets";

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
            last_reporter_at: row.get(13),
            last_resolver_at: row.get(14),
            reporter_thread_id: row.get(15),
            reporter_thread_owner: row.get(16),
            reporter_thread_error: row.get(17),
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

fn author_label(author_kind: &str, fallback: Option<&str>) -> String {
    match author_kind {
        "reporter_agent" => "报障 Agent".to_owned(),
        "resolver_agent" => "修复 Agent".to_owned(),
        "system" => "系统".to_owned(),
        _ => fallback.unwrap_or("匿名用户").to_owned(),
    }
}

pub async fn comments(state: &AppState, ticket_id: &str) -> AppResult<Vec<CommentView>> {
    let rows = sqlx::query(
        "SELECT c.id,c.kind,c.author_kind,c.author_user_id,COALESCE(u.display_name,c.author_label) AS name,c.body,c.created_at
         FROM comments c LEFT JOIN users u ON u.id=c.author_user_id
         WHERE c.ticket_id=? ORDER BY c.created_at ASC, c.rowid ASC",
    )
    .bind(ticket_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .iter()
        .map(|row| CommentView {
            id: row.get(0),
            kind: row.get(1),
            author_kind: row.get(2),
            author_user_id: row.get(3),
            author_name: row.get(4),
            body: row.get(5),
            created_at: row.get(6),
        })
        .collect())
}

pub async fn insert_comment(
    state: &AppState,
    ticket_id: &str,
    kind: &str,
    author_kind: &str,
    author_user_id: Option<&str>,
    label: Option<&str>,
    body: &str,
) -> AppResult<String> {
    let id = new_id();
    let timestamp = now();
    sqlx::query(
        "INSERT INTO comments(id,ticket_id,kind,author_kind,author_user_id,author_label,body,created_at) VALUES(?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(ticket_id)
    .bind(kind)
    .bind(author_kind)
    .bind(author_user_id)
    .bind(label)
    .bind(body)
    .bind(timestamp)
    .execute(&state.db)
    .await?;
    let stamp_column = match author_kind {
        "resolver_agent" => "last_resolver_at",
        "reporter_agent" | "user" => "last_reporter_at",
        _ => "",
    };
    if stamp_column.is_empty() {
        sqlx::query("UPDATE tickets SET updated_at=? WHERE id=?")
            .bind(timestamp)
            .bind(ticket_id)
            .execute(&state.db)
            .await?;
    } else {
        sqlx::query(&format!(
            "UPDATE tickets SET {stamp_column}=?, updated_at=? WHERE id=?"
        ))
        .bind(timestamp)
        .bind(timestamp)
        .bind(ticket_id)
        .execute(&state.db)
        .await?;
    }
    Ok(id)
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
        comment_count: row.get(10),
        has_evidence: row.get::<i64, _>(11) != 0,
        thread_id: row.get(12),
    }
}

const SUMMARY_SELECT: &str = "
SELECT t.id,t.project_id,p.name,t.number,t.title,t.status,t.reporter_user_id,u.display_name,
       t.created_at,t.updated_at,
       (SELECT COUNT(*) FROM comments c WHERE c.ticket_id=t.id AND c.kind<>'system') AS comment_count,
       (SELECT EXISTS(SELECT 1 FROM comments c WHERE c.ticket_id=t.id AND c.kind='evidence')) AS has_evidence,
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
    projects::load_project(&state, &project_id).await?;
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
        "{SUMMARY_SELECT} WHERE t.reporter_user_id=? ORDER BY t.updated_at DESC LIMIT 200"
    ))
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
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("工单标题不能为空"));
    }
    if title.chars().count() > 200 {
        return Err(AppError::bad_request("工单标题不能超过 200 个字符"));
    }
    let body = input.body.trim();
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("工单正文不能超过 20000 个字符"));
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
        "INSERT INTO tickets(id,project_id,number,reporter_user_id,title,body,status,created_at,updated_at,submitted_at,last_reporter_at)
         VALUES(?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(timestamp)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    if input.submit {
        crate::spawn_thread_creation(state.clone(), id.clone());
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
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    let reported_by_name = display_name(&state, &ticket.reporter_user_id).await?;
    let closed_by_name = match (
        ticket.closed_by_kind.as_deref(),
        ticket.closed_by_user_id.as_deref(),
    ) {
        (Some("user"), Some(uid)) => display_name(&state, uid).await?,
        (Some(kind), _) => Some(author_label(kind, None)),
        _ => None,
    };
    let is_reporter = ticket.reporter_user_id == user.id;
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
    Ok(Json(TicketView {
        agent: AgentEndpoint {
            ticket_url: format!("{}/api/agent/tickets/{}", state.public_base_url, ticket.id),
            comments_url: format!(
                "{}/api/agent/tickets/{}/comments",
                state.public_base_url, ticket.id
            ),
            close_url: format!(
                "{}/api/agent/tickets/{}/close",
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
        is_reporter,
        is_project_owner: project.owner_user_id == user.id,
        comments: comments(&state, &ticket.id).await?,
        reporter_thread,
        resolver_thread_url: project.resolver_thread_url.clone(),
    }))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTicket>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
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

pub async fn comment(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<CreateComment>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    if ticket.status == "draft" && ticket.reporter_user_id != user.id {
        return Err(AppError::not_found("工单不存在"));
    }
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("评论不能为空"));
    }
    if body.chars().count() > 20_000 {
        return Err(AppError::bad_request("评论不能超过 20000 个字符"));
    }
    insert_comment(
        &state,
        &ticket.id,
        "comment",
        "user",
        Some(&user.id),
        user.display_name.as_deref(),
        body,
    )
    .await?;
    // The reporter's Thread owns the investigation, so every comment is a new
    // lead for it: wake it in the background instead of making the commenter
    // wait on Cybion.
    let project = projects::load_project(&state, &ticket.project_id).await?;
    let role = if ticket.reporter_user_id == user.id {
        "报障人"
    } else if project.owner_user_id == user.id {
        "项目所有者"
    } else {
        "其他成员"
    };
    let nudge = crate::prompt::comment_nudge(&crate::prompt::CommentNudge {
        ticket_number: ticket.number,
        author: user.display_name.as_deref().unwrap_or("匿名用户"),
        role,
        body,
        closed: ticket.status == "closed",
    });
    // Someone else's comment cannot conjure a Thread on the reporter's behalf:
    // only the reporter's own comment may create a missing one.
    if ticket.reporter_thread_id.is_some() || ticket.reporter_user_id == user.id {
        crate::spawn_reporter_progress(state.clone(), ticket.id.clone(), nudge);
    }
    read(State(state), Extension(user), Path(id)).await
}

pub async fn submit(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以提交工单"));
    }
    if ticket.status != "draft" {
        return Err(AppError::conflict("只有草稿可以提交"));
    }
    let timestamp = now();
    sqlx::query(
        "UPDATE tickets SET status='open', submitted_at=?, last_reporter_at=?, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(timestamp)
    .bind(timestamp)
    .bind(&id)
    .execute(&state.db)
    .await?;
    insert_comment(
        &state,
        &ticket.id,
        "system",
        "system",
        None,
        Some("报障人提交了工单"),
        "工单已提交，等待修复 Agent 处理。",
    )
    .await?;
    if ticket.reporter_thread_id.is_none() {
        crate::spawn_thread_creation(state.clone(), ticket.id.clone());
    }
    read(State(state), Extension(user), Path(id)).await
}

pub async fn close(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
    let allowed = ticket.reporter_user_id == user.id || project.owner_user_id == user.id;
    if !allowed {
        return Err(AppError::forbidden("只有报障人或项目所有者可以关闭工单"));
    }
    close_ticket(
        &state,
        &ticket,
        "user",
        Some(&user.id),
        user.display_name.as_deref().unwrap_or("用户"),
    )
    .await?;
    read(State(state), Extension(user), Path(id)).await
}

pub async fn close_ticket(
    state: &AppState,
    ticket: &TicketRow,
    author_kind: &str,
    user_id: Option<&str>,
    label: &str,
) -> AppResult<()> {
    if ticket.status == "closed" {
        return Ok(());
    }
    let timestamp = now();
    sqlx::query(
        "UPDATE tickets SET status='closed', closed_at=?, closed_by_kind=?, closed_by_user_id=?, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(author_kind)
    .bind(user_id)
    .bind(timestamp)
    .bind(&ticket.id)
    .execute(&state.db)
    .await?;
    insert_comment(
        state,
        &ticket.id,
        "system",
        "system",
        None,
        Some("系统"),
        &format!("**{label}** 关闭了工单。"),
    )
    .await?;
    Ok(())
}

pub async fn reopen(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<TicketView>> {
    let ticket = load_ticket(&state, &id).await?;
    let project = projects::load_project(&state, &ticket.project_id).await?;
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
        "UPDATE tickets SET status='open', closed_at=NULL, closed_by_kind=NULL, closed_by_user_id=NULL, last_reporter_at=?, updated_at=? WHERE id=?",
    )
    .bind(timestamp)
    .bind(timestamp)
    .bind(&id)
    .execute(&state.db)
    .await?;
    insert_comment(
        &state,
        &ticket.id,
        "system",
        "system",
        None,
        Some("系统"),
        "工单被重新打开，等待修复 Agent 处理。",
    )
    .await?;
    read(State(state), Extension(user), Path(id)).await
}

pub async fn read_thread(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<ThreadView>> {
    let ticket = load_ticket(&state, &id).await?;
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
    let live = match account::reporter_key(&state, &ticket.reporter_user_id).await? {
        Some(key) => state.cybion.read_thread(&key, &thread_id).await,
        None => Err("报障人还没有连接 Cybion 账号".to_owned()),
    };
    let (title, status, display_status, context_tokens, error) = match live {
        Ok(info) => (
            Some(info.title),
            Some(info.status),
            Some(info.display_status),
            info.context_tokens,
            None,
        ),
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
    if ticket.reporter_user_id != user.id {
        return Err(AppError::forbidden("只有报障人可以创建现场调查 Thread"));
    }
    if ticket.reporter_thread_id.is_some() {
        return Err(AppError::conflict("该工单已经有 Cybion Thread"));
    }
    ensure_reporter_thread(&state, &ticket.id).await?;
    read_thread(State(state), Extension(user), Path(id)).await
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
    let Some(key) = account::reporter_key(state, &ticket.reporter_user_id).await? else {
        set_thread_error(
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
            set_thread_error(state, ticket_id, Some(&error)).await?;
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
    .bind(crate::crypto::token_hint(&token))
    .bind(ticket_id)
    .execute(&state.db)
    .await?;

    let briefing = crate::prompt::reporter_briefing(&crate::prompt::ReporterBriefing {
        cybion_base: state.cybion.base().to_owned(),
        ticket_number: ticket.number,
        project_name: &project.name,
        project_description: &project.description,
        title: &ticket.title,
        body: &ticket.body,
        reporter: &reporter,
        draft: ticket.status == "draft",
        api_base: &state.public_base_url,
        ticket_id: &ticket.id,
        token: &token,
    });
    if let Err(error) = state.cybion.post_input(&key, &thread.id, &briefing).await {
        set_thread_error(
            state,
            ticket_id,
            Some(&format!("Thread 已创建，但调查简报发送失败：{error}")),
        )
        .await?;
    }
    Ok(())
}

/// Pushes a new message into the reporter's Thread, creating the Thread first
/// when the Ticket never got one. Failures land on the Ticket so the UI can
/// show them instead of losing the message silently.
pub async fn progress_reporter_thread(
    state: &AppState,
    ticket_id: &str,
    nudge: &str,
) -> AppResult<()> {
    ensure_reporter_thread(state, ticket_id).await?;
    let ticket = load_ticket(state, ticket_id).await?;
    let Some(thread_id) = ticket.reporter_thread_id.clone() else {
        return Ok(());
    };
    let Some(key) = account::reporter_key(state, &ticket.reporter_user_id).await? else {
        set_thread_error(
            state,
            ticket_id,
            Some("报障人没有连接 Cybion，评论无法推进现场调查 Thread"),
        )
        .await?;
        return Ok(());
    };
    match state.cybion.post_input(&key, &thread_id, nudge).await {
        Ok(()) => Ok(()),
        Err(error) => {
            set_thread_error(
                state,
                ticket_id,
                Some(&format!("评论已记录，但推进现场调查 Thread 失败：{error}")),
            )
            .await?;
            Err(AppError::unavailable(error))
        }
    }
}

async fn set_thread_error(state: &AppState, ticket_id: &str, error: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET reporter_thread_error=? WHERE id=?")
        .bind(error)
        .bind(ticket_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Shared by the browser UI and the agent API so both sides describe a Ticket
/// the same way.
pub async fn agent_view(state: &AppState, ticket_id: &str) -> AppResult<Value> {
    let ticket = load_ticket(state, ticket_id).await?;
    let project = projects::load_project(state, &ticket.project_id).await?;
    let comments = comments(state, ticket_id).await?;
    Ok(json!({
        "id": ticket.id,
        "number": ticket.number,
        "title": ticket.title,
        "body": ticket.body,
        "status": ticket.status,
        "project": project.name,
        "reporter": display_name(state, &ticket.reporter_user_id).await?,
        "created_at": ticket.created_at,
        "updated_at": ticket.updated_at,
        "comments": comments,
    }))
}
