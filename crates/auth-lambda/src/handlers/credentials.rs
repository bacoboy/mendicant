use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::patch;
use serde::Deserialize;
use uuid::Uuid;

use db::credentials::CredentialRepository;
use domain::credential::CredentialId;
use domain::user::UserId;

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/auth/credentials/{id}",
            patch(rename_credential).delete(delete_credential),
        )
}

// ── PATCH /auth/credentials/{id} ─────────────────────────────────────────────

#[derive(Deserialize)]
struct RenameRequest {
    nickname: String,
}

async fn rename_credential(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<RenameRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let cred_id = CredentialId(id);
    let repo = CredentialRepository::new(state.db.clone());

    // Verify the credential belongs to this user.
    let cred = repo.get(&cred_id).await.map_err(|_| AppError::NotFound)?;
    if cred.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    let nickname = req.nickname.trim().to_string();
    if nickname.is_empty() {
        return Err(AppError::BadRequest("nickname cannot be empty".into()));
    }

    repo.update_nickname(&user_id, &cred_id, &nickname)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /auth/credentials/{id} ────────────────────────────────────────────

async fn delete_credential(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims.sub)?;
    let cred_id = CredentialId(id);
    let repo = CredentialRepository::new(state.db.clone());

    // Verify the credential belongs to this user.
    let cred = repo.get(&cred_id).await.map_err(|_| AppError::NotFound)?;
    if cred.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    // Guard: never delete the user's last passkey — they would be locked out.
    let all = repo
        .list_for_user(&user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    if all.len() <= 1 {
        return Err(AppError::BadRequest(
            "Cannot delete your only passkey — you would be locked out.".into(),
        ));
    }

    repo.delete(&user_id, &cred_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_user_id(sub: &str) -> Result<UserId, AppError> {
    Uuid::parse_str(sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub in token")))
}
