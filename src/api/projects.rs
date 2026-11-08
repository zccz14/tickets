use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    auth::CurrentUser,
    crypto::{new_id, now},
    error::{AppError, AppResult},
    model::{CreateGrant, CreateProject, GrantView, ProjectView, UpdateProject},
};

pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_user_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub visibility: String,
}

pub async fn load_project(state: &AppState, id: &str) -> AppResult<ProjectRow> {
    let row = sqlx::query(
        "SELECT id,name,description,owner_user_id,created_at,updated_at,visibility FROM projects WHERE id=?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("项目不存在"))?;
    Ok(ProjectRow {
        id: row.get(0),
        name: row.get(1),
        description: row.get(2),
        owner_user_id: row.get(3),
        created_at: row.get(4),
        updated_at: row.get(5),
        visibility: row.get(6),
    })
}

/// Visibility is a two-value enum stored as text; anything else is a bug in the
/// caller rather than a state the database may hold.
fn validate_visibility(raw: &str) -> AppResult<String> {
    match raw.trim() {
        "public" => Ok("public".to_owned()),
        "private" => Ok("private".to_owned()),
        other => Err(AppError::bad_request(format!("未知的项目可见性: {other}"))),
    }
}

/// A project is visible when it is public, when the viewer owns it, or when the
/// owner granted that user access. Private projects answer 404 rather than 403
/// so their existence is not leaked.
pub async fn can_view(state: &AppState, project: &ProjectRow, user_id: &str) -> AppResult<bool> {
    if project.owner_user_id == user_id || project.visibility == "public" {
        return Ok(true);
    }
    let granted: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM project_grants WHERE project_id=? AND user_id=?)",
    )
    .bind(&project.id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(granted != 0)
}

pub async fn require_view(state: &AppState, project: &ProjectRow, user_id: &str) -> AppResult<()> {
    if can_view(state, project, user_id).await? {
        Ok(())
    } else {
        Err(AppError::not_found("项目不存在"))
    }
}

fn validate_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    let length = trimmed.chars().count();
    if length == 0 {
        return Err(AppError::bad_request("项目名称不能为空"));
    }
    if length > 80 {
        return Err(AppError::bad_request("项目名称不能超过 80 个字符"));
    }
    Ok(trimmed.to_owned())
}

const PROJECT_COLUMNS: &str = "
SELECT p.id,p.name,p.description,p.owner_user_id,p.created_at,p.updated_at,
       EXISTS(SELECT 1 FROM cybion_accounts a WHERE a.user_id=p.owner_user_id) AS resolver_ready,
       u.display_name,
       (SELECT COUNT(*) FROM tickets t WHERE t.project_id=p.id AND t.status='open') AS open_tickets,
       (SELECT COUNT(*) FROM tickets t WHERE t.project_id=p.id AND t.status<>'draft') AS total_tickets,
       p.visibility,
       EXISTS(SELECT 1 FROM project_grants g WHERE g.project_id=p.id AND g.user_id=?) AS is_member
FROM projects p
LEFT JOIN users u ON u.id=p.owner_user_id
";

fn project_view(row: &sqlx::sqlite::SqliteRow, viewer: &str) -> ProjectView {
    let owner_user_id: String = row.get(3);
    ProjectView {
        id: row.get(0),
        name: row.get(1),
        description: row.get(2),
        is_owner: owner_user_id == viewer,
        owner_user_id,
        created_at: row.get(4),
        updated_at: row.get(5),
        resolver_ready: row.get::<i64, _>(6) != 0,
        owner_name: row.get(7),
        open_tickets: row.get(8),
        total_tickets: row.get(9),
        visibility: row.get(10),
        is_member: row.get::<i64, _>(11) != 0,
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Vec<ProjectView>>> {
    let rows = sqlx::query(&format!(
        "{PROJECT_COLUMNS}
         WHERE p.visibility='public' OR p.owner_user_id=?
            OR EXISTS(SELECT 1 FROM project_grants g WHERE g.project_id=p.id AND g.user_id=?)
         ORDER BY open_tickets DESC, p.updated_at DESC"
    ))
    .bind(&user.id)
    .bind(&user.id)
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.iter().map(|row| project_view(row, &user.id)).collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateProject>,
) -> AppResult<Json<ProjectView>> {
    let name = validate_name(&input.name)?;
    let description = input.description.trim().to_owned();
    if description.chars().count() > 4000 {
        return Err(AppError::bad_request("项目描述不能超过 4000 个字符"));
    }
    let visibility = validate_visibility(input.visibility.as_deref().unwrap_or("private"))?;
    let id = new_id();
    let timestamp = now();
    sqlx::query(
        "INSERT INTO projects(id,owner_user_id,name,description,visibility,created_at,updated_at) VALUES(?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&name)
    .bind(&description)
    .bind(&visibility)
    .bind(timestamp)
    .bind(timestamp)
    .execute(&state.db)
    .await?;
    let project = load_project(&state, &id).await?;
    let row = sqlx::query(&format!("{PROJECT_COLUMNS} WHERE p.id=?"))
        .bind(&user.id)
        .bind(&project.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(project_view(&row, &user.id)))
}

pub async fn read(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectView>> {
    let project = load_project(&state, &id).await?;
    require_view(&state, &project, &user.id).await?;
    let row = sqlx::query(&format!("{PROJECT_COLUMNS} WHERE p.id=?"))
        .bind(&user.id)
        .bind(&id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(project_view(&row, &user.id)))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProject>,
) -> AppResult<Json<ProjectView>> {
    let project = load_project(&state, &id).await?;
    require_view(&state, &project, &user.id).await?;
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以修改项目"));
    }
    if let Some(name) = input.name.as_deref() {
        let name = validate_name(name)?;
        sqlx::query("UPDATE projects SET name=? WHERE id=?")
            .bind(name)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(description) = input.description.as_deref() {
        if description.chars().count() > 4000 {
            return Err(AppError::bad_request("项目描述不能超过 4000 个字符"));
        }
        sqlx::query("UPDATE projects SET description=? WHERE id=?")
            .bind(description.trim())
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(visibility) = input.visibility.as_deref() {
        let visibility = validate_visibility(visibility)?;
        sqlx::query("UPDATE projects SET visibility=? WHERE id=?")
            .bind(visibility)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    sqlx::query("UPDATE projects SET updated_at=? WHERE id=?")
        .bind(now())
        .bind(&id)
        .execute(&state.db)
        .await?;
    read(State(state), Extension(user), Path(id)).await
}

/// The owner's grant list. Grants only matter for private projects, but they are
/// kept while a project is public so flipping it back does not lose members.
pub async fn list_grants(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let project = load_project(&state, &id).await?;
    require_view(&state, &project, &user.id).await?;
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以管理成员"));
    }
    let rows = sqlx::query(
        "SELECT user_id,created_at FROM project_grants WHERE project_id=? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;
    let grants: Vec<GrantView> = rows
        .iter()
        .map(|row| GrantView {
            user_id: row.get(0),
            created_at: row.get(1),
        })
        .collect();
    Ok(Json(json!({ "grants": grants })))
}

pub async fn add_grant(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(input): Json<CreateGrant>,
) -> AppResult<Json<Value>> {
    let project = load_project(&state, &id).await?;
    require_view(&state, &project, &user.id).await?;
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以管理成员"));
    }
    let target = input.user_id.trim();
    if target.is_empty() {
        return Err(AppError::bad_request("用户 ID 不能为空"));
    }
    if target.chars().count() > 128 {
        return Err(AppError::bad_request("用户 ID 过长"));
    }
    if target == project.owner_user_id {
        return Err(AppError::bad_request("项目所有者本来就能访问"));
    }
    let existing: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM project_grants WHERE project_id=? AND user_id=?)",
    )
    .bind(&id)
    .bind(target)
    .fetch_one(&state.db)
    .await?;
    if existing != 0 {
        return Err(AppError::conflict("该用户已经在成员列表里"));
    }
    sqlx::query("INSERT INTO project_grants(project_id,user_id,created_at) VALUES(?,?,?)")
        .bind(&id)
        .bind(target)
        .bind(now())
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "user_id": target, "granted": true })))
}

pub async fn remove_grant(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path((id, target)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    let project = load_project(&state, &id).await?;
    require_view(&state, &project, &user.id).await?;
    if project.owner_user_id != user.id {
        return Err(AppError::forbidden("只有项目所有者可以管理成员"));
    }
    let result = sqlx::query("DELETE FROM project_grants WHERE project_id=? AND user_id=?")
        .bind(&id)
        .bind(&target)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "removed": result.rows_affected() > 0 })))
}
