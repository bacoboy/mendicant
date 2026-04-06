use askama::Template;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use std::collections::HashMap;
use anyhow;

use db::users::UserRepository;
use domain::user::UserId;

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(landing_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/register-confirm", get(register_confirm_page))
        .route("/activate", get(activate_page))
        .route("/me", get(profile_page))
        .route("/logout", post(logout))
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
#[template(path = "activate.html")]
struct ActivatePage {
    prefill_code: String,
}

#[derive(Template)]
#[template(path = "profile.html")]
#[allow(dead_code)]
struct ProfilePage {
    id: String,
    email: String,
    display_name: String,
    role: String,
    status: String,
    debug_jwt_sub: String,
    debug_jwt_email: String,
    debug_jwt_role: String,
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

async fn activate_page(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let prefill_code = params.get("code").cloned().unwrap_or_default();
    render(ActivatePage { prefill_code })
}

async fn profile_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub in token")))?;

    let user = UserRepository::new(state.db)
        .get(&user_id)
        .await
        .map_err(|e| match e {
            db::error::DbError::NotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        })?;

    let role = serde_json::to_value(&user.role)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_default();
    let status = serde_json::to_value(&user.status)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_default();

    let page = ProfilePage {
        id: user.id.to_string(),
        email: user.email,
        display_name: user.display_name,
        role,
        status,
        debug_jwt_sub: claims.sub.clone(),
        debug_jwt_email: claims.email.clone(),
        debug_jwt_role: serde_json::to_string(&claims.role).unwrap_or_default(),
    };

    Ok(render(page))
}

/// POST /logout — clear the auth cookie and redirect to login
async fn logout(
    AuthUser(_claims): AuthUser,
) -> impl IntoResponse {
    let mut response = Redirect::to("/").into_response();
    let headers = response.headers_mut();
    // Clear the auth cookie by setting it to empty with an expired date
    headers.insert(
        header::SET_COOKIE,
        "auth=; Max-Age=0; Path=/; HttpOnly; SameSite=Strict".parse().unwrap(),
    );
    response
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
