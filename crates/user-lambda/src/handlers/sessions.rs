use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;

use db::refresh_tokens::RefreshTokenRepository;
use domain::user::UserId;

use crate::error::AppError;
use crate::handlers::{extract_refresh_jti, is_secure_context};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/logout", post(logout))
        .route("/me/sessions/revoke-others", post(revoke_other_sessions))
}

/// POST /me/logout — revoke the refresh token and clear all auth cookies.
async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(jti) = extract_refresh_jti(&headers) {
        let refresh_repo = RefreshTokenRepository::new(state.db.clone());
        match refresh_repo.revoke(&jti).await {
            Ok(()) => {}
            // Token already gone — still clear cookies.
            Err(db::error::DbError::NotFound | db::error::DbError::ConditionalCheckFailed) => {}
            Err(e) => return Err(AppError::Internal(anyhow::anyhow!(e))),
        }
    }

    let secure = is_secure_context();
    let secure_flag = if secure { "; Secure" } else { "" };
    let response = axum::response::Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/")
        .header(axum::http::header::SET_COOKIE, format!("auth=; HttpOnly{}; SameSite=Strict; Path=/; Max-Age=0", secure_flag))
        .header(axum::http::header::SET_COOKIE, format!("auth_exp=; SameSite=Strict; Path=/; Max-Age=0"))
        .header(axum::http::header::SET_COOKIE, format!("refresh_token=; HttpOnly{}; SameSite=Strict; Path=/; Max-Age=0", secure_flag))
        .body(axum::body::Body::empty())
        .unwrap();
    Ok(response)
}

/// POST /me/sessions/revoke-others — revoke all refresh tokens for the
/// current user except the one belonging to this session.
async fn revoke_other_sessions(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let current_jti = extract_refresh_jti(&headers);
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("malformed sub")))?;

    let refresh_repo = RefreshTokenRepository::new(state.db.clone());
    let tokens = refresh_repo.list_for_user(&user_id).await
        .map_err(|e| AppError::Internal(e.into()))?;

    for token in tokens {
        if current_jti.as_deref() == Some(&token.jti) {
            continue;
        }
        if let Err(e) = refresh_repo.revoke(&token.jti).await {
            tracing::warn!(jti = %token.jti, error = %e, "failed to revoke session");
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
