//! The resolver side is a push, not a poll.
//!
//! Submitting a Ticket opens one Cybion Thread on the project owner's account
//! and pushes the whole assignment into it. There is no queue endpoint, no
//! schedule, and no Worker to pick — the owner's account decides where the
//! Thread runs.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::*;

#[tokio::test]
async fn submitting_pushes_the_ticket_to_the_owners_resolver_thread() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;

    let ticket = reporter
        .create_ticket(
            &project,
            "回调 502",
            "线上偶发 502，日志里是上游超时",
            false,
        )
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    let (status, submitted) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    let pushed = reporter
        .wait_for_ticket(&id, "修复 Thread 应在提交后自动创建", |ticket| {
            ticket["resolver_thread"]["thread_id"].is_string()
        })
        .await;
    assert_eq!(pushed["status"], "open");
    assert!(
        pushed["resolver_thread"]["error"].is_null(),
        "派发不应留下错误: {pushed}"
    );

    // The Thread belongs to the project owner, created with their own key.
    let created = env.mock.calls_to("/v1/threads");
    assert_eq!(created.len(), 1, "一次提交只应开一个修复 Thread");
    assert_eq!(created[0].bearer.as_deref(), Some(MOCK_API_KEY));

    let briefing = env.mock.resolver_briefing(&id);
    assert!(
        briefing.contains("线上偶发 502"),
        "简报要带现象: {briefing}"
    );
    assert!(briefing.contains("支付网关"));
    assert!(briefing.contains("/api/agent/tickets/"));
    assert!(briefing.contains("/resolve"), "简报要给关闭入口");
    assert!(
        !briefing.contains("worker_id ="),
        "修复侧不该绑定 Worker: {briefing}"
    );
    assert!(
        briefing.contains("cybion_list_workers"),
        "设备由 Agent 自己挑"
    );

    // The dispatch itself is folded context; one short owner line starts it.
    let mut opening = String::new();
    for _ in 0..300 {
        opening = env.mock.last_input();
        if opening == "请处理这张工单。" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(opening, "请处理这张工单。");
}

#[tokio::test]
async fn a_submit_without_the_owners_cybion_records_why_and_retries_later() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    let project = owner.create_project("笔记本集群").await;
    let ticket = reporter
        .create_ticket(&project, "节点离线", "两台节点从昨晚开始离线", false)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();

    let (status, _) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let stuck = reporter
        .wait_for_ticket(&id, "没有 Cybion 账号时应记下原因", |ticket| {
            ticket["resolver_thread"]["error"].is_string()
        })
        .await;
    assert!(
        stuck["resolver_thread"]["error"]
            .as_str()
            .unwrap()
            .contains("连接 Cybion"),
        "{stuck}"
    );
    assert!(stuck["resolver_thread"]["thread_id"].is_null());
    assert!(env.mock.calls_to("/v1/threads").is_empty());

    // Only the project owner may retry, and only once Cybion is connected.
    let (status, body) = reporter
        .post(&format!("/api/tickets/{id}/dispatch"), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    owner.connect_cybion().await;
    let (status, dispatched) = owner
        .post(&format!("/api/tickets/{id}/dispatch"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{dispatched}");
    assert!(
        dispatched["resolver_thread"]["thread_id"].is_string(),
        "{dispatched}"
    );
    assert!(dispatched["resolver_thread"]["error"].is_null());
}

#[tokio::test]
async fn reopening_and_new_evidence_push_the_same_thread_again() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    reporter.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let ticket = reporter
        .create_ticket(&project, "回调 502", "线上偶发 502", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    reporter
        .wait_for_ticket(&id, "先等修复 Thread 建好", |ticket| {
            ticket["resolver_thread"]["thread_id"].is_string()
        })
        .await;
    let threads_after_submit = env.mock.calls_to("/v1/threads").len();

    // The reporter Agent finds something new on an open ticket: the resolver is
    // told, without opening a second Thread.
    let token = extract_agent_token(&env.mock.reporter_briefing(&id));
    let (status, body) = reporter
        .evidence(&token, &id, "### 关键证据\n上游 30s 超时")
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let note = reporter
        .wait_for_ticket(&id, "新证据应推送修复 Thread", |ticket| {
            ticket["evidence"].is_string()
        })
        .await;
    assert_eq!(note["evidence"], "### 关键证据\n上游 30s 超时");

    // Closing and reopening re-uses the Thread it already has.
    let (status, closed) = owner.post(&format!("/api/tickets/{id}/close"), None).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    let (status, reopened) = reporter
        .post(&format!("/api/tickets/{id}/reopen"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(reopened["status"], "open");
    assert_eq!(
        env.mock.calls_to("/v1/threads").len(),
        threads_after_submit,
        "重开不应新建 Thread"
    );
    // The reopen note is pushed in the background, like every other nudge.
    let mut nudge = String::new();
    for _ in 0..300 {
        nudge = env.mock.last_input();
        if nudge.contains("tickets-resolver-reopen") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(nudge.contains("tickets-resolver-reopen"), "{nudge}");
}

#[tokio::test]
async fn the_resolver_agent_resolves_and_closes_in_one_call() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let ticket = owner
        .create_ticket(&project, "回调 502", "线上偶发 502", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    owner
        .wait_for_ticket(&id, "先等修复 Thread 建好", |ticket| {
            ticket["resolver_thread"]["thread_id"].is_string()
        })
        .await;
    let token = extract_agent_token(&env.mock.resolver_briefing(&id));

    let (status, who) = owner
        .call_with(Some(&token), "GET", "/api/agent/whoami", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{who}");
    assert_eq!(who["role"], "resolver");
    assert_eq!(who["ticket_id"], id);

    // A conclusion is required: closing silently is not possible.
    let (status, body) = owner.resolve(&token, &id, "   ").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, resolved) = owner
        .resolve(&token, &id, "### 根因\n上游超时\n\n### 改了什么\n重试 4 次")
        .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["closed"], true);

    let ticket = owner.ticket(&id).await;
    assert_eq!(ticket["status"], "closed");
    assert_eq!(ticket["closed_by_kind"], "resolver_agent");
    assert_eq!(ticket["closed_by_name"], "修复 Agent");
    assert_eq!(
        ticket["resolution"],
        "### 根因\n上游超时\n\n### 改了什么\n重试 4 次"
    );
    assert!(ticket["resolution_at"].is_i64());

    // Resolving twice is a no-op rather than an error.
    let (status, again) = owner.resolve(&token, &id, "再写一次").await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["already"], true);
    assert_eq!(
        owner.ticket(&id).await["resolution"],
        "### 根因\n上游超时\n\n### 改了什么\n重试 4 次"
    );
}

#[tokio::test]
async fn a_draft_is_never_pushed_and_cannot_be_dispatched() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let draft = owner
        .create_ticket(&project, "还没想清楚", "先记一下", false)
        .await;
    let id = draft["id"].as_str().unwrap().to_owned();
    assert_eq!(draft["status"], "draft");

    let (status, body) = owner
        .post(&format!("/api/tickets/{id}/dispatch"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // Opening the draft starts the investigation, not the resolver.
    let (status, _) = owner
        .call_with(
            Some("jwt-browser"),
            "GET",
            &format!("/api/tickets/{id}/thread/live?after=0"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let pushed = owner
        .wait_for_ticket(&id, "草稿应只有报障 Thread", |ticket| {
            ticket["reporter_thread"]["thread_id"].is_string()
        })
        .await;
    assert!(pushed["resolver_thread"]["thread_id"].is_null());
    assert!(
        env.mock.try_resolver_briefing(&id).is_none(),
        "草稿不该被派发"
    );
    assert_eq!(env.mock.calls_to("/v1/threads").len(), 1);
}

#[tokio::test]
async fn dispatch_is_owner_only_and_refuses_closed_tickets() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let ticket = reporter
        .create_ticket(&project, "回调 502", "线上偶发 502", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    owner
        .wait_for_ticket(&id, "先等修复 Thread 建好", |ticket| {
            ticket["resolver_thread"]["thread_id"].is_string()
        })
        .await;
    let before = env.mock.calls_to("/v1/threads").len();

    // Dispatching an already-dispatched ticket changes nothing.
    let (status, _) = owner
        .post(&format!("/api/tickets/{id}/dispatch"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(env.mock.calls_to("/v1/threads").len(), before);

    let (status, closed) = owner.post(&format!("/api/tickets/{id}/close"), None).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    let (status, body) = owner
        .post(&format!("/api/tickets/{id}/dispatch"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(json!(body).is_object());
}

#[tokio::test]
async fn a_project_is_ready_as_soon_as_its_owner_connects_cybion() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let project = owner.create_project("支付网关").await;
    let (_, before) = owner.get(&format!("/api/projects/{project}")).await;
    assert_eq!(before["resolver_ready"], false);
    assert!(before["resolver_worker_id"].is_null());

    owner.connect_cybion().await;
    let (_, after) = owner.get(&format!("/api/projects/{project}")).await;
    assert_eq!(after["resolver_ready"], true);

    // There is no Worker endpoint left to configure: Cybion decides the machine.
    let (status, _) = owner.get("/api/cybion/workers").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = owner
        .put(
            &format!("/api/projects/{project}/resolver"),
            json!({ "worker_id": "worker-1" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
