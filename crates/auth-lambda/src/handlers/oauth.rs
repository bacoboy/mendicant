use axum::Router;
use axum::routing::post;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/device", post(device_authorize))
        .route("/oauth/token", post(device_token))
        .route("/activate", post(activate_complete))
}

/// RFC 8628 §3.1 — device authorization request (called by CLI).
/// Returns device_code, user_code, verification_uri, expires_in, interval.
async fn device_authorize() -> &'static str {
    todo!("create DeviceGrant, store in oauth_devices table, return JSON")
}

/// RFC 8628 §3.4 — device access token request (polled by CLI).
/// Returns access_token + refresh_token when approved, or authorization_pending / expired_token.
async fn device_token() -> &'static str {
    todo!("poll DeviceGrant status, issue tokens if approved")
}

/// Called by the browser after the user authenticates and enters their user_code.
/// Returns a Datastar SSE stream confirming approval.
async fn activate_complete() -> &'static str {
    todo!("verify user_code, mark DeviceGrant approved, return SSE confirmation")
}
