use axum::{
    Router,
    routing::{get, post},
    extract::{State, Form},
    response::{Html, IntoResponse, Redirect, Response},
    http::StatusCode,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use std::sync::Arc;
use serde::Deserialize;
use axum::extract::Path;
use askama::Template;
use rusqlite::Connection;

use crate::server::AppState;
use crate::server::templates::{IndexTemplate, LoginTemplate, ConfigTemplate, WorkerStateDto, ViewTemplate, VideoTemplate};
use crate::db::queries;
use crate::server::hls;
use crate::server::screenshot;
use axum::extract::Query;

pub fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
}

pub fn protected_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/config", get(config_page).post(config_submit))
        .route("/view/{id}", get(view_page))
        .route("/hls/{id}/master.m3u8", get(hls::master_playlist))
        .route("/hls/{id}/playlist.m3u8", get(hls::hls_playlist))
        .route("/hls/{id}/{segment}", get(hls::hls_segment))
        .route("/screenshot/{id}", get(screenshot::screenshot))
}

#[derive(Deserialize)]
struct IndexQuery {
    search: Option<String>,
    sort: Option<String>,
}

async fn index(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IndexQuery>,
) -> impl IntoResponse {
    let conn = Connection::open(&state.db_path).unwrap();
    let files = queries::search_media(&conn, query.search.as_deref(), query.sort.as_deref()).unwrap_or_default();
    
    let template = IndexTemplate { 
        files, 
        search_query: query.search.unwrap_or_default(),
        sort_order: query.sort.unwrap_or_else(|| "last_modified_desc".to_string()),
    };
    Html(template.render().unwrap())
}

async fn view_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = Connection::open(&state.db_path).unwrap();
    let media = match queries::get_media_by_id(&conn, id).unwrap() {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "Media not found").into_response(),
    };
    
    match media.media_type {
        crate::db::models::MediaType::Video => {
            let template = VideoTemplate { file: media };
            Html(template.render().unwrap()).into_response()
        },
        _ => {
            let template = ViewTemplate { file: media };
            Html(template.render().unwrap()).into_response()
        }
    }
}

async fn config_page(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let worker_state = {
        let ws = state.worker_state.lock().await;
        WorkerStateDto {
            pending_count: ws.pending_count,
            processing_count: ws.processing_count,
            completed_count: ws.completed_count,
            failed_count: ws.failed_count,
        }
    };
    
    let template = ConfigTemplate {
        settings: state.settings.clone(),
        worker_state,
    };
    Html(template.render().unwrap())
}

#[derive(Deserialize)]
struct ConfigUpdate {
    server_port: u16,
    thumbnail_size: u32,
    thumbnail_quality: u8,
    generate_timeline_thumbnails: Option<String>, // HTML checkboxes send "on" if checked, nothing if unchecked
}

async fn config_submit(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<ConfigUpdate>,
) -> Redirect {
    let mut current_settings = state.settings.clone();
    current_settings.server_port = payload.server_port;
    current_settings.thumbnail_size = payload.thumbnail_size;
    current_settings.thumbnail_quality = payload.thumbnail_quality;
    current_settings.generate_timeline_thumbnails = payload.generate_timeline_thumbnails.is_some();
    
    let _ = current_settings.save(std::path::Path::new("config.yml"));
    
    Redirect::to("/config")
}

async fn login_page() -> impl IntoResponse {
    let template = LoginTemplate {};
    Html(template.render().unwrap())
}

#[derive(Deserialize)]
struct LoginPayload {
    password: String,
}

async fn login_submit(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Form(payload): Form<LoginPayload>,
) -> Result<(CookieJar, Redirect), Response> {
    if let Some(expected_pass) = &state.settings.optional_password {
        if expected_pass == &payload.password {
            let cookie = Cookie::build(("auth_session", "authenticated"))
                .path("/")
                .http_only(true)
                .build();
            return Ok((jar.add(cookie), Redirect::to("/")));
        }
    }
    
    // If no password configured, or wrong password
    Err((StatusCode::UNAUTHORIZED, "Invalid password").into_response())
}
