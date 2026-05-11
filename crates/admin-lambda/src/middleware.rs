use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use domain::token::AccessTokenClaims;
use domain::user::Role;

use crate::error::AppError;
use crate::jwt::verify;
use crate::state::AppState;

/// Axum extractor that provides the verified JWT claims to handlers. By the
/// time a handler runs, the router-level `require_admin` middleware has
/// already verified the token and checked the role — this extractor just
/// re-parses to make the claims available to the handler.
pub struct AuthUser(pub AccessTokenClaims);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, AppError> {
        let token = extract_token(parts)?;
        let claims = verify(&token, &state.decoding_key)?;
        Ok(AuthUser(claims))
    }
}

/// Router-level guard. Every request to admin-lambda passes through this:
/// JWT must verify AND claims.role must be Administrator. Returns 403 (or
/// the redirect-to-/login for missing/invalid tokens) before any handler
/// runs, so a missing handler-level role check cannot accidentally expose
/// a route.
pub async fn require_admin(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let (mut parts, body) = req.into_parts();
    let token = extract_token(&parts)?;
    let claims = verify(&token, &state.decoding_key)?;
    if claims.role != Role::Administrator {
        return Err(AppError::Forbidden);
    }
    parts.extensions.insert(claims);
    let req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}

fn extract_token(parts: &Parts) -> Result<String, AppError> {
    if let Some(auth) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Ok(token.to_string());
            }
        }
    }
    if let Some(cookie_hdr) = parts.headers.get(axum::http::header::COOKIE) {
        if let Ok(s) = cookie_hdr.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(token) = part.strip_prefix("auth=") {
                    return Ok(token.to_string());
                }
            }
        }
    }
    Err(AppError::Unauthorized)
}
