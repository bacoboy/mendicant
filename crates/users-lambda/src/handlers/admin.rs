use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use anyhow::Context as _;

use db::users::UserRepository;
use domain::user::{Role, User, UserId, UserStatus};

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users))
        .route("/admin/users/:id", get(get_user).patch(patch_user).delete(delete_user))
}

fn require_admin(auth: &AuthUser) -> Result<(), AppError> {
    if auth.0.role != Role::Administrator {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

// ── Response type (shared) ────────────────────────────────────────────────────

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

// ── Pagination helper ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<u32>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct ListResponse {
    users: Vec<UserResponse>,
    next_cursor: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /admin/users — list all users (Administrator role required).
async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    require_admin(&auth)?;
    let limit = q.limit.unwrap_or(50).min(200);
    let (users, next_cursor) = UserRepository::new(state.db)
        .list(limit, q.cursor)
        .await
        .context("failed to list users")?;
    Ok(Json(ListResponse {
        users: users.iter().map(UserResponse::from).collect(),
        next_cursor,
    }))
}

/// GET /admin/users/:id — get a single user by ID (Administrator role required).
async fn get_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    require_admin(&auth)?;
    let user_id = parse_user_id(&id)?;
    let user = UserRepository::new(state.db)
        .get(&user_id)
        .await
        .map_err(|e| match e {
            db::error::DbError::NotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        })?;
    Ok(Json(UserResponse::from(&user)))
}

#[derive(Deserialize)]
struct PatchUserRequest {
    role: Option<Role>,
    status: Option<UserStatus>,
}

/// PATCH /admin/users/:id — update a user's role and/or status (Administrator role required).
async fn patch_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<PatchUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    require_admin(&auth)?;
    if req.role.is_none() && req.status.is_none() {
        return Err(AppError::BadRequest("no fields provided".into()));
    }

    let user_id = parse_user_id(&id)?;
    let repo = UserRepository::new(state.db);

    if let Some(role) = req.role {
        repo.update_role(&user_id, &role)
            .await
            .context("failed to update role")?;
    }
    if let Some(status) = req.status {
        repo.update_status(&user_id, &status)
            .await
            .context("failed to update status")?;
    }

    let user = repo.get(&user_id).await.map_err(|e| match e {
        db::error::DbError::NotFound => AppError::NotFound,
        other => AppError::Internal(other.into()),
    })?;

    Ok(Json(UserResponse::from(&user)))
}

/// DELETE /admin/users/:id — suspend a user account (Administrator role required).
/// Does not hard-delete; sets status to Suspended.
async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_admin(&auth)?;
    let user_id = parse_user_id(&id)?;
    UserRepository::new(state.db)
        .update_status(&user_id, &UserStatus::Suspended)
        .await
        .map_err(|e| match e {
            db::error::DbError::NotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user_id(s: &str) -> Result<UserId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(UserId)
        .map_err(|_| AppError::BadRequest("invalid user ID".into()))
}
