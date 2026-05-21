use std::path::PathBuf;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum MediaType {
    Video,
    Image,
    Unknown,
}

impl MediaType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "webm" => MediaType::Video,
            "jpg" | "jpeg" | "png" | "gif" | "webp" => MediaType::Image,
            _ => MediaType::Unknown,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            MediaType::Video => "video",
            MediaType::Image => "image",
            MediaType::Unknown => "unknown",
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s {
            "video" => MediaType::Video,
            "image" => MediaType::Image,
            _ => MediaType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaFile {
    pub id: i64,
    pub path: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    pub thumbnail_path: Option<String>,
    pub vtt_path: Option<String>,
    pub process_status: String, // e.g. "pending", "processing", "completed", "failed"
    pub last_modified: i64,
}
