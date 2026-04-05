use axum::Router;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/static/passkey-plugin.js", get(passkey_plugin_js))
}

/// Serve the passkey Datastar plugin. The file is compiled into the binary
/// via include_str!, so no filesystem access is needed at runtime.
async fn passkey_plugin_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../static/passkey-plugin.js"),
    )
}
