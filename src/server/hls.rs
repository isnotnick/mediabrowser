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
    
    // We will serve a custom generated VOD playlist that has the full duration instantly.
    // ffmpeg will write to ffmpeg.m3u8, but we won't serve it.
    let playlist_path = hls_dir.join("playlist.m3u8");
    let ffmpeg_playlist_path = hls_dir.join("ffmpeg.m3u8");
    
    if !playlist_path.exists() {
        // Fetch duration
        let duration = get_video_duration(&media.path).unwrap_or(0.0);
        
        // Generate our static VOD playlist
        let mut rewritten = String::new();
        rewritten.push_str("#EXTM3U\n");
        rewritten.push_str("#EXT-X-VERSION:3\n");
        rewritten.push_str("#EXT-X-TARGETDURATION:10\n");
        rewritten.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        rewritten.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        
        let segments = (duration / 10.0).ceil() as u32;
        let mut remaining_duration = duration;
        
        for i in 0..segments {
            let seg_duration = if remaining_duration >= 10.0 { 10.0 } else { remaining_duration };
            remaining_duration -= 10.0;
            
            rewritten.push_str(&format!("#EXTINF:{:.6},\n", seg_duration));
            rewritten.push_str(&format!("/hls/{}/segment_{:03}.ts\n", id, i));
        }
        rewritten.push_str("#EXT-X-ENDLIST\n");
        
        let _ = fs::write(&playlist_path, &rewritten).await;
        
        // Start ffmpeg in the background
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
            "-force_key_frames", "expr:gte(t,n_forced*10)",
            "-hls_segment_filename", hls_dir.join("segment_%03d.ts").to_str().unwrap(),
            ffmpeg_playlist_path.to_str().unwrap()
        ]);
        
        cmd.stdout(Stdio::null())
           .stderr(Stdio::null());
           
        let _ = cmd.spawn();
    }
    
    match fs::read_to_string(&playlist_path).await {
        Ok(content) => {
            ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], content).into_response()
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read playlist").into_response(),
    }
}

pub async fn hls_segment(
    State(_state): State<Arc<AppState>>,
    AxumPath((id, segment)): AxumPath<(i64, String)>,
) -> impl IntoResponse {
    let hls_dir = Path::new(".app_data").join("hls").join(id.to_string());
    let segment_path = hls_dir.join(&segment);
    let ffmpeg_playlist_path = hls_dir.join("ffmpeg.m3u8");
    
    // Wait for the segment to be fully written by checking if ffmpeg has published it to its internal m3u8
    for _ in 0..150 { // Wait up to 15 seconds for transcode to catch up
        if let Ok(content) = fs::read_to_string(&ffmpeg_playlist_path).await {
            // ffmpeg writes the segment to disk, then updates the m3u8 file.
            // If the segment filename appears in the m3u8, it is completely written and safe to serve.
            if content.contains(&segment) {
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    match fs::read(&segment_path).await {
        Ok(data) => ([(header::CONTENT_TYPE, "video/MP2T")], data).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Segment not found").into_response(),
    }
}
