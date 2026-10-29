use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist/"]
struct Assets;

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

/// Serves the embedded SPA. Unknown paths fall back to `index.html` so the hash
/// router owns every route; `/api/*` never reaches this handler because the API
/// router matches first.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(asset) = Assets::get(path) {
        return response(path, asset.data.into_owned());
    }
    match Assets::get("index.html") {
        Some(asset) => response("index.html", asset.data.into_owned()),
        None => (StatusCode::NOT_FOUND, "UI 资源缺失").into_response(),
    }
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
        ],
        Body::from(bytes),
    )
        .into_response()
}
