use askama::Template;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, patch};
use axum::Json;
use serde::Deserialize;
use std::collections::HashMap;
use anyhow;
use time::OffsetDateTime;

use db::credentials::CredentialRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::user::UserId;

use crate::error::AppError;
use crate::handlers::NavUser;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(landing_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/register-confirm", get(register_confirm_page))
        .route("/recover", get(recover_page))
        .route("/activate", get(activate_page))
        .route("/me", get(profile_page))
        .route("/me", patch(update_profile))
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

struct CredentialRow {
    id: String,
    nickname: String,
    last_used_at: String,
    created_at: String,
    can_delete: bool,
}

struct SessionRow {
    client_hint: String,
    expires_in: String,
    is_current: bool,
}

#[derive(Template)]
#[template(path = "profile.html")]
#[allow(dead_code)]
struct ProfilePage {
    nav: NavUser,
    id: String,
    email: String,
    display_name: String,
    role: String,
    status: String,
    credentials: Vec<CredentialRow>,
    sessions: Vec<SessionRow>,
    has_other_sessions: bool,
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

async fn recover_page(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();
    if token.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Missing recovery token.").into_response();
    }
    render(RecoverPage { token }).into_response()
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
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub in token")))?;

    let current_jti = extract_refresh_jti(&headers);

    let user_repo = UserRepository::new(state.db.clone());
    let cred_repo = CredentialRepository::new(state.db.clone());
    let session_repo = RefreshTokenRepository::new(state.db.clone());

    let (user, raw_creds, raw_sessions) = tokio::try_join!(
        user_repo.get(&user_id),
        cred_repo.list_for_user(&user_id),
        session_repo.list_for_user(&user_id),
    ).map_err(|e: db::error::DbError| match e {
        db::error::DbError::NotFound => AppError::NotFound,
        other => AppError::Internal(other.into()),
    })?;

    let can_delete = raw_creds.len() > 1;
    let credentials: Vec<CredentialRow> = raw_creds
        .into_iter()
        .map(|c| CredentialRow {
            id: c.id.0,
            nickname: c.nickname.unwrap_or_else(|| "Unnamed passkey".to_string()),
            last_used_at: fmt_dt(c.last_used_at),
            created_at: fmt_dt(c.created_at),
            can_delete,
        })
        .collect();

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut sessions: Vec<SessionRow> = raw_sessions
        .into_iter()
        .map(|t| {
            let is_current = current_jti.as_deref() == Some(&t.jti);
            SessionRow {
                client_hint: t.client_hint.unwrap_or_else(|| "Unknown client".into()),
                expires_in: fuzzy_duration(t.expires_at - now),
                is_current,
            }
        })
        .collect();
    // Current session always at the top.
    sessions.sort_by_key(|s| !s.is_current);
    let has_other_sessions = sessions.iter().any(|s| !s.is_current);

    let role = serde_json::to_value(&user.role)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_default();
    let status = serde_json::to_value(&user.status)
        .and_then(|v| serde_json::from_value::<String>(v))
        .unwrap_or_default();

    let page = ProfilePage {
        nav: NavUser {
            email: claims.email.clone(),
            is_admin: claims.role == domain::user::Role::Administrator,
        },
        id: user.id.to_string(),
        email: user.email,
        display_name: user.display_name,
        role,
        status,
        credentials,
        sessions,
        has_other_sessions,
        debug_jwt_sub: claims.sub.clone(),
        debug_jwt_email: claims.email.clone(),
        debug_jwt_role: serde_json::to_string(&claims.role).unwrap_or_default(),
    };

    Ok(render(page))
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    display_name: String,
}

async fn update_profile(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = body.display_name.trim().to_string();
    if name.is_empty() || name.len() > 128 {
        return Err(AppError::BadRequest("display name must be 1–128 characters".into()));
    }
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub")))?;
    UserRepository::new(state.db)
        .update_display_name(&user_id, &name)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn extract_refresh_jti(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_hdr = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_hdr.split(';') {
        let part = part.trim();
        if let Some(jti) = part.strip_prefix("refresh_token=") {
            return Some(jti.to_string());
        }
    }
    None
}

fn fuzzy_duration(secs: i64) -> String {
    if secs <= 0 { return "expired".into(); }
    let minutes = secs / 60;
    let hours   = minutes / 60;
    let days    = hours / 24;
    if days >= 2      { return format!("expires in {} days", days); }
    if days == 1      { return "expires in 1 day".into(); }
    if hours >= 2     { return format!("expires in {} hours", hours); }
    if hours == 1     { return "expires in 1 hour".into(); }
    if minutes >= 2   { return format!("expires in {} minutes", minutes); }
    "expires in a moment".into()
}

fn fmt_dt(dt: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        dt.year(),
        dt.month() as u8,
        dt.day(),
        dt.hour(),
        dt.minute(),
    )
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
