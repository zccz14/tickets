//! Both agent tokens are bound to exactly one Ticket. A reporter token writes
//! evidence; a resolver token writes the conclusion. Neither reaches a second
//! Ticket, and neither has a project-wide view, because there is no queue.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::*;

/// Submits a ticket and waits for both Threads, returning its tokens.
async fn ticket_with_both_tokens(
    env: &Env,
    owner: &App,
    reporter: &App,
    project: &str,
    title: &str,
) -> (String, String, String) {
    // Both sides speak through their own Cybion account.
    reporter.connect_cybion().await;
    let ticket = reporter
        .create_ticket(project, title, "现象：偶发失败", false)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();
    let (status, _) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    owner
        .wait_for_ticket(&id, "两侧 Thread 都应建好", |ticket| {
            ticket["reporter_thread"]["thread_id"].is_string()
                && ticket["resolver_thread"]["thread_id"].is_string()
        })
        .await;
    let _ = env;
    let reporter_token = extract_agent_token(&env.mock.reporter_briefing(&id));
    let resolver_token = extract_agent_token(&env.mock.resolver_briefing(&id));
    (id, reporter_token, resolver_token)
}

#[tokio::test]
async fn the_reporter_token_writes_evidence_and_nothing_else() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let (id, token, _) =
        ticket_with_both_tokens(&env, &owner, &reporter, &project, "回调超时").await;

    let (status, who) = reporter
        .call_with(Some(&token), "GET", "/api/agent/whoami", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{who}");
    assert_eq!(who["role"], "reporter");
    assert_eq!(who["ticket_id"], id);
    assert_eq!(who["user_id"], "reporter");

    let (status, body) = reporter
        .evidence(&token, &id, "### 现场环境\n网关 2.3.1")
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ticket = reporter.ticket(&id).await;
    assert_eq!(ticket["evidence"], "### 现场环境\n网关 2.3.1");
    assert!(ticket["evidence_at"].is_i64());

    // Writing again replaces the report instead of piling up.
    let (status, _) = reporter
        .evidence(&token, &id, "### 现场环境\n网关 2.3.2")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        reporter.ticket(&id).await["evidence"],
        "### 现场环境\n网关 2.3.2"
    );

    // The reporter Agent cannot close: that is the resolver's verb.
    let (status, body) = reporter.resolve(&token, &id, "我来关").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(error_message(&body), "只有修复 Agent 可以给出处理结论");

    // Empty evidence is a bad request, not an empty report.
    let (status, _) = reporter.evidence(&token, &id, "   ").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_resolver_token_is_bound_to_its_own_ticket() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let (mine, token, resolver_token) =
        ticket_with_both_tokens(&env, &owner, &reporter, &project, "回调超时").await;
    let (other, _, _) =
        ticket_with_both_tokens(&env, &owner, &reporter, &project, "对账不平").await;

    // Reading its own ticket works and carries the evidence written so far.
    let (status, view) = owner
        .call_with(
            Some(&resolver_token),
            "GET",
            &format!("/api/agent/tickets/{mine}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["id"], mine);
    assert!(view["url"].as_str().unwrap().contains(&mine));

    // A second ticket in the same project is out of scope on every verb.
    for (method, uri, body) in [
        ("GET", format!("/api/agent/tickets/{other}"), None),
        (
            "POST",
            format!("/api/agent/tickets/{other}/resolve"),
            Some(json!({ "resolution": "越权" })),
        ),
        (
            "POST",
            format!("/api/agent/tickets/{other}/evidence"),
            Some(json!({ "body": "越权" })),
        ),
    ] {
        let (status, body) = owner
            .call_with(Some(&resolver_token), method, &uri, body)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} -> {body}");
        assert_eq!(error_message(&body), "该令牌只对创建它的工单有效");
    }

    // Evidence stays the reporter Agent's channel.
    let (status, body) = owner
        .call_with(
            Some(&resolver_token),
            "POST",
            &format!("/api/agent/tickets/{mine}/evidence"),
            Some(json!({ "body": "伪造证据" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(error_message(&body), "只有报障 Agent 可以写入现场证据");
    let _ = token;
}

#[tokio::test]
async fn there_is_no_queue_to_read() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    owner.connect_cybion().await;
    let project = owner.create_project("支付网关").await;
    let (_, _, resolver_token) =
        ticket_with_both_tokens(&env, &owner, &reporter, &project, "回调超时").await;

    // The polling protocol is gone: there is no queue route to call.
    let (status, _) = owner
        .call_with(Some(&resolver_token), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn agent_routes_reject_anything_but_a_live_agent_token() {
    let env = Env::new().await;
    let user = env.app("alice").await;

    let (status, body) = user.get("/api/agent/whoami").await;
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
