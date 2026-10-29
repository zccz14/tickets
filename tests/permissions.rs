mod support;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::*;

async fn ids(app: &App) -> Vec<String> {
    let (status, projects) = app.get("/api/projects").await;
    assert_eq!(status, StatusCode::OK, "{projects}");
    projects
        .as_array()
        .unwrap()
        .iter()
        .map(|project| project["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn new_projects_are_private_and_hidden_from_others() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;

    // The default is private even when the client says nothing.
    let (status, project) = alice
        .post("/api/projects", Some(json!({ "name": "支付网关" })))
        .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    assert_eq!(project["visibility"], "private");
    assert_eq!(project["is_member"], false);
    let id = project["id"].as_str().unwrap().to_owned();

    assert!(ids(&alice).await.contains(&id));
    assert!(
        !ids(&bob).await.contains(&id),
        "私有项目不应出现在别人的列表里"
    );

    // A private project answers 404 instead of 403 so its existence stays hidden.
    let (status, body) = bob.get(&format!("/api/projects/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error_message(&body), "项目不存在");

    let (status, _) = bob.get(&format!("/api/projects/{id}/tickets")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = bob
        .post(
            &format!("/api/projects/{id}/tickets"),
            Some(json!({ "title": "偷偷建一张", "body": "", "submit": true })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = bob.get(&format!("/api/projects/{id}/grants")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_grant_opens_a_private_project_for_one_user() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;
    let id = alice.create_project_with("支付网关", "private").await;

    let (status, body) = alice
        .post(
            &format!("/api/projects/{id}/grants"),
            Some(json!({ "user_id": "bob" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert!(ids(&bob).await.contains(&id), "被授权的用户应看到私有项目");
    let (status, view) = bob.get(&format!("/api/projects/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["is_member"], true);
    assert_eq!(view["is_owner"], false);

    // The granted member can file and read tickets in that project.
    let ticket = bob
        .create_ticket(&id, "回调超时", "3% 的支付回调超过 30 秒", true)
        .await;
    let ticket_id = ticket["id"].as_str().unwrap().to_owned();
    let (status, seen) = alice.get(&format!("/api/tickets/{ticket_id}")).await;
    assert_eq!(status, StatusCode::OK, "{seen}");
    assert_eq!(seen["is_project_owner"], true);

    // Membership is not ownership: project settings, grants and the resolver
    // stay with the owner.
    let (status, _) = bob
        .patch(&format!("/api/projects/{id}"), json!({ "name": "改名" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob
        .post(
            &format!("/api/projects/{id}/grants"),
            Some(json!({ "user_id": "carol" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob
        .post(&format!("/api/projects/{id}/resolver-key"), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Revoking the grant closes the door again.
    let (status, removed) = alice
        .call("DELETE", &format!("/api/projects/{id}/grants/bob"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["removed"], true);
    let (status, _) = bob.get(&format!("/api/projects/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = bob.get(&format!("/api/tickets/{ticket_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn public_projects_are_open_to_every_signed_in_user() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;

    let (status, project) = alice
        .post(
            "/api/projects",
            Some(json!({ "name": "公开看板", "visibility": "public" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    assert_eq!(project["visibility"], "public");
    let id = project["id"].as_str().unwrap().to_owned();

    assert!(ids(&bob).await.contains(&id));
    let (status, view) = bob.get(&format!("/api/projects/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["is_member"], false);

    let ticket = bob
        .create_ticket(&id, "页面白屏", "打开项目页什么都没有", true)
        .await;
    let ticket_id = ticket["id"].as_str().unwrap().to_owned();
    let (status, body) = bob
        .post(
            &format!("/api/tickets/{ticket_id}/comments"),
            Some(json!({ "body": "补一张截图" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Public means "can read and report", not "can administer".
    let (status, _) = bob
        .patch(
            &format!("/api/projects/{id}"),
            json!({ "visibility": "private" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bob.get(&format!("/api/projects/{id}/grants")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn switching_a_project_back_to_private_hides_existing_tickets() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;
    let id = alice.create_project("公开看板").await;
    let ticket = bob.create_ticket(&id, "页面白屏", "", true).await;
    let ticket_id = ticket["id"].as_str().unwrap().to_owned();

    let (_, mine) = bob.get("/api/me/tickets").await;
    assert!(
        mine.as_array()
            .unwrap()
            .iter()
            .any(|ticket| ticket["id"] == ticket_id)
    );

    let (status, body) = alice
        .patch(
            &format!("/api/projects/{id}"),
            json!({ "visibility": "private" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["visibility"], "private");

    let (status, _) = bob.get(&format!("/api/tickets/{ticket_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, mine) = bob.get("/api/me/tickets").await;
    assert!(
        !mine
            .as_array()
            .unwrap()
            .iter()
            .any(|ticket| ticket["id"] == ticket_id),
        "失去访问权后，我的工单里也不该再出现它"
    );
    // The owner still sees everything.
    let (status, seen) = alice.get(&format!("/api/tickets/{ticket_id}")).await;
    assert_eq!(status, StatusCode::OK, "{seen}");
}

#[tokio::test]
async fn grant_management_is_owner_only_and_idempotent() {
    let env = Env::new().await;
    let alice = env.app("alice").await;
    let bob = env.app("bob").await;
    let id = alice.create_project_with("支付网关", "private").await;

    let (status, _) = bob.get(&format!("/api/projects/{id}/grants")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "非成员连项目都看不到");

    // Granting the owner is pointless and rejected.
    let (status, body) = alice
        .post(
            &format!("/api/projects/{id}/grants"),
            Some(json!({ "user_id": "alice" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, _) = alice
        .post(
            &format!("/api/projects/{id}/grants"),
            Some(json!({ "user_id": "bob" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = bob.get(&format!("/api/projects/{id}/grants")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "成员可以看项目，但不能管成员"
    );
    let (status, body) = alice
        .post(
            &format!("/api/projects/{id}/grants"),
            Some(json!({ "user_id": "bob" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (_, grants) = alice.get(&format!("/api/projects/{id}/grants")).await;
    let listed: Vec<Value> = grants["grants"].as_array().unwrap().clone();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["user_id"], "bob");
}
