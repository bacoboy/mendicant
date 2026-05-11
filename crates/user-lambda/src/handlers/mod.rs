use axum::Router;

use crate::state::AppState;

#[allow(dead_code)]
pub(crate) struct NavUser {
    pub email: String,
    pub is_admin: bool,
}

mod admin;
mod credentials;
mod passkey;
mod profile;
mod sessions;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(profile::routes())
        .merge(passkey::routes())
        .merge(sessions::routes())
        .merge(credentials::routes())
        .merge(admin::routes())
}

pub(crate) fn is_secure_context() -> bool {
    std::env::var("ENVIRONMENT")
        .map(|e| e != "dev")
        .unwrap_or(true)
}

pub(crate) fn extract_refresh_jti(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_hdr = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie_hdr.split(';') {
        let part = part.trim();
        if let Some(jti) = part.strip_prefix("refresh_token=") {
            return Some(jti.to_string());
        }
    }
    None
}
