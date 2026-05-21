use askama::Template;
use crate::db::models::MediaFile;
use crate::config::Settings;
use crate::server::filters;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub files: Vec<MediaFile>,
    pub search_query: String,
    pub sort_order: String,
}

#[derive(Template)]
#[template(path = "config.html")]
pub struct ConfigTemplate {
    pub settings: Settings,
    pub worker_state: WorkerStateDto,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {}

#[derive(Template)]
#[template(path = "view.html")]
pub struct ViewTemplate {
    pub file: MediaFile,
}

#[derive(Template)]
#[template(path = "video.html")]
pub struct VideoTemplate {
    pub file: MediaFile,
}

pub struct WorkerStateDto {
    pub pending_count: u32,
    pub processing_count: u32,
    pub completed_count: u32,
    pub failed_count: u32,
}
