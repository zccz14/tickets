mod support;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::json;
use support::*;
use tickets::crypto::hash_token;

const ALICE_KEY: &str = "cyb_alice_0123456789";
const BOB_KEY: &str = "cyb_bob_9876543210";

async fn connect(app: &App, key: &str) {
    let (status, me) = app
        .put("/api/account/cybion", json!({ "api_key": key }))
        .await;
    assert_eq!(status, StatusCode::OK, "连接 Cybion 失败: {me}");
}

async fn wait_for_thread(app: &App, ticket_id: &str) -> serde_json::Value {
    for _ in 0..200 {
        let ticket = app.ticket(ticket_id).await;
        if ticket["reporter_thread"]["thread_id"].is_string() {
            return ticket;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("工单 {ticket_id} 的 Thread 没有在预期时间内创建");
}

async fn wait_for_thread_error(app: &App, ticket_id: &str) -> serde_json::Value {
    for _ in 0..200 {
        let ticket = app.ticket(ticket_id).await;
        if ticket["reporter_thread"]["error"].is_string() {
            return ticket;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("工单 {ticket_id} 没有记录 Thread 错误");
}

#[tokio::test]
async fn connecting_cybion_probes_the_key_and_encrypts_it() {
    let env = Env::new().await;
    let user = env.app("alice").await;

    // Shape check happens before any network call.
    let (status, body) = user
        .put("/api/account/cybion", json!({ "api_key": "nope" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(error_message(&body).contains("cyb_"));
    assert!(env.mock.calls_to("/v1/threads").is_empty());

    // A key Cybion rejects is not stored.
    env.mock.fail_read_with(401);
    let (status, body) = user
        .put("/api/account/cybion", json!({ "api_key": ALICE_KEY }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(error_message(&body).contains("无效"));
    let (_, me) = user.get("/api/me").await;
    assert_eq!(me["cybion_connected"], false);

    // A working key is stored sealed: the database never holds the plaintext.
    env.mock.fail_read_with(0);
    connect(&user, ALICE_KEY).await;
    let cipher: String =
        sqlx::query_scalar("SELECT api_key_cipher FROM cybion_accounts WHERE user_id=?")
            .bind("alice")
            .fetch_one(&user.state.db)
            .await
            .unwrap();
    assert_ne!(cipher, ALICE_KEY);
    assert!(!cipher.contains(ALICE_KEY));
    assert_eq!(user.state.secrets.open(&cipher).unwrap(), ALICE_KEY);

    let (_, me) = user.get("/api/me").await;
    assert_eq!(me["cybion_connected"], true);
    assert_eq!(me["cybion_key_hint"], "cyb_alice_01…");

    let (status, body) = user.call("DELETE", "/api/account/cybion", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["disconnected"], true);
    let (_, me) = user.get("/api/me").await;
    assert_eq!(me["cybion_connected"], false);
}

#[tokio::test]
async fn a_failed_thread_creation_is_recorded_and_can_be_retried() {
    let env = Env::new().await;
    let user = env.app("alice").await;
    connect(&user, ALICE_KEY).await;
    let project = user.create_project("笔记本集群").await;

    env.mock.fail_create_with(500);
    let ticket = user
        .create_ticket(&project, "节点掉线", "两台节点从昨晚起离线", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    let failed = wait_for_thread_error(&user, &id).await;
    assert_eq!(failed["status"], "open", "Thread 失败不阻塞工单提交");
    assert!(
        failed["reporter_thread"]["error"]
            .as_str()
            .unwrap()
            .contains("Cybion 返回 500")
    );
    assert_eq!(env.mock.thread_count(), 0);

    // Only the reporter may retry.
    let other = env.app("bob").await;
    let (status, _) = other.post(&format!("/api/tickets/{id}/thread"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    env.mock.fail_create_with(0);
    let (status, thread) = user.post(&format!("/api/tickets/{id}/thread"), None).await;
    assert_eq!(status, StatusCode::OK, "{thread}");
    assert!(thread["thread_id"].is_string());
    assert!(thread["error"].is_null());
    let recovered = user.ticket(&id).await;
    assert!(recovered["reporter_thread"]["error"].is_null());
    assert_eq!(env.mock.thread_count(), 1);
}

#[tokio::test]
async fn a_failed_briefing_keeps_the_thread_and_says_so() {
    let env = Env::new().await;
    let user = env.app("alice").await;
    connect(&user, ALICE_KEY).await;
    let project = user.create_project("支付网关").await;

    env.mock.fail_input_with(500);
    let ticket = user
        .create_ticket(&project, "退款回调丢失", "用户退款成功但订单没变", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    let broken = wait_for_thread_error(&user, &id).await;
    assert!(broken["reporter_thread"]["thread_id"].is_string());
    assert!(
        broken["reporter_thread"]["error"]
            .as_str()
            .unwrap()
            .contains("调查简报发送失败")
    );

    // The Thread exists, so asking again is a conflict rather than a duplicate.
    let (status, _) = user.post(&format!("/api/tickets/{id}/thread"), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn each_ticket_gets_its_reporters_own_key_and_a_hashed_token() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;
    connect(&alice, ALICE_KEY).await;
    connect(&bob, BOB_KEY).await;
    let alice_project = alice.create_project("支付网关").await;
    let bob_project = bob.create_project("LOL 数据管道").await;

    let alice_ticket = alice
        .create_ticket(&alice_project, "回调超时", "3% 的回调超过 30 秒", true)
        .await;
    let alice_id = alice_ticket["id"].as_str().unwrap().to_owned();
    let bob_ticket = bob
        .create_ticket(&bob_project, "采集器崩溃", "退出码 137", true)
        .await;
    let bob_id = bob_ticket["id"].as_str().unwrap().to_owned();
    wait_for_thread(&alice, &alice_id).await;
    wait_for_thread(&bob, &bob_id).await;

    // Every Thread is created with the reporter's own key, never a shared one.
    let creates = env.mock.calls_to("/v1/threads");
    assert_eq!(creates.len(), 2);
    assert_eq!(creates[0].bearer.as_deref(), Some(ALICE_KEY));
    assert_eq!(creates[1].bearer.as_deref(), Some(BOB_KEY));
    assert_eq!(creates[0].body["title"], "[#1] 回调超时");
    assert_eq!(
        creates[0].body["external_ref"],
        format!("{PUBLIC_BASE}/#/tickets/{alice_id}")
    );

    // The briefing hands the Thread the ticket, the API base and its token.
    let briefing = env.mock.briefing_for(&alice_id);
    assert!(briefing.contains(&alice_id));
    assert!(briefing.contains("支付网关"));
    assert!(briefing.contains(PUBLIC_BASE));
    assert!(briefing.contains("只读"));
    let token = extract_agent_token(&briefing);
    assert_eq!(token.len(), 3 + 48);

    // The database keeps only the hash and a short hint.
    let (hash, hint): (String, String) = sqlx::query_as(
        "SELECT reporter_agent_token_hash,reporter_agent_token_hint FROM tickets WHERE id=?",
    )
    .bind(&alice_id)
    .fetch_one(&alice.state.db)
    .await
    .unwrap();
    assert_eq!(hash, hash_token(&token));
    assert_ne!(hash, token);
    assert_eq!(hint, format!("{}…", &token[..12]));

    // The Thread is owned by the reporter and points back at the ticket.
    let stored: String = sqlx::query_scalar("SELECT reporter_thread_owner FROM tickets WHERE id=?")
        .bind(&alice_id)
        .fetch_one(&alice.state.db)
        .await
        .unwrap();
    assert_eq!(stored, "alice");
    let view = alice.ticket(&alice_id).await;
    assert_eq!(
        view["reporter_thread"]["url"],
        format!("{}/#/threads/thread-0001", env.mock.base)
    );
    assert_eq!(
        view["agent"]["comments_url"],
        format!("{PUBLIC_BASE}/api/agent/tickets/{alice_id}/comments")
    );
}

#[tokio::test]
async fn only_the_reporter_sees_the_thread_link() {
    let env = Env::new().await;
    let reporter = env.app("reporter").await;
    let observer = env.app("observer").await;
    connect(&reporter, ALICE_KEY).await;
    let project = reporter.create_project("笔记本集群").await;
    let ticket = reporter
        .create_ticket(&project, "节点离线", "两台节点离线", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    wait_for_thread(&reporter, &id).await;

    let (status, live) = reporter.get(&format!("/api/tickets/{id}/thread")).await;
    assert_eq!(status, StatusCode::OK, "{live}");
    assert_eq!(live["title"], "[#1] 示例工单");
    assert_eq!(live["display_status"], "运行中");
    assert_eq!(live["context_tokens"], 4321);
    assert_eq!(
        live["url"],
        format!("{}/#/threads/thread-0001", env.mock.base)
    );
    // The probe used the reporter's own key.
    let probe = env
        .mock
        .calls_to("/v1/threads/thread-0001")
        .into_iter()
        .last()
        .expect("应当探测过 Thread");
    assert_eq!(probe.bearer.as_deref(), Some(ALICE_KEY));

    // Anyone signed in can read an open ticket, but never the Thread link.
    let (status, seen) = observer.get(&format!("/api/tickets/{id}/thread")).await;
    assert_eq!(status, StatusCode::OK, "{seen}");
    assert_eq!(seen["thread_id"], "thread-0001");
    assert!(seen["url"].is_null());

    // A Cybion outage surfaces on the card instead of breaking the page.
    env.mock.fail_read_with(503);
    let (status, broken) = reporter.get(&format!("/api/tickets/{id}/thread")).await;
    assert_eq!(status, StatusCode::OK, "{broken}");
    assert!(
        broken["error"]
            .as_str()
            .unwrap()
            .contains("Cybion 返回 503")
    );
    assert!(broken["url"].is_string());
}

/// Polls the mock until the Thread receives an input containing `needle`.
async fn wait_for_input(env: &Env, thread_id: &str, needle: &str) -> Call {
    let path = format!("/v1/threads/{thread_id}/inputs");
    for _ in 0..200 {
        if let Some(call) = env.mock.calls_to(&path).into_iter().find(|call| {
            call.body["input"]
                .as_str()
                .is_some_and(|input| input.contains(needle))
        }) {
            return call;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("Thread {thread_id} 没有收到包含「{needle}」的输入");
}

#[tokio::test]
async fn a_comment_wakes_the_reporter_thread() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    connect(&alice, ALICE_KEY).await;

    let project = alice.create_project("payments").await;
    let ticket = alice
        .create_ticket(&project, "支付回调 502", "线上回调偶发 502", true)
        .await;
    let id = ticket["id"].as_str().expect("工单 id").to_owned();
    let ready = wait_for_thread(&alice, &id).await;
    let thread_id = ready["reporter_thread"]["thread_id"]
        .as_str()
        .expect("thread id")
        .to_owned();

    alice.comment(&id, "补充：只在凌晨批量对账时出现").await;

    let nudge = wait_for_input(&env, &thread_id, "只在凌晨批量对账时出现").await;
    let text = nudge.body["input"].as_str().unwrap();
    assert!(text.contains("报障人"), "评论唤醒没有标明角色: {text}");
    assert!(
        text.contains(&format!("#{}", ready["number"])),
        "缺少工单号: {text}"
    );
    assert_eq!(
        nudge.bearer.as_deref(),
        Some(ALICE_KEY),
        "唤醒必须用报障人自己的 Key"
    );
}

#[tokio::test]
async fn the_briefing_carries_the_project_description() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    connect(&alice, ALICE_KEY).await;

    let project = alice.create_project("payments").await;
    let (status, updated) = alice
        .patch(
            &format!("/api/projects/{project}"),
            json!({ "description": "支付网关与对账回调，夜里跑批" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");

    let ticket = alice
        .create_ticket(&project, "支付回调 502", "线上回调偶发 502", true)
        .await;
    let id = ticket["id"].as_str().expect("工单 id").to_owned();
    wait_for_thread(&alice, &id).await;

    let briefing = env.mock.briefing_for(&id);
    assert!(
        briefing.contains("支付网关与对账回调，夜里跑批"),
        "简报缺少项目描述: {briefing}"
    );
}
