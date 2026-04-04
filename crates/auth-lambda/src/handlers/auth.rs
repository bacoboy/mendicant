use axum::Router;
use axum::routing::post;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register/begin", post(register_begin))
        .route("/auth/register/complete", post(register_complete))
        .route("/auth/login/begin", post(login_begin))
        .route("/auth/login/complete", post(login_complete))
}

/// Returns a WebAuthn registration challenge as a Datastar SSE stream
/// that patches the page signals with the challenge options.
async fn register_begin() -> &'static str {
    todo!("generate WebAuthn registration challenge, store in challenges table, return SSE")
}

/// Verifies the WebAuthn registration response, stores the credential,
/// creates the user if needed, and issues tokens.
async fn register_complete() -> &'static str {
    todo!("verify registration, store credential and user, issue JWT")
}

/// Returns a WebAuthn authentication challenge as a Datastar SSE stream.
async fn login_begin() -> &'static str {
    todo!("generate WebAuthn authentication challenge, store in challenges table, return SSE")
}

/// Verifies the WebAuthn authentication assertion and issues tokens.
async fn login_complete() -> &'static str {
    todo!("verify assertion, update sign_count, issue JWT")
}
