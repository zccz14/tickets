//! The resolver is a Cybion Ultimate Machine: Tickets mints the key, builds the
//! command and asks Cybion to schedule it. Nothing here asks the owner to paste
//! a secret or a shell snippet.

mod support;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::*;

const TOKEN: &str = "jwt-alice";

async fn connect(env: &Env, app: &App) {
    let (status, me) = app
        .call_with(Some(TOKEN), "PUT", "/api/account/cybion", Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::OK, "自动连接 Cybion 失败: {me}");
    assert_eq!(me["cybion_connected"], true);
    assert_eq!(env.mock.calls_to("/api/api-keys").len(), 1);
}

async fn wait_for_thread(env: &Env) {
    for _ in 0..200 {
        if !env.mock.calls_to("/v1/threads").is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("没有创建 Cybion Thread");
}

async fn project_view(app: &App, id: &str) -> Value {
    let (status, project) = app.get(&format!("/api/projects/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{project}");
    project
}

#[tokio::test]
async fn connecting_mints_a_key_with_the_browser_token() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    connect(&env, &alice).await;

    let minted = env.mock.calls_to("/api/api-keys");
    assert_eq!(
        minted[0].bearer.as_deref(),
        Some(TOKEN),
        "铸钥必须用用户自己的令牌"
    );

    let project = alice.create_project("payments").await;
    alice
        .create_ticket(&project, "回调 502", "线上偶发", true)
        .await;
    wait_for_thread(&env).await;
    let created = env.mock.calls_to("/v1/threads");
    assert_eq!(
        created[0].bearer.as_deref(),
        Some(MOCK_API_KEY),
        "Thread 必须用后台铸出来的 Key 创建"
    );
}

#[tokio::test]
async fn connecting_a_project_creates_the_ultimate_machine() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    connect(&env, &alice).await;
    let project = alice.create_project("payments").await;

    let (status, body) = alice
        .call_with(
            Some(TOKEN),
            "POST",
            &format!("/api/projects/{project}/resolver"),
            Some(json!({ "worker_id": "worker-1" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "接入终极机器失败: {body}");
    assert_eq!(body["machine"]["id"], "machine-0001");
    assert_eq!(body["machine"]["worker_id"], "worker-1");

    let created = env.mock.calls_to("/api/machines");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].bearer.as_deref(), Some(TOKEN));
    let command = created[0].body["command"].as_str().expect("检查命令");
    assert!(command.contains("api/agent/queue"), "{command}");
    assert!(
        command.contains("tk_"),
        "命令里必须带 resolver 令牌: {command}"
    );
    assert_eq!(created[0].body["interval_seconds"], 120);

    let view = project_view(&alice, &project).await;
    assert_eq!(view["resolver_machine_id"], "machine-0001");
    assert_eq!(view["resolver_ready"], true);
    assert!(
        view["resolver_thread_url"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/#/threads/thread-machine-0001"),
        "{view}"
    );
}

#[tokio::test]
async fn a_machine_can_be_paused_and_disconnected() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    connect(&env, &alice).await;
    let project = alice.create_project("payments").await;
    alice
        .call_with(
            Some(TOKEN),
            "POST",
            &format!("/api/projects/{project}/resolver"),
            Some(json!({ "worker_id": "worker-1" })),
        )
        .await;

    let (status, body) = alice
        .call_with(
            Some(TOKEN),
            "PATCH",
            &format!("/api/projects/{project}/resolver"),
            Some(json!({ "enabled": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["machine"]["enabled"], false);
    assert_eq!(
        env.mock.calls_to("/api/machines/machine-0001")[0].method,
        "PATCH"
    );

    let (status, body) = alice
        .call_with(
            Some(TOKEN),
            "DELETE",
            &format!("/api/projects/{project}/resolver"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["disconnected"], true);

    let view = project_view(&alice, &project).await;
    assert_eq!(view["resolver_ready"], false);
    assert!(view["resolver_machine_id"].is_null(), "{view}");
    assert!(
        env.mock
            .calls_to("/api/machines/machine-0001")
            .iter()
            .any(|call| call.method == "DELETE"),
        "断开时必须删除 Cybion 里的机器"
    );
}

#[tokio::test]
async fn workers_come_from_cybion() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let (status, body) = alice
        .call_with(Some(TOKEN), "GET", "/api/cybion/workers", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["workers"].as_array().expect("workers").len(), 2);
    assert_eq!(body["workers"][0]["label"], "MacMini");
    assert_eq!(
        env.mock.calls_to("/api/workers")[0].bearer.as_deref(),
        Some(TOKEN)
    );
}

#[tokio::test]
async fn resolver_calls_need_a_browser_token() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let project = alice.create_project("payments").await;
    let (status, body) = alice
        .post(
            &format!("/api/projects/{project}/resolver"),
            Some(json!({ "worker_id": "worker-1" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(error_message(&body).contains("Auth Mini"), "{body}");
}
