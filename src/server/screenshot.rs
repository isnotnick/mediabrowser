use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    http::{StatusCode, header, HeaderMap, HeaderValue},
};
use std::sync::Arc;
use std::path::Path as StdPath;
use tokio::process::Command;
use tokio::fs;
use std::process::Stdio;
use serde::Deserialize;

use crate::server::AppState;
use crate::db::queries;
use crate::processing::ffmpeg::find_ffmpeg;
use rusqlite::Connection;

#[derive(Deserialize)]
pub struct ScreenshotParams {
    pub t: Option<f64>,
}

pub async fn screenshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<ScreenshotParams>,
) -> impl IntoResponse {
    let conn = Connection::open(&state.db_path).unwrap();
    let media = match queries::get_media_by_id(&conn, id).unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };

    let timestamp = params.t.unwrap_or(0.0).max(0.0);

    // Round to nearest second for the cache key so scrubbing reuses frames.
    let cache_key = timestamp.round() as i64;
    let cache_dir = StdPath::new(".app_data").join("screenshots").join(id.to_string());
    let _ = fs::create_dir_all(&cache_dir).await;
    let cache_path = cache_dir.join(format!("{}.jpg", cache_key));

    if cache_path.exists() {
        if let Ok(data) = fs::read(&cache_path).await {
            return jpeg_response(data);
        }
    }

    let ffmpeg = match find_ffmpeg() {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg not found").into_response()
        }
    };

    let output = Command::new(ffmpeg)
        .args([
            "-ss",
            &format!("{:.3}", timestamp),
            "-i",
            &media.path,
            "-vframes",
            "1",
            "-f",
            "image2",
            "-vcodec",
            "mjpeg",
            "-q:v",
            "2",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            let _ = fs::write(&cache_path, &o.stdout).await;
            jpeg_response(o.stdout)
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Screenshot failed").into_response(),
    }
}

fn jpeg_response(data: Vec<u8>) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    // Allow the browser to cache frames for an hour; scrubbing reuses them without re-fetching.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    (headers, data).into_response()
}
