use axum::Router;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/static/datastar.js", get(datastar_js))
        .route("/static/passkey-plugin.js", get(passkey_plugin_js))
        .route("/static/theme.css", get(theme_css))
        .route("/robots.txt", get(robots_txt))
        .route("/favicon.svg", get(favicon_svg))
        .route("/favicon.ico", get(favicon_ico))
        .route("/favicon-32x32.png", get(favicon_32))
        .route("/apple-touch-icon.png", get(apple_touch_icon))
        .route("/favicon-192.png", get(favicon_192))
}

async fn datastar_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../static/datastar.js"),
    )
}

async fn passkey_plugin_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../static/passkey-plugin.js"),
    )
}

async fn theme_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/theme.css"),
    )
}

async fn robots_txt() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        include_str!("../../static/robots.txt"),
    )
}

async fn favicon_svg() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../../static/favicon.svg"),
    )
}

async fn favicon_ico() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../../static/favicon.ico").as_ref(),
    )
}

async fn favicon_32() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../static/favicon-32x32.png").as_ref(),
    )
}

async fn apple_touch_icon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../static/apple-touch-icon.png").as_ref(),
    )
}

async fn favicon_192() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../static/favicon-192.png").as_ref(),
    )
}
