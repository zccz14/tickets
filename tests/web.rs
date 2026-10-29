//! The embedded SPA: navigation routes fall back to `index.html`, while a
//! missing bundle file stays a 404 so a stale asset hash cannot turn into a
//! silently unstyled page.

mod support;

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn fetch(app: &support::App, uri: &str) -> (StatusCode, HeaderMap, String) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn header_of(headers: &HeaderMap, name: header::HeaderName) -> &str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn navigation_routes_serve_the_page() {
    let app = support::Env::new().await.anonymous_app().await;
    for uri in ["/", "/projects", "/projects/abc/tickets/1"] {
        let (status, headers, body) = fetch(&app, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} -> {body}");
        assert_eq!(
            header_of(&headers, header::CONTENT_TYPE),
            "text/html; charset=utf-8"
        );
        assert_eq!(header_of(&headers, header::CACHE_CONTROL), "no-cache");
        assert_eq!(
            header_of(&headers, header::X_CONTENT_TYPE_OPTIONS),
            "nosniff"
        );
        assert!(body.contains(r#"<div id="root">"#), "{uri} -> {body}");
    }
}

#[tokio::test]
async fn the_page_only_references_embedded_assets() {
    let app = support::Env::new().await.anonymous_app().await;
    let (status, _, page) = fetch(&app, "/").await;
    assert_eq!(status, StatusCode::OK);

    let mut assets: Vec<String> = Vec::new();
    for marker in ["src=\"/assets/", "href=\"/assets/"] {
        let mut rest = page.as_str();
        while let Some(start) = rest.find(marker) {
            rest = &rest[start + marker.len() - "/assets/".len()..];
            let end = rest.find('"').expect("属性值结束");
            assets.push(rest[..end].to_owned());
            rest = &rest[end..];
        }
    }
    assert!(
        !assets.is_empty(),
        "首页应当引用至少一个 assets 文件: {page}"
    );

    for asset in assets {
        let (status, headers, body) = fetch(&app, &asset).await;
        assert_eq!(status, StatusCode::OK, "{asset} 未内嵌: {body}");
        assert_eq!(
            header_of(&headers, header::CACHE_CONTROL),
            "public, max-age=31536000, immutable",
            "{asset}"
        );
        assert_ne!(
            header_of(&headers, header::CONTENT_TYPE),
            "text/html; charset=utf-8"
        );
    }
}

#[tokio::test]
async fn a_missing_asset_is_a_404_not_the_page() {
    let app = support::Env::new().await.anonymous_app().await;
    for uri in [
        "/assets/index-deadbeef.css",
        "/assets/vendor-deadbeef.js",
        "/favicon.ico",
    ] {
        let (status, headers, body) = fetch(&app, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri} -> {body}");
        assert_eq!(
            header_of(&headers, header::CACHE_CONTROL),
            "no-store",
            "{uri}"
        );
        assert!(
            !body.contains("<div id=\"root\">"),
            "{uri} 返回了页面: {body}"
        );
    }
}

#[tokio::test]
async fn unknown_api_paths_answer_json() {
    let app = support::Env::new().await.anonymous_app().await;
    let (status, headers, body) = fetch(&app, "/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        header_of(&headers, header::CONTENT_TYPE),
        "application/json"
    );
    assert_eq!(body, r#"{"error":"接口不存在"}"#);
}
