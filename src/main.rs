use crate::config::Settings;
use anyhow::Result;
use std::path::{Path, PathBuf};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

mod config;
mod db;
mod scanner;
mod processing;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config_path = Path::new("config.yml");
    let settings = Settings::load_or_create(config_path)?;

    println!("Starting with settings: {:?}", settings);

    // Initialize Database
    let db_path = Path::new("media.db");
    let conn = Connection::open(db_path)?;
    db::schema::init_db(&conn)?;
    
    // Ensure media directory exists
    if !settings.media_directory.exists() {
        std::fs::create_dir_all(&settings.media_directory)?;
    }

    // Initial Scan
    scanner::scanner::scan_directory(&conn, &settings.media_directory)?;

    // Start file watcher
    let mut watcher_rx = scanner::watcher::start_watcher(&settings.media_directory)?;
    
    // Process watcher events in background
    let db_path_clone = db_path.to_path_buf();
    tokio::spawn(async move {
        let conn = Connection::open(&db_path_clone).unwrap();
        while let Some(event) = watcher_rx.recv().await {
            match event {
                scanner::watcher::WatcherEvent::Created(path) => {
                    tracing::info!("File created: {:?}", path);
                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            let media_type = db::models::MediaType::from_extension(ext);
                            if matches!(media_type, db::models::MediaType::Video | db::models::MediaType::Image) {
                                let path_str = path.to_string_lossy().to_string();
                                let metadata = std::fs::metadata(&path).ok();
                                let size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
                                let last_modified = metadata
                                    .and_then(|m| m.modified().ok())
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as i64)
                                    .unwrap_or(0);
                                let _ = db::queries::insert_or_ignore_media(&conn, &path_str, &media_type, size, last_modified);
                            }
                        }
                    }
                },
                scanner::watcher::WatcherEvent::Removed(path) => {
                    tracing::info!("File removed: {:?}", path);
                    let path_str = path.to_string_lossy().to_string();
                    let _ = db::queries::remove_media_by_path(&conn, &path_str);
                },
                scanner::watcher::WatcherEvent::Modified(_) => {}
            }
        }
    });

    // Start processing worker
    let worker_state = Arc::new(Mutex::new(processing::worker::WorkerState {
        pending_count: 0,
        processing_count: 0,
        completed_count: 0,
        failed_count: 0,
    }));
    
    let worker_state_clone = worker_state.clone();
    let db_path_buf = db_path.to_path_buf();
    let settings_clone = settings.clone();
    tokio::spawn(async move {
        processing::worker::start_worker(db_path_buf, settings_clone, worker_state_clone).await;
    });

    // Start Web Server
    let port = settings.server_port;
    let app_state = Arc::new(server::AppState {
        settings,
        db_path: db_path.to_path_buf(),
        worker_state,
    });
    
    let app = server::create_router(app_state);
    
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Server listening on 0.0.0.0:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
