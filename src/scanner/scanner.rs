use walkdir::WalkDir;
use std::path::Path;
use rusqlite::Connection;
use crate::db::models::MediaType;
use crate::db::queries;

pub fn scan_directory(conn: &Connection, dir: &Path) -> anyhow::Result<()> {
    tracing::info!("Scanning directory for media: {:?}", dir);
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e: walkdir::Result<walkdir::DirEntry>| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s: &std::ffi::OsStr| s.to_str()) {
                let media_type = MediaType::from_extension(ext);
                if matches!(media_type, MediaType::Video | MediaType::Image) {
                    let path_str = path.to_string_lossy().to_string();
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
                    let last_modified = metadata
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    
                    match queries::insert_or_ignore_media(conn, &path_str, &media_type, size, last_modified) {
                        Ok(true) => tracing::debug!("Added new media file: {}", path_str),
                        Ok(false) => {}, // Already exists
                        Err(e) => tracing::error!("Failed to insert media file {}: {}", path_str, e),
                    }
                }
            }
        }
    }
    
    tracing::info!("Finished scanning directory.");
    Ok(())
}
