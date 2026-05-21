use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use std::sync::Arc;

use crate::server::AppState;

pub async fn auth_middleware(
    cookie_jar: CookieJar,
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // If no password is set, allow all
    if state.settings.optional_password.is_none() {
        return Ok(next.run(request).await);
    }
    
    // Check if on login path to avoid redirect loop
    if request.uri().path() == "/login" {
        return Ok(next.run(request).await);
    }

    let is_authenticated = cookie_jar
        .get("auth_session")
        .map(|cookie| cookie.value() == "authenticated")
        .unwrap_or(false);

    if is_authenticated {
        Ok(next.run(request).await)
    } else {
        Err(Redirect::to("/login").into_response())
    }
}
