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
        .route("/", get(landing_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/register-confirm", get(register_confirm_page))
        .route("/recover", get(recover_page))
        .route("/activate", get(activate_page))
}

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingPage;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterPage;

#[derive(Template)]
#[template(path = "register-confirm.html")]
struct RegisterConfirmPage;

#[derive(Template)]
#[template(path = "recover.html")]
#[allow(dead_code)]
struct RecoverPage {
    token: String,
}

#[derive(Template)]
#[template(path = "activate.html")]
struct ActivatePage {
    prefill_code: String,
}

async fn landing_page() -> impl IntoResponse {
    render(LandingPage)
}

async fn login_page() -> impl IntoResponse {
    render(LoginPage)
}

async fn register_page() -> impl IntoResponse {
    render(RegisterPage)
}

async fn register_confirm_page() -> impl IntoResponse {
    render(RegisterConfirmPage)
}

async fn recover_page(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();
    if token.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing recovery token.").into_response();
    }
    render(RecoverPage { token }).into_response()
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
