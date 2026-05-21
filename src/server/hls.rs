use axum::{
    extract::{Path as AxumPath, State},
    response::IntoResponse,
    http::{StatusCode, header},
};
use std::sync::Arc;
use tokio::process::Command;
use std::process::Stdio;
use std::path::Path;
use tokio::fs;

use crate::server::AppState;
use crate::db::queries;
use crate::processing::ffmpeg::{find_ffmpeg, get_video_duration};
use rusqlite::Connection;

pub async fn master_playlist(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    let conn = Connection::open(&state.db_path).unwrap();
    let media = match queries::get_media_by_id(&conn, id).unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };

    let m3u8 = format!(
        "#EXTM3U\n#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=2000000\n/hls/{}/playlist.m3u8\n",
        media.id
    );

    ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], m3u8).into_response()
}

pub async fn hls_playlist(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> impl IntoResponse {
    let conn = Connection::open(&state.db_path).unwrap();
    let media = match queries::get_media_by_id(&conn, id).unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };

    let hls_dir = Path::new(".app_data").join("hls").join(id.to_string());
    let _ = fs::create_dir_all(&hls_dir).await;

    let playlist_path = hls_dir.join("playlist.m3u8");

    if !playlist_path.exists() {
        let duration = get_video_duration(&media.path).unwrap_or(0.0);

        let mut content = String::new();
        content.push_str("#EXTM3U\n");
        content.push_str("#EXT-X-VERSION:3\n");
        content.push_str("#EXT-X-TARGETDURATION:10\n");
        content.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        content.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");

        let segments = (duration / 10.0).ceil() as u32;
        let mut remaining = duration;

        for i in 0..segments {
            let seg_dur = if remaining >= 10.0 { 10.0 } else { remaining };
            remaining -= 10.0;
            content.push_str(&format!("#EXTINF:{:.6},\n", seg_dur));
            content.push_str(&format!("/hls/{}/segment_{:03}.ts\n", id, i));
        }
        content.push_str("#EXT-X-ENDLIST\n");

        let _ = fs::write(&playlist_path, &content).await;
    }

    match fs::read_to_string(&playlist_path).await {
        Ok(content) => {
            ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], content).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read playlist").into_response(),
    }
}

pub async fn hls_segment(
    State(state): State<Arc<AppState>>,
    AxumPath((id, segment)): AxumPath<(i64, String)>,
) -> impl IntoResponse {
    let hls_dir = Path::new(".app_data").join("hls").join(id.to_string());
    let _ = fs::create_dir_all(&hls_dir).await;
    let segment_path = hls_dir.join(&segment);

    // Serve from cache immediately
    if segment_path.exists() {
        if let Ok(data) = fs::read(&segment_path).await {
            return ([(header::CONTENT_TYPE, "video/MP2T")], data).into_response();
        }
    }

    // Parse segment number to calculate seek time
    let seg_num: u32 = segment
        .strip_prefix("segment_")
        .and_then(|s| s.strip_suffix(".ts"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let seek_time = seg_num as f64 * 10.0;

    let media = {
        let conn = Connection::open(&state.db_path).unwrap();
        match queries::get_media_by_id(&conn, id).unwrap() {
            Some(m) => m,
            None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
        }
    };

    let ffmpeg = match find_ffmpeg() {
        Ok(p) => p,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg not found").into_response(),
    };

    // Transcode directly to a temp file; rename atomically when done.
    // Using a unique temp name avoids corrupting concurrent requests for the same segment.
    let temp_path = hls_dir.join(format!(".{}.partial", segment));

    let output = Command::new(ffmpeg)
        .args([
            "-ss", &format!("{:.3}", seek_time),
            "-i", &media.path,
            "-t", "10",
            "-c:v", "libx264",
            "-preset", "ultrafast",
            "-c:a", "aac",
            "-f", "mpegts",
            "-y",
            temp_path.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            // Atomic rename: on Linux rename() is atomic so concurrent requests are safe
            let _ = fs::rename(&temp_path, &segment_path).await;
            match fs::read(&segment_path).await {
                Ok(data) => ([(header::CONTENT_TYPE, "video/MP2T")], data).into_response(),
                Err(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read segment").into_response()
                }
            }
        }
        _ => {
            let _ = fs::remove_file(&temp_path).await;
            (StatusCode::INTERNAL_SERVER_ERROR, "Segment transcoding failed").into_response()
        }
    }
}
