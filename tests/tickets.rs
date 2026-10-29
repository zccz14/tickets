mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::*;

#[tokio::test]
async fn drafts_are_visible_only_to_their_reporter() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;
    let project = alice.create_project("支付网关").await;
    let draft = alice
        .create_ticket(
            &project,
            "充值回调偶发超时",
            "大约 3% 的回调超过 30 秒",
            false,
        )
        .await;
    let id = draft["id"].as_str().unwrap();
    assert_eq!(draft["status"], "draft");

    // Another user cannot read, comment on, or probe the Thread of a draft.
    let (status, _) = bob.get(&format!("/api/tickets/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = bob
        .post(
            &format!("/api/tickets/{id}/comments"),
            Some(json!({ "body": "看到了" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = bob.get(&format!("/api/tickets/{id}/thread")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Nor does it leak into any listing bob can ask for.
    for filter in ["all", "open", "closed"] {
        let (status, list) = bob
            .get(&format!("/api/projects/{project}/tickets?status={filter}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            list.as_array().unwrap().is_empty(),
            "{filter} 列表泄露了别人的草稿: {list}"
        );
    }

    // The reporter sees it as a draft and in the full list.
    let (_, drafts) = alice
        .get(&format!("/api/projects/{project}/tickets?status=draft"))
        .await;
    assert_eq!(drafts.as_array().unwrap().len(), 1);
    let (_, all) = alice
        .get(&format!("/api/projects/{project}/tickets?status=all"))
        .await;
    assert_eq!(all.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn submitting_opens_the_ticket_and_puts_it_in_the_queue() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    let project = owner.create_project("LOL 数据管道").await;
    let ticket = reporter
        .create_ticket(&project, "采集器停止写入", "最近 2 小时没有新记录", false)
        .await;
    let id = ticket["id"].as_str().unwrap();

    let (status, submitted) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(submitted["status"], "open");
    assert!(submitted["submitted_at"].is_i64());
    assert!(submitted["body"].as_str().unwrap().contains("2 小时"));
    let comments = submitted["comments"].as_array().unwrap();
    assert_eq!(comments.last().unwrap()["kind"], "system");

    // A second submit is a conflict, not a silent no-op.
    let (status, _) = reporter
        .post(&format!("/api/tickets/{id}/submit"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Only the reporter may submit.
    let other = env.app("alice").await;
    let draft = other.create_ticket(&project, "另一个草稿", "", false).await;
    let (status, _) = reporter
        .post(
            &format!("/api/tickets/{}/submit", draft["id"].as_str().unwrap()),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let key = owner.create_resolver_key(&project).await;
    let (status, queue) = owner
        .call_with(Some(&key), "GET", "/api/agent/queue", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(queue["total_open"], 1);
    let pending = queue["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["id"], *id);
    assert_eq!(pending[0]["number"], 1);
}

#[tokio::test]
async fn closing_and_reopening_follow_the_permission_matrix() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let reporter = env.app("reporter").await;
    let stranger = env.app("stranger").await;
    let project = owner.create_project("笔记本集群").await;
    let ticket = reporter
        .create_ticket(&project, "节点离线", "两台节点从昨晚开始离线", true)
        .await;
    let id = ticket["id"].as_str().unwrap().to_owned();

    // A stranger cannot close it.
    let (status, body) = stranger
        .post(&format!("/api/tickets/{id}/close"), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    // Nor edit someone else's ticket.
    let (status, _) = stranger
        .patch(
            &format!("/api/tickets/{id}"),
            json!({ "title": "挪作他用" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The reporter closes it; closing twice adds no second audit comment.
    let (status, closed) = reporter
        .post(&format!("/api/tickets/{id}/close"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(closed["status"], "closed");
    assert_eq!(closed["closed_by_kind"], "user");
    let comments_after_first = closed["comments"].as_array().unwrap().len();
    let (status, closed_again) = reporter
        .post(&format!("/api/tickets/{id}/close"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        closed_again["comments"].as_array().unwrap().len(),
        comments_after_first
    );

    // A closed ticket is frozen until it is reopened.
    let (status, _) = reporter
        .patch(&format!("/api/tickets/{id}"), json!({ "body": "补充信息" }))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = stranger
        .post(&format!("/api/tickets/{id}/reopen"), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, reopened) = reporter
        .post(&format!("/api/tickets/{id}/reopen"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(reopened["status"], "open");
    assert!(reopened["closed_at"].is_null());

    // The project owner may also close a ticket filed by someone else.
    let (status, closed) = owner.post(&format!("/api/tickets/{id}/close"), None).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(closed["closed_by_kind"], "user");
}

#[tokio::test]
async fn project_settings_belong_to_the_owner() {
    let env = Env::new().await;
    let owner = env.app("owner").await;
    let other = env.app("other").await;
    let project = owner.create_project("支付网关").await;

    let (status, _) = other
        .patch(
            &format!("/api/projects/{project}"),
            json!({ "name": "改名" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = other
        .post(&format!("/api/projects/{project}/resolver-key"), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A pasted Cybion link is normalised, a bare id is kept, junk is rejected.
    let (status, project_view) = owner
        .patch(
            &format!("/api/projects/{project}"),
            json!({ "resolver_thread_url": "https://cybion.test/#/threads/abc-123" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{project_view}");
    assert_eq!(
        project_view["resolver_thread_url"],
        "https://cybion.test/#/threads/abc-123"
    );
    let (status, project_view) = owner
        .patch(
            &format!("/api/projects/{project}"),
            json!({ "resolver_thread_url": "abc-123" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(project_view["resolver_thread_url"], "abc-123");
    let (status, _) = owner
        .patch(
            &format!("/api/projects/{project}"),
            json!({ "resolver_thread_url": "https://example.com/not-a-thread" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ticket_numbers_are_per_project_and_never_reused() {
    let env = Env::new().await;
    let user = env.app("alice").await;
    let first = user.create_project("项目一").await;
    let second = user.create_project("项目二").await;

    let mut numbers = Vec::new();
    for index in 0..3 {
        let ticket = user
            .create_ticket(&first, &format!("工单 {index}"), "", false)
            .await;
        numbers.push(ticket["number"].as_i64().unwrap());
    }
    assert_eq!(numbers, vec![1, 2, 3]);
    let other = user
        .create_ticket(&second, "另一个项目的工单", "", false)
        .await;
    assert_eq!(other["number"], 1);
}

#[tokio::test]
async fn concurrent_creates_get_distinct_numbers() {
    let env = Env::new().await;
    let app = env.app("alice").await;
    let project = app.create_project("并发").await;

    let mut handles = Vec::new();
    for index in 0..10 {
        let router = app.router.clone();
        let project = project.clone();
        handles.push(tokio::spawn(async move {
            let uri = format!("/api/projects/{project}/tickets");
            let body = json!({ "title": format!("并发工单 {index}"), "body": "" });
            support::send(router, "POST", &uri, Some(body)).await
        }));
    }
    let mut numbers = Vec::new();
    for handle in handles {
        let (status, ticket) = handle.await.expect("任务不应崩溃");
        assert_eq!(status, StatusCode::OK, "{ticket}");
        numbers.push(ticket["number"].as_i64().unwrap());
    }
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=10).collect::<Vec<_>>());
}

#[tokio::test]
async fn browser_routes_require_a_signed_in_user() {
    let env = Env::new().await;
    let anonymous = env.anonymous_app().await;

    let (status, health) = anonymous.get("/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");
    let (status, meta) = anonymous.get("/api/meta").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meta["public_base_url"], PUBLIC_BASE);

    for uri in ["/api/me", "/api/projects", "/api/me/tickets"] {
        let (status, body) = anonymous.get(uri).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri} -> {body}");
        assert_eq!(error_message(&body), "缺少访问令牌");
    }
}
