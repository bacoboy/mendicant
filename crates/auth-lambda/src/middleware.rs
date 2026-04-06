use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use domain::token::AccessTokenClaims;

use crate::error::AppError;
use crate::signing::verify_jwt;
use crate::state::AppState;

/// Axum extractor that verifies the JWT from the request and provides
/// the decoded claims to handlers. Accepts both:
///   - `Authorization: Bearer <token>` (CLI/API clients)
///   - `Cookie: auth=<token>` (browser clients)
pub struct AuthUser(pub AccessTokenClaims);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, AppError> {
        let token = extract_token(parts)?;
        let claims = verify_jwt(&token, &state.decoding_key)
            .map_err(|e| {
                tracing::error!("JWT verification failed: {}", e);
                AppError::Unauthorized
            })?;
        Ok(AuthUser(claims))
    }
}

fn extract_token(parts: &Parts) -> Result<String, AppError> {
    if let Some(auth) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                tracing::debug!("extracted token from Authorization header");
                return Ok(token.to_string());
            }
        }
    }
    if let Some(cookie_hdr) = parts.headers.get(axum::http::header::COOKIE) {
        if let Ok(s) = cookie_hdr.to_str() {
            tracing::debug!("cookie header: {}", s);
            for part in s.split(';') {
                let part = part.trim();
                if let Some(token) = part.strip_prefix("auth=") {
                    tracing::debug!("extracted token from auth cookie");
                    return Ok(token.to_string());
                }
            }
        }
    }
    tracing::error!("no token found in request headers");
    Err(AppError::Unauthorized)
}
