use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../ui/dist"]
struct UiAssets;

pub async fn serve_ui(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    if raw == "api" || raw.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = if raw.is_empty() { "index.html" } else { raw };
    if let Some(file) = UiAssets::get(path) {
        return file_response(path, file.data.as_ref());
    }
    if let Some(file) = UiAssets::get("index.html") {
        return file_response("index.html", file.data.as_ref());
    }
    placeholder()
}

fn file_response(path: &str, data: &[u8]) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    (
        [(header::CONTENT_TYPE, mime.essence_str().to_string())],
        data.to_vec(),
    )
        .into_response()
}

fn placeholder() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><meta charset=utf-8><title>Orbitry</title><body style=\"font-family:system-ui;background:#09090d;color:#f4f4f7;padding:48px\"><h1>Orbitry</h1><p>The UI was not embedded in this build.</p></body>",
    )
        .into_response()
}
