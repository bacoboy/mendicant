use askama::Template;
use anyhow::Context as _;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::Challenge;
use domain::user::{Role, User, UserId, UserStatus};

use crate::error::AppError;
use crate::handlers::NavUser;
use crate::handlers::util::{
    aaguid_display, fmt_dt_short, format_role, format_status,
    parse_role_filter, parse_status_filter, trunc,
};
use crate::middleware::AuthUser;
use crate::state::AppState;

const RECOVERY_TOKEN_TTL_SECS: i64 = 86_400; // 24 hours

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(admin_users_page))
        .route("/admin/users/{id}", get(admin_user_detail_page).patch(patch_user).delete(admin_delete_user))
        .route("/admin/users/{id}/status", post(admin_user_set_status))
        .route("/admin/users/{id}/reset-passkey", post(admin_reset_passkey))
}

// ── List page ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UsersQuery {
    q: Option<String>,
    role: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
}

struct UserRow {
    id: String,
    email: String,
    display_name: String,
    role: String,
    status: String,
    credential_count: usize,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin-users.html")]
#[allow(dead_code)]
struct AdminUsersPage {
    nav: NavUser,
    users: Vec<UserRow>,
    query: String,
    role_filter: String,
    status_filter: String,
    is_filtered: bool,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    active_section: &'static str,
    active_table: &'static str,
}

async fn admin_users_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<UsersQuery>,
) -> Result<Html<String>, AppError> {
    let email_query = q.q.as_deref().filter(|s| !s.is_empty());
    let role_filter = q.role.as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_role_filter)
        .transpose()?;
    let status_filter = q.status.as_deref()
        .filter(|s| !s.is_empty())
        .map(parse_status_filter)
        .transpose()?;

    let is_filtered = email_query.is_some() || role_filter.is_some() || status_filter.is_some();

    let (raw_users, next_cursor) = UserRepository::new(state.db.clone())
        .list(50, q.cursor.clone(), email_query, role_filter.as_ref(), status_filter.as_ref())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let cred_repo = CredentialRepository::new(state.db.clone());
    let mut users = Vec::with_capacity(raw_users.len());
    for u in raw_users {
        let cred_count = cred_repo.list_for_user(&u.id).await.map(|v| v.len()).unwrap_or(0);
        users.push(UserRow {
            id: u.id.to_string(),
            email: u.email,
            display_name: u.display_name,
            role: format_role(&u.role),
            status: format_status(&u.status),
            credential_count: cred_count,
            created_at: trunc(&u.created_at.to_string(), 16),
        });
    }

    Ok(Html(AdminUsersPage {
        nav: NavUser { email: claims.email.clone(), is_admin: true },
        users,
        query: q.q.unwrap_or_default(),
        role_filter: q.role.unwrap_or_default(),
        status_filter: q.status.unwrap_or_default(),
        is_filtered,
        next_cursor,
        prev_cursor: None,
        active_section: "users",
        active_table: "",
    }.render().map_err(|e| anyhow::anyhow!(e))?))
}

// ── Detail page ───────────────────────────────────────────────────────────────

struct AdminCredRow {
    nickname: String,
    key_type: String,
    sign_count: String,
    last_used_at: String,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin-user-detail.html")]
#[allow(dead_code)]
struct AdminUserDetailPage {
    nav: NavUser,
    user_id: String,
    user_email: String,
    display_name: String,
    role: String,
    status: String,
    created_at: String,
    active_session_count: usize,
    credentials: Vec<AdminCredRow>,
    active_section: &'static str,
    active_table: &'static str,
}

async fn admin_user_detail_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    let user_id = parse_user_id(&id)?;

    let user_repo = UserRepository::new(state.db.clone());
    let cred_repo = CredentialRepository::new(state.db.clone());
    let token_repo = RefreshTokenRepository::new(state.db.clone());

    let (user, raw_creds, sessions) = tokio::try_join!(
        user_repo.get(&user_id),
        cred_repo.list_for_user(&user_id),
        token_repo.list_for_user(&user_id),
    ).map_err(|e: db::error::DbError| match e {
        db::error::DbError::NotFound => AppError::NotFound,
        other => AppError::Internal(other.into()),
    })?;

    let credentials = raw_creds.into_iter().map(|c| AdminCredRow {
        nickname: c.nickname.unwrap_or_else(|| "Unnamed passkey".into()),
        key_type: aaguid_display(&c.aaguid.to_string()),
        sign_count: c.sign_count.to_string(),
        last_used_at: fmt_dt_short(c.last_used_at),
        created_at: fmt_dt_short(c.created_at),
    }).collect();

    Ok(Html(AdminUserDetailPage {
        nav: NavUser { email: claims.email.clone(), is_admin: true },
        user_id: user.id.to_string(),
        user_email: user.email,
        display_name: user.display_name,
        role: format_role(&user.role),
        status: format_status(&user.status),
        created_at: fmt_dt_short(user.created_at),
        active_session_count: sessions.len(),
        credentials,
        active_section: "users",
        active_table: "",
    }.render().map_err(|e| anyhow::anyhow!(e))?))
}

// ── User actions: PATCH, status, delete, reset-passkey ────────────────────────

#[derive(Deserialize)]
struct PatchUserRequest {
    role: Option<Role>,
    status: Option<UserStatus>,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    email: String,
    display_name: String,
    role: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<&User> for UserResponse {
    fn from(u: &User) -> Self {
        use time::format_description::well_known::Rfc3339;
        Self {
            id: u.id.to_string(),
            email: u.email.clone(),
            display_name: u.display_name.clone(),
            role: serde_json::to_value(&u.role)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_default(),
            status: serde_json::to_value(&u.status)
                .and_then(|v| serde_json::from_value::<String>(v))
                .unwrap_or_default(),
            created_at: u.created_at.format(&Rfc3339).unwrap_or_default(),
            updated_at: u.updated_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

async fn patch_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    if req.role.is_none() && req.status.is_none() {
        return Err(AppError::BadRequest("no fields provided".into()));
    }
    let user_id = parse_user_id(&id)?;
    let repo = UserRepository::new(state.db);

    if let Some(role) = req.role {
        repo.update_role(&user_id, &role).await.context("failed to update role")?;
    }
    if let Some(status) = req.status {
        repo.update_status(&user_id, &status).await.context("failed to update status")?;
    }

    let user = repo.get(&user_id).await.map_err(|e| match e {
        db::error::DbError::NotFound => AppError::NotFound,
        other => AppError::Internal(other.into()),
    })?;

    Ok(Json(UserResponse::from(&user)))
}

#[derive(Deserialize)]
struct SetStatusRequest {
    status: String,
}

async fn admin_user_set_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetStatusRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&id)?;
    let new_status = parse_status_filter(&req.status)?;

    UserRepository::new(state.db.clone())
        .update_status(&user_id, &new_status)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Revoke all sessions when suspending.
    if new_status == UserStatus::Suspended {
        RefreshTokenRepository::new(state.db.clone())
            .revoke_all_for_user(&user_id)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn admin_delete_user(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&id)?;

    RefreshTokenRepository::new(state.db.clone())
        .revoke_all_for_user(&user_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    CredentialRepository::new(state.db.clone())
        .delete_all_for_user(&user_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    UserRepository::new(state.db.clone())
        .delete(&user_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    tracing::info!(admin = %claims.email, deleted_user = %user_id, "admin deleted user");

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct ResetPasskeyResponse {
    recovery_url: String,
}

async fn admin_reset_passkey(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ResetPasskeyResponse>, AppError> {
    let user_id = parse_user_id(&id)?;

    // Verify user exists before issuing a token.
    UserRepository::new(state.db.clone())
        .get(&user_id)
        .await
        .map_err(|e| match e {
            db::error::DbError::NotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        })?;

    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + RECOVERY_TOKEN_TTL_SECS;
    let token = Challenge::new_passkey_recovery(user_id.to_string(), expires_at);
    let token_id = token.id.clone();

    ChallengeRepository::new(state.db.clone())
        .put(&token)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let recovery_url = format!("{}/recover?token={}", state.base_url, token_id);

    tracing::info!(admin = %claims.email, target_user = %user_id, "admin issued passkey recovery token");

    Ok(Json(ResetPasskeyResponse { recovery_url }))
}

fn parse_user_id(s: &str) -> Result<UserId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(UserId)
        .map_err(|_| AppError::BadRequest("invalid user id".into()))
}
