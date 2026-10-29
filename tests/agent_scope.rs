mod support;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::json;
use support::*;

const REPORTER_KEY: &str = "cyb_reporter_key_0001";

async fn connect_cybion(app: &App, key: &str) {
    let (status, me) = app
        .put("/api/account/cybion", json!({ "api_key": key }))
        .await;
    assert_eq!(status, StatusCode::OK, "连接 Cybion 失败: {me}");
    assert_eq!(me["cybion_connected"], true);
    assert_eq!(me["cybion_key_hint"].as_str().unwrap().chars().count(), 13);
}

/// Waits for the background Thread creation the app kicks off on submit, then
/// lifts the one-off reporter token out of the briefing Cybion received.
async fn reporter_token(env: &Env, reporter: &App, ticket_id: &str) -> String {
    for _ in 0..200 {
        let ticket = reporter.ticket(ticket_id).await;
        if ticket["reporter_thread"]["thread_id"].is_string()
            && let Some(briefing) = env.mock.try_briefing_for(ticket_id)
        {
            return extract_agent_token(&briefing);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("工单 {ticket_id} 的 Thread 没有在预期时间内创建");
}

#[tokio::test]
async fn reporter_token_can_only_touch_its_own_ticket() {
    let env = Env::new().await;
    let reporter = env.app("reporter").await;
    connect_cybion(&reporter, REPORTER_KEY).await;
    let project = reporter.create_project("支付网关").await;
    let first = reporter
        .create_ticket(&project, "回调超时", "3% 的回调超过 30 秒", true)
        .await;
    let second = reporter
        .create_ticket(&project, "对账不平", "昨天的账单差 12 元", true)
        .await;
    let first_id = first["id"].as_str().unwrap().to_owned();
    let second_id = second["id"].as_str().unwrap().to_owned();
    let token = reporter_token(&env, &reporter, &first_id).await;

    let (status, who) = reporter
        .call_with(Some(&token), "GET", "/api/agent/whoami", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{who}");
    assert_eq!(who["role"], "reporter");
    assert_eq!(who["ticket_id"], first_id);
    assert_eq!(who["user_id"], "reporter");

    // Job one: read the scene and write evidence back to the own ticket.
    let (status, evidence) = reporter
        .call_with(
            Some(&token),
            "POST",
            &format!("/api/agent/tickets/{first_id}/comments"),
            Some(json!({ "kind": "evidence", "body": "### 现场环境\n网关 2.3.1" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{evidence}");
    assert_eq!(evidence["kind"], "evidence");
    let ticket = reporter.ticket(&first_id).await;
    let comments = ticket["comments"].as_array().unwrap();
    let last = comments.last().unwrap();
    assert_eq!(last["kind"], "evidence");
    assert_eq!(last["author_kind"], "reporter_agent");
    assert_eq!(last["author_name"], "报障 Agent · #1");

    // The same token is powerless on any other ticket, even in the same project.
    for (method, uri, body) in [
        ("GET", format!("/api/agent/tickets/{second_id}"), None),
        (
            "POST",
            format!("/api/agent/tickets/{second_id}/comments"),
            Some(json!({ "body": "越权" })),
        ),
        (
            "POST",
            format!("/api/agent/tickets/{second_id}/close"),
            Some(json!({})),
        ),
    ] {
        let (status, body) = reporter.call_with(Some(&token), method, &uri, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} -> {body}");
        assert_eq!(error_message(&body), "该令牌只对创建它的工单有效");
    }

    // Only the resolver reads the queue.
    let (status, body) = reporter
        .call_with(Some(&token), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    // Closing through the reporter token is attributed to the reporter Agent.
    let (status, closed) = reporter
        .call_with(
            Some(&token),
            "POST",
            &format!("/api/agent/tickets/{first_id}/close"),
            Some(json!({ "body": "现场证据已补齐" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    let ticket = reporter.ticket(&first_id).await;
    assert_eq!(ticket["status"], "closed");
    assert_eq!(ticket["closed_by_kind"], "reporter_agent");
    // The chip names the side; the audit comment carries the exact agent label.
    assert_eq!(ticket["closed_by_name"], "报障 Agent");
    let audit = ticket["comments"].as_array().unwrap().last().unwrap();
    assert_eq!(audit["author_kind"], "system");
    assert_eq!(audit["body"], "**报障 Agent · #1** 关闭了工单。");
}

#[tokio::test]
async fn a_draft_thread_investigates_before_submitting() {
    let env = Env::new().await;
    let reporter = env.app("reporter").await;
    connect_cybion(&reporter, REPORTER_KEY).await;
    let project = reporter.create_project("LOL 数据管道").await;
    let draft = reporter
        .create_ticket(&project, "采集器不写入", "不确定从哪里开始查", false)
        .await;
    let id = draft["id"].as_str().unwrap().to_owned();
    assert_eq!(draft["status"], "draft");
    assert!(draft["reporter_thread"]["thread_id"].is_null());
    assert_eq!(env.mock.thread_count(), 0, "草稿不应该自动开 Thread");

    // The reporter explicitly asks for an investigation Thread.
    let (status, thread) = reporter
        .post(&format!("/api/tickets/{id}/thread"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{thread}");
    assert!(thread["thread_id"].is_string());
    let briefing = env.mock.briefing_for(&id);
    assert!(briefing.contains("草稿"), "简报应当说明这是草稿");
    assert!(briefing.contains("LOL 数据管道"));
    assert!(briefing.contains(PUBLIC_BASE));
    assert_eq!(env.mock.thread_count(), 1);

    // A second attempt is refused rather than opening a duplicate Thread.
    let (status, body) = reporter
        .post(&format!("/api/tickets/{id}/thread"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // The reporter Agent may add evidence while the ticket is still a draft;
    // the resolver key may not see the draft at all.
    let token = extract_agent_token(&briefing);
    let (status, _) = reporter
        .call_with(
            Some(&token),
            "POST",
            &format!("/api/agent/tickets/{id}/comments"),
            Some(json!({ "kind": "evidence", "body": "采集器进程不存在" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let key = reporter.create_resolver_key(&project).await;
    let (status, body) = reporter
        .call_with(Some(&key), "GET", &format!("/api/agent/tickets/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(error_message(&body), "草稿工单不在修复队列里");

    // Submitting reuses the existing Thread and lands in the resolver queue.
    let (status, submitted) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(
        submitted["reporter_thread"]["thread_id"],
        thread["thread_id"]
    );
    assert_eq!(env.mock.thread_count(), 1, "提交不应重复创建 Thread");
    let (_, queue) = reporter
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(queue["pending"].as_array().unwrap().len(), 1);
    assert_eq!(queue["pending"][0]["evidence"], "采集器进程不存在");
}

#[tokio::test]
async fn resolver_key_is_bound_to_one_project() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let first = owner.create_project("支付网关").await;
    let second = owner.create_project("笔记本集群").await;
    let key = owner.create_resolver_key(&first).await;
    let mine = owner
        .create_ticket(&first, "回调超时", "3% 超时", true)
        .await;
    let other = owner
        .create_ticket(&second, "节点离线", "两台离线", true)
        .await;
    let mine_id = mine["id"].as_str().unwrap().to_owned();
    let other_id = other["id"].as_str().unwrap().to_owned();

    let (status, who) = owner
        .call_with(Some(&key), "GET", "/api/agent/whoami", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{who}");
    assert_eq!(who["role"], "resolver");
    assert_eq!(who["project_id"], first);

    // In-project access works.
    let (status, view) = owner
        .call_with(
            Some(&key),
            "GET",
            &format!("/api/agent/tickets/{mine_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["id"], mine_id);

    // Out-of-project access does not, on any verb.
    for (method, uri, body) in [
        ("GET", format!("/api/agent/tickets/{other_id}"), None),
        (
            "POST",
            format!("/api/agent/tickets/{other_id}/comments"),
            Some(json!({ "body": "越权" })),
        ),
        (
            "POST",
            format!("/api/agent/tickets/{other_id}/close"),
            Some(json!({})),
        ),
    ] {
        let (status, body) = owner.call_with(Some(&key), method, &uri, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} -> {body}");
        assert_eq!(error_message(&body), "该令牌只对所属项目有效");
    }

    // Evidence is the reporter Agent's exclusive channel.
    let (status, body) = owner
        .call_with(
            Some(&key),
            "POST",
            &format!("/api/agent/tickets/{mine_id}/comments"),
            Some(json!({ "kind": "evidence", "body": "伪造证据" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(error_message(&body), "只有报障 Agent 可以写入证据");

    // An unknown kind is a bad request, not a silent comment.
    let (status, _) = owner
        .call_with(
            Some(&key),
            "POST",
            &format!("/api/agent/tickets/{mine_id}/comments"),
            Some(json!({ "kind": "verdict", "body": "随便" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Closing is idempotent and leaves the queue.
    for expected in [false, true] {
        let (status, closed) = owner
            .call_with(
                Some(&key),
                "POST",
                &format!("/api/agent/tickets/{mine_id}/close"),
                Some(json!({ "body": "已修复并验证" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{closed}");
        assert_eq!(closed["closed"], true);
        if expected {
            assert_eq!(closed["already"], true);
        } else {
            assert!(closed["already"].is_null());
        }
    }
    let (_, queue) = owner
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(queue["total_open"], 0);
    assert!(queue["pending"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn the_queue_is_oldest_first_capped_at_five() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let project = owner.create_project("支付网关").await;
    let key = owner.create_resolver_key(&project).await;

    let mut ids = Vec::new();
    for index in 1..=7 {
        let ticket = owner
            .create_ticket(
                &project,
                &format!("工单 {index}"),
                &"很长的现象描述 ".repeat(80),
                true,
            )
            .await;
        let id = ticket["id"].as_str().unwrap().to_owned();
        owner.stamp(&id, 1000 + index, 0).await;
        ids.push(id);
    }

    let (status, queue) = owner
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(queue["total_open"], 7);
    let pending = queue["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 5, "一次最多返回 5 张");
    let numbers: Vec<i64> = pending
        .iter()
        .map(|t| t["number"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5], "先返回最久没被处理的");
    // Long bodies are collapsed and truncated so the check output stays small.
    let summary = pending[0]["summary"].as_str().unwrap();
    assert!(summary.ends_with('…'));
    assert!(summary.chars().count() <= 601);

    // A resolver comment marks the ticket as answered and removes it.
    let (status, _) = owner
        .call_with(
            Some(&key),
            "POST",
            &format!("/api/agent/tickets/{}/comments", ids[0]),
            Some(json!({ "body": "请补充网关版本" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, queue) = owner
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    let numbers: Vec<i64> = queue["pending"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["number"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, vec![2, 3, 4, 5, 6]);
    assert_eq!(queue["total_open"], 7, "total_open 统计所有 Open 工单");

    // The reporter answering puts it back at the tail of the queue.
    let (status, _) = owner
        .post(
            &format!("/api/tickets/{}/comments", ids[0]),
            Some(json!({ "body": "网关 2.3.1" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, queue) = owner
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    let numbers: Vec<i64> = queue["pending"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["number"].as_i64().unwrap())
        .collect();
    assert_eq!(
        numbers,
        vec![2, 3, 4, 5, 6],
        "被回复过的工单排到队尾，仍受 5 张上限约束"
    );
    assert_eq!(queue["total_open"], 7);
}

#[tokio::test]
async fn agent_routes_reject_anything_but_a_live_agent_token() {
    let env = Env::new().await;
    let user = env.app("alice").await;

    let (status, body) = user.get("/api/agent/queue").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_message(&body), "缺少访问令牌");

    let (status, body) = user
        .call_with(Some("tk_deadbeef"), "GET", "/api/agent/whoami", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(error_message(&body).contains("Agent 令牌无效"));

    // A browser-style token is not an agent token, even if it is well formed.
    let (status, body) = user
        .call_with(
            Some("eyJhbGciOiJIUzI1NiJ9.e30.signature"),
            "GET",
            "/api/agent/whoami",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(error_message(&body), "无效的 Agent 令牌");
}
