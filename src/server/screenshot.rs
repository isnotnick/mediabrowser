use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    http::{StatusCode, header, HeaderMap, HeaderValue},
};
use std::sync::Arc;
use tokio::process::Command;
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

    let ffmpeg = match find_ffmpeg() {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg not found").into_response()
        }
    };

    // Fast input seek, then grab exactly one frame piped as JPEG to stdout
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
            let filename = format!("screenshot_{}.jpg", format_timestamp(timestamp));
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                    .unwrap_or(HeaderValue::from_static("attachment")),
            );
            (headers, o.stdout).into_response()
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Screenshot failed").into_response(),
    }
}

fn format_timestamp(seconds: f64) -> String {
    let h = (seconds / 3600.0) as u32;
    let m = ((seconds % 3600.0) / 60.0) as u32;
    let s = (seconds % 60.0) as u32;
    format!("{:02}h{:02}m{:02}s", h, m, s)
}
