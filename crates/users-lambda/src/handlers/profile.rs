use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use anyhow::Context as _;

use db::users::UserRepository;
use domain::user::{User, UserId};

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_profile).patch(patch_profile))
}

#[derive(Serialize)]
struct ProfileResponse {
    id: String,
    email: String,
    display_name: String,
    role: String,
    status: String,
    created_at: String,
}

impl From<&User> for ProfileResponse {
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
        }
    }
}

fn parse_user_id(sub: &str) -> Result<UserId, AppError> {
    uuid::Uuid::parse_str(sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub in token")))
}

/// GET /me — return the authenticated user's profile.
async fn get_profile(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<ProfileResponse>, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let user = UserRepository::new(state.db)
        .get(&user_id)
        .await
        .map_err(|e| match e {
            db::error::DbError::NotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        })?;
    Ok(Json(ProfileResponse::from(&user)))
}

#[derive(Deserialize)]
struct PatchProfileRequest {
    display_name: Option<String>,
}

/// PATCH /me — update the authenticated user's display_name.
async fn patch_profile(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<PatchProfileRequest>,
) -> Result<Json<ProfileResponse>, AppError> {
    if req.display_name.is_none() {
        return Err(AppError::BadRequest("no fields provided".into()));
    }

    let user_id = parse_user_id(&claims.sub)?;
    let repo = UserRepository::new(state.db);

    let mut user = repo.get(&user_id).await.map_err(|e| match e {
        db::error::DbError::NotFound => AppError::NotFound,
        other => AppError::Internal(other.into()),
    })?;

    if let Some(name) = req.display_name {
        user.display_name = name;
    }
    user.updated_at = OffsetDateTime::now_utc();

    repo.put(&user).await.context("failed to save profile")?;

    Ok(Json(ProfileResponse::from(&user)))
}
