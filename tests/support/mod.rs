//! A real router over a throwaway database, plus a fake Cybion.
//!
//! Browser routes are exercised through `--dev-user` so each test can act as a
//! different signed-in user without minting Auth Mini tokens. Agent routes get
//! the real tokens the app hands out: the resolver key from the settings API
//! and the reporter token from the briefing the fake Cybion received.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tickets::{AppState, Options, bootstrap, db, router};
use tower::ServiceExt;

#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub bearer: Option<String>,
    pub body: Value,
}

#[derive(Default)]
pub struct MockState {
    calls: Mutex<Vec<Call>>,
    threads: Mutex<Vec<String>>,
    create_status: Mutex<u16>,
    input_status: Mutex<u16>,
    read_status: Mutex<u16>,
    seq: Mutex<u64>,
    machines: Mutex<HashMap<String, Value>>,
}

/// An in-process stand-in for Cybion's `/v1/threads` API. It records every call
/// so tests can assert what the server sent and with whose key.
pub struct MockCybion {
    pub base: String,
    state: Arc<MockState>,
}

impl MockCybion {
    pub fn calls(&self) -> Vec<Call> {
        self.state.calls.lock().unwrap().clone()
    }

    pub fn calls_to(&self, path: &str) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|call| call.path == path)
            .collect()
    }

    pub fn fail_create_with(&self, status: u16) {
        *self.state.create_status.lock().unwrap() = status;
    }

    pub fn fail_input_with(&self, status: u16) {
        *self.state.input_status.lock().unwrap() = status;
    }

    pub fn fail_read_with(&self, status: u16) {
        *self.state.read_status.lock().unwrap() = status;
    }

    pub fn thread_count(&self) -> usize {
        self.state.threads.lock().unwrap().len()
    }

    /// The briefing posted into the Thread that mentions this ticket.
    pub fn briefing_for(&self, ticket_id: &str) -> String {
        self.try_briefing_for(ticket_id)
            .unwrap_or_else(|| panic!("没有找到包含工单 {ticket_id} 的调查简报"))
    }

    pub fn try_briefing_for(&self, ticket_id: &str) -> Option<String> {
        self.calls()
            .into_iter()
            .filter(|call| call.path.ends_with("/inputs"))
            .filter_map(|call| call.body["input"].as_str().map(str::to_owned))
            .find(|input| input.contains(ticket_id))
    }
}

async fn create_thread(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "POST".to_owned(),
        path: "/v1/threads".to_owned(),
        bearer: bearer(&headers),
        body: body.clone(),
    });
    let status = *state.create_status.lock().unwrap();
    if status != 0 {
        return (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({ "error": "mock create failure" })),
        )
            .into_response();
    }
    let mut seq = state.seq.lock().unwrap();
    *seq += 1;
    let id = format!("thread-{:04}", *seq);
    drop(seq);
    state.threads.lock().unwrap().push(id.clone());
    Json(json!({
        "id": id,
        "title": body["title"],
        "status": "idle",
        "display_status": "空闲",
        "created_by": "mock-user",
        "context_tokens": 1234
    }))
    .into_response()
}

async fn post_input(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "POST".to_owned(),
        path: format!("/v1/threads/{id}/inputs"),
        bearer: bearer(&headers),
        body: body.clone(),
    });
    let status = *state.input_status.lock().unwrap();
    if status != 0 {
        return (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({ "error": "mock input failure" })),
        )
            .into_response();
    }
    Json(json!({ "accepted": true })).into_response()
}

async fn read_thread(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "GET".to_owned(),
        path: format!("/v1/threads/{id}"),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    let status = *state.read_status.lock().unwrap();
    if status != 0 {
        return (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({ "error": "mock read failure" })),
        )
            .into_response();
    }
    if !state
        .threads
        .lock()
        .unwrap()
        .iter()
        .any(|known| known == &id)
    {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "thread not found" })),
        )
            .into_response();
    }
    Json(json!({
        "id": id,
        "title": "[#1] 示例工单",
        "status": "running",
        "display_status": "运行中",
        "created_by": "mock-user",
        "context_tokens": 4321
    }))
    .into_response()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_owned)
}

/// The key the fake Cybion hands out when Tickets mints one.
pub const MOCK_API_KEY: &str = "cyb_mock_user_0123456789";

pub async fn start_mock_cybion() -> MockCybion {
    let state = Arc::new(MockState::default());
    let app = Router::new()
        .route("/v1/threads", post(create_thread))
        .route("/v1/threads/{id}/inputs", post(post_input))
        .route("/v1/threads/{id}", get(read_thread))
        .route("/api/api-keys", post(create_api_key))
        .route("/api/api-keys/{id}", delete(delete_api_key))
        .route("/api/workers", get(list_workers))
        .route("/api/machines", post(create_machine))
        .route(
            "/api/machines/{id}",
            get(read_machine)
                .patch(update_machine)
                .delete(delete_machine),
        )
        .route("/api/machines/{id}/run", post(run_machine))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("绑定 mock 端口");
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    MockCybion {
        base: format!("http://{address}"),
        state,
    }
}

/// One data directory plus the fake Cybion it points at. Every `app()` call
/// opens its own pool on the same database, which is how tests switch users.
pub struct Env {
    pub dir: TempDir,
    pub mock: MockCybion,
}

pub const PUBLIC_BASE: &str = "https://tickets.test";

impl Env {
    pub async fn new() -> Self {
        let mock = start_mock_cybion().await;
        let dir = tempfile::tempdir().expect("临时目录");
        let pool = db::connect(&dir.path().join("tickets.db"))
            .await
            .expect("建库");
        db::put_setting(&pool, "cybion_base_url", &mock.base)
            .await
            .unwrap();
        db::put_setting(&pool, "public_base_url", PUBLIC_BASE)
            .await
            .unwrap();
        pool.close().await;
        Self { dir, mock }
    }

    pub async fn app(&self, user: &str) -> App {
        self.build(Some(user)).await
    }

    /// No `--dev-user`: browser routes must reject unauthenticated requests.
    pub async fn anonymous_app(&self) -> App {
        self.build(None).await
    }

    async fn build(&self, dev_user: Option<&str>) -> App {
        let state = bootstrap(Options {
            listen: "127.0.0.1:0".parse().unwrap(),
            data_dir: self.dir.path().to_path_buf(),
            public_base_url: Some(PUBLIC_BASE.to_owned()),
            dev_user: dev_user.map(str::to_owned),
        })
        .await
        .expect("启动应用状态");
        let router = router(state.clone());
        App { state, router }
    }
}

pub struct App {
    pub state: AppState,
    pub router: Router,
}

impl App {
    pub async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send(self.router.clone(), method, uri, body).await
    }

    pub async fn call_with(
        &self,
        token: Option<&str>,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_with_token(self.router.clone(), token, method, uri, body).await
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.call("GET", uri, None).await
    }

    pub async fn post(&self, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.call("POST", uri, Some(body.unwrap_or(Value::Null)))
            .await
    }

    pub async fn put(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.call("PUT", uri, Some(body)).await
    }

    pub async fn patch(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.call("PATCH", uri, Some(body)).await
    }

    pub async fn create_project(&self, name: &str) -> String {
        let (status, project) = self
            .post("/api/projects", Some(json!({ "name": name })))
            .await;
        assert_eq!(status, StatusCode::OK, "创建项目失败: {project}");
        project["id"].as_str().expect("项目 id").to_owned()
    }

    pub async fn create_ticket(
        &self,
        project_id: &str,
        title: &str,
        body: &str,
        submit: bool,
    ) -> Value {
        let (status, ticket) = self
            .post(
                &format!("/api/projects/{project_id}/tickets"),
                Some(json!({ "title": title, "body": body, "submit": submit })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "创建工单失败: {ticket}");
        ticket
    }

    pub async fn ticket(&self, id: &str) -> Value {
        let (status, ticket) = self.get(&format!("/api/tickets/{id}")).await;
        assert_eq!(status, StatusCode::OK, "读取工单失败: {ticket}");
        ticket
    }

    pub async fn create_resolver_key(&self, project_id: &str) -> String {
        let (status, key) = self
            .post(&format!("/api/projects/{project_id}/resolver-key"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "生成修复密钥失败: {key}");
        key["token"].as_str().expect("修复密钥").to_owned()
    }

    /// Sets the queue watermark columns directly, so ordering tests do not
    /// depend on the wall clock.
    pub async fn stamp(&self, ticket_id: &str, reporter_at: i64, resolver_at: i64) {
        sqlx::query("UPDATE tickets SET last_reporter_at=?, last_resolver_at=? WHERE id=?")
            .bind(reporter_at)
            .bind(resolver_at)
            .bind(ticket_id)
            .execute(&self.state.db)
            .await
            .expect("写入队列水位");
    }

    pub async fn comment(&self, ticket_id: &str, body: &str) -> Value {
        let (status, ticket) = self
            .post(
                &format!("/api/tickets/{ticket_id}/comments"),
                Some(json!({ "body": body })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "评论失败: {ticket}");
        ticket
    }
}

/// One HTTP round-trip through the router, with an optional bearer token.
pub async fn send_with_token(
    router: Router,
    token: Option<&str>,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(Value::Null) | None => Body::empty(),
        Some(value) => {
            builder = builder.header(axum::http::header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
    };
    let response = router
        .oneshot(builder.body(body).unwrap())
        .await
        .expect("路由不应崩溃");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("读取响应体")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, value)
}

pub async fn send(
    router: Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send_with_token(router, None, method, uri, body).await
}

/// `tk_` plus 48 hex characters, extracted from the briefing the Thread got.
pub fn extract_agent_token(briefing: &str) -> String {
    let start = briefing.find("tk_").expect("简报里应当带一次性令牌");
    let tail = &briefing[start + 3..];
    let end = tail
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(tail.len());
    format!("tk_{}", &tail[..end])
}

pub fn error_message(value: &Value) -> &str {
    value["error"].as_str().unwrap_or("")
}

async fn create_api_key(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "POST".to_owned(),
        path: "/api/api-keys".to_owned(),
        bearer: bearer(&headers),
        body: body.clone(),
    });
    Json(json!({
        "key": { "id": "key-0001", "label": body["label"], "prefix": "cyb_mock", "created_at": 1 },
        "secret": MOCK_API_KEY
    }))
    .into_response()
}

async fn delete_api_key(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "DELETE".to_owned(),
        path: format!("/api/api-keys/{id}"),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    StatusCode::NO_CONTENT.into_response()
}

async fn list_workers(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Json<Value> {
    state.calls.lock().unwrap().push(Call {
        method: "GET".to_owned(),
        path: "/api/workers".to_owned(),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    Json(json!([
        { "id": "worker-1", "label": "MacMini", "status": "online", "owner_user_id": "alice", "last_seen_at": 10 },
        { "id": "worker-2", "label": "MacBookAir", "status": "offline", "owner_user_id": "alice" }
    ]))
}

async fn create_machine(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "POST".to_owned(),
        path: "/api/machines".to_owned(),
        bearer: bearer(&headers),
        body: body.clone(),
    });
    let id = format!("machine-{:04}", state.machines.lock().unwrap().len() + 1);
    let machine = json!({
        "id": id,
        "name": body["name"],
        "worker_id": body["worker_id"],
        "worker_label": "MacMini",
        "command": body["command"],
        "intent": body["intent"],
        "interval_seconds": body["interval_seconds"],
        "thread_id": format!("thread-{id}"),
        "thread_title": "终极机器 · 测试",
        "enabled": true,
        "last_run_at": null,
        "last_exit": null,
        "created_at": 1
    });
    state.machines.lock().unwrap().insert(id, machine.clone());
    Json(machine).into_response()
}

async fn read_machine(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "GET".to_owned(),
        path: format!("/api/machines/{id}"),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    match state.machines.lock().unwrap().get(&id).cloned() {
        Some(machine) => Json(machine).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "machine not found" })),
        )
            .into_response(),
    }
}

async fn update_machine(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "PATCH".to_owned(),
        path: format!("/api/machines/{id}"),
        bearer: bearer(&headers),
        body: body.clone(),
    });
    let mut machines = state.machines.lock().unwrap();
    let Some(machine) = machines.get_mut(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(enabled) = body["enabled"].as_bool() {
        machine["enabled"] = json!(enabled);
    }
    Json(machine.clone()).into_response()
}

async fn delete_machine(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "DELETE".to_owned(),
        path: format!("/api/machines/{id}"),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    if state.machines.lock().unwrap().remove(&id).is_some() {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn run_machine(
    State(state): State<Arc<MockState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    state.calls.lock().unwrap().push(Call {
        method: "POST".to_owned(),
        path: format!("/api/machines/{id}/run"),
        bearer: bearer(&headers),
        body: Value::Null,
    });
    if state.machines.lock().unwrap().contains_key(&id) {
        StatusCode::ACCEPTED.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
