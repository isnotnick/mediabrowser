use axum::{
    extract::{Path as AxumPath, State},
    response::{IntoResponse, Response},
    http::{StatusCode, header},
};
use std::sync::Arc;
use tokio::process::Command;
use std::process::Stdio;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::server::AppState;
use crate::db::queries;
use crate::processing::ffmpeg::find_ffmpeg;
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
    
    // We'll generate segments dynamically. First, create a master playlist pointing to our dynamic segment endpoint.
    // For simplicity, we create a single stream.
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
    
    // If playlist doesn't exist, start ffmpeg to generate the stream
    if !playlist_path.exists() {
        let ffmpeg = find_ffmpeg().unwrap();
        
        let mut cmd = Command::new(ffmpeg);
        cmd.args([
            "-i", &media.path,
            "-c:v", "copy",
            "-c:a", "aac",
            "-f", "hls",
            "-hls_time", "10",
            "-hls_list_size", "0",
            "-hls_playlist_type", "event",
            // Use force_key_frames to ensure segments are split even if video lacks keyframes
            "-force_key_frames", "expr:gte(t,n_forced*10)",
            "-hls_segment_filename", hls_dir.join("segment_%03d.ts").to_str().unwrap(),
            playlist_path.to_str().unwrap()
        ]);
        
        cmd.stdout(Stdio::null())
           .stderr(Stdio::null());
           
        // Start process in background
        let child = cmd.spawn();
        if child.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to start ffmpeg").into_response();
        }
        
        // Wait a bit for the playlist to be generated
        for _ in 0..20 {
            if playlist_path.exists() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
    
    match fs::read_to_string(&playlist_path).await {
        Ok(content) => {
            // Rewrite segment paths in the playlist to point to our endpoint
            let mut rewritten = String::new();
            for line in content.lines() {
                if line.starts_with("segment_") {
                    rewritten.push_str(&format!("/hls/{}/{}\n", id, line));
                } else {
                    rewritten.push_str(line);
                    rewritten.push('\n');
                }
            }
            ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], rewritten).into_response()
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read playlist").into_response(),
    }
}

pub async fn hls_segment(
    State(_state): State<Arc<AppState>>,
    AxumPath((id, segment)): AxumPath<(i64, String)>,
) -> impl IntoResponse {
    let segment_path = Path::new(".app_data").join("hls").join(id.to_string()).join(segment);
    
    // Wait slightly if segment is not yet available but ffmpeg is running
    for _ in 0..50 {
        if segment_path.exists() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    match fs::read(&segment_path).await {
        Ok(data) => ([(header::CONTENT_TYPE, "video/MP2T")], data).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Segment not found").into_response(),
    }
}
