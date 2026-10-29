use axum::{
    body::Body,
    http::{HeaderName, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist/"]
struct Assets;

const NOSNIFF: HeaderName = HeaderName::from_static("x-content-type-options");

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "map" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serves the embedded SPA.
///
/// Only a path that reads like a navigation route falls back to `index.html`.
/// A missing file under `assets/`, or any path whose last segment carries an
/// extension, answers 404 instead: the browser asked for CSS or JavaScript, and
/// handing back HTML leaves the page unstyled. That is exactly what a client
/// still holding a previous bundle's asset hash hits after a deploy replaced
/// the files, so the fallback must never swallow it.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(asset) = Assets::get(path) {
        return response(path, asset.data.into_owned());
    }
    if path.starts_with("api/") {
        return missing_api();
    }
    if looks_like_file(path) {
        return missing_file(path);
    }
    match Assets::get("index.html") {
        Some(asset) => response("index.html", asset.data.into_owned()),
        None => (StatusCode::NOT_FOUND, "UI 资源缺失").into_response(),
    }
}

/// A bundle file, a favicon, a source map — anything the page loads by URL.
fn looks_like_file(path: &str) -> bool {
    path.rsplit('/').next().unwrap_or_default().contains('.')
}

fn missing_file(path: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (NOSNIFF, "nosniff".to_owned()),
        ],
        Body::from(format!("资源不存在: {path}")),
    )
        .into_response()
}

fn missing_api() -> Response {
    (
        StatusCode::NOT_FOUND,
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Body::from(r#"{"error":"接口不存在"}"#),
    )
        .into_response()
}

fn response(path: &str, bytes: Vec<u8>) -> Response {
    let immutable = path.starts_with("assets/");
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, content_type(path).to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (NOSNIFF, "nosniff".to_owned()),
        ],
        Body::from(bytes),
    )
        .into_response()
}
