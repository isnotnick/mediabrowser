use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;

use crate::db::{queries, models::MediaType};
use crate::config::Settings;
use super::ffmpeg;

pub struct WorkerState {
    pub pending_count: u32,
    pub processing_count: u32,
    pub completed_count: u32,
    pub failed_count: u32,
}

pub async fn start_worker(
    db_path: PathBuf,
    settings: Settings,
    state: Arc<Mutex<WorkerState>>,
) {
    let thumbs_dir = Path::new(".app_data").join("thumbnails");
    let vtt_dir = Path::new(".app_data").join("vtt");
    
    std::fs::create_dir_all(&thumbs_dir).unwrap_or_default();
    std::fs::create_dir_all(&vtt_dir).unwrap_or_default();

    loop {
        // Run in a block to isolate DB connection
        let pending_files = {
            let conn = Connection::open(&db_path).unwrap();
            queries::get_pending_media(&conn).unwrap_or_default()
        };

        {
            let mut s = state.lock().await;
            s.pending_count = pending_files.len() as u32;
        }

        if pending_files.is_empty() {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        for file in pending_files {
            {
                let mut s = state.lock().await;
                s.pending_count = s.pending_count.saturating_sub(1);
                s.processing_count += 1;
            }

            tracing::info!("Processing file: {}", file.path);
            let conn = Connection::open(&db_path).unwrap();
            let _ = queries::update_process_status(&conn, file.id, "processing");

            let is_video = matches!(file.media_type, MediaType::Video);
            let thumb_filename = format!("{}.jpg", file.id);
            let thumb_path = thumbs_dir.join(&thumb_filename);
            
            let video_duration = if is_video {
                ffmpeg::get_video_duration(&file.path).ok()
            } else {
                None
            };
            
            let res = ffmpeg::generate_thumbnail(
                &file.path,
                thumb_path.to_str().unwrap(),
                is_video,
                settings.thumbnail_size,
                settings.thumbnail_quality,
                video_duration,
            );

            match res {
                Ok(_) => {
                    let _ = queries::update_thumbnail(&conn, file.id, thumb_path.to_str().unwrap());
                    
                    // If video, try generating VTT only if enabled
                    if is_video && settings.generate_timeline_thumbnails {
                        if let Some(duration) = video_duration {
                            let vtt_img_filename = format!("{}_sprite.jpg", file.id);
                            let vtt_filename = format!("{}.vtt", file.id);
                            
                            let vtt_img_path = vtt_dir.join(&vtt_img_filename);
                            let vtt_path = vtt_dir.join(&vtt_filename);
                            
                            if let Ok(_) = ffmpeg::generate_vtt_sprite_sheet(
                                &file.path,
                                vtt_img_path.to_str().unwrap(),
                                vtt_path.to_str().unwrap(),
                                duration,
                                160, // Default sprite tile size
                            ) {
                                let _ = queries::update_vtt(&conn, file.id, vtt_path.to_str().unwrap());
                            }
                        }
                    }

                    let _ = queries::update_process_status(&conn, file.id, "completed");
                    let mut s = state.lock().await;
                    s.processing_count = s.processing_count.saturating_sub(1);
                    s.completed_count += 1;
                },
                Err(e) => {
                    tracing::error!("Failed to process {}: {:?}", file.path, e);
                    let _ = queries::update_process_status(&conn, file.id, "failed");
                    let mut s = state.lock().await;
                    s.processing_count = s.processing_count.saturating_sub(1);
                    s.failed_count += 1;
                }
            }
        }
    }
}
