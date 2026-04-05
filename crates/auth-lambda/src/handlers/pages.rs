use askama::Template;
use axum::Router;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use std::collections::HashMap;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(login_page))
        .route("/register", get(register_page))
        .route("/activate", get(activate_page))
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterPage;

#[derive(Template)]
#[template(path = "activate.html")]
struct ActivatePage {
    prefill_code: String,
}

async fn login_page() -> impl IntoResponse {
    render(LoginPage)
}

async fn register_page() -> impl IntoResponse {
    render(RegisterPage)
}

async fn activate_page(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let prefill_code = params.get("code").cloned().unwrap_or_default();
    render(ActivatePage { prefill_code })
}

fn render(t: impl Template) -> impl IntoResponse {
    match t.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "template render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response()
        }
    }
}
