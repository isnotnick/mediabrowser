pub mod auth;
pub mod routes;
pub mod templates;
pub mod assets;
pub mod hls;
pub mod filters;
pub mod screenshot;

use axum::{Router, middleware};
use tower_http::{services::ServeDir, trace::TraceLayer};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::Mutex;
use crate::config::Settings;
use crate::processing::worker::WorkerState;

pub struct AppState {
    pub settings: Settings,
    pub db_path: PathBuf,
    pub worker_state: Arc<Mutex<WorkerState>>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    // Media and static assets (thumbnails, vtt) shouldn't strictly need auth to simplify video player loading, 
    // but we can put it behind auth if desired. For now, let's put everything behind auth.
    
    Router::new()
        .merge(routes::protected_routes())
        .nest_service("/media", ServeDir::new(&state.settings.media_directory))
        .nest_service("/.app_data", ServeDir::new(".app_data"))
        .route("/assets/{*file}", axum::routing::get(assets::static_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .merge(routes::public_routes())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
