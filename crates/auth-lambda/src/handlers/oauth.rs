use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use anyhow::Context as _;

use db::oauth_devices::OAuthDeviceRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::oauth::{DeviceGrant, DeviceGrantStatus};
use domain::user::UserId;

use crate::error::AppError;
use crate::jwt::issue_tokens;
use crate::signing::verify_jwt;
use crate::sse::SseResponse;
use crate::state::AppState;

const DEVICE_GRANT_TTL_SECS: i64 = 15 * 60; // 15 minutes
const POLLING_INTERVAL_SECS: u64 = 5;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/device", post(device_authorize))
        .route("/oauth/token", post(device_token))
        .route("/activate", post(activate_complete))
}

// ── Device authorization (RFC 8628 §3.1) ─────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
struct DeviceAuthorizeRequest {
    client_id: String,
    scope: Option<String>,
}

#[derive(Serialize)]
struct DeviceAuthorizeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: String,
    expires_in: i64,
    interval: u64,
}

/// RFC 8628 §3.1 — device authorization request (called by CLI).
/// Returns device_code, user_code, verification_uri, expires_in, interval.
async fn device_authorize(
    State(state): State<AppState>,
    Json(_req): Json<DeviceAuthorizeRequest>,
) -> Result<Json<DeviceAuthorizeResponse>, AppError> {
    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + DEVICE_GRANT_TTL_SECS;
    let grant = DeviceGrant::new(expires_at);

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:9000".into());
    let verification_uri = format!("{base_url}/activate");
    let verification_uri_complete =
        format!("{base_url}/activate?code={}", grant.user_code);

    OAuthDeviceRepository::new(state.db.clone())
        .put(&grant)
        .await
        .context("failed to store device grant")?;

    Ok(Json(DeviceAuthorizeResponse {
        device_code: grant.device_code,
        user_code: grant.user_code,
        verification_uri,
        verification_uri_complete,
        expires_in: DEVICE_GRANT_TTL_SECS,
        interval: POLLING_INTERVAL_SECS,
    }))
}

// ── Device token poll (RFC 8628 §3.4) ────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
struct DeviceTokenRequest {
    grant_type: String,
    device_code: String,
    client_id: String,
}

#[derive(Serialize)]
struct DeviceTokenSuccess {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
}

/// RFC 8628 §3.4 — device access token request (polled by CLI).
/// Returns access_token + refresh_token when approved, or an RFC error.
async fn device_token(
    State(state): State<AppState>,
    Json(req): Json<DeviceTokenRequest>,
) -> impl IntoResponse {
    match do_device_token(&state, req).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => {
            let status = match e.as_str() {
                "authorization_pending" | "slow_down" | "access_denied" | "expired_token"
                | "unsupported_grant_type" => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({ "error": e }))).into_response()
        }
    }
}

async fn do_device_token(
    state: &AppState,
    req: DeviceTokenRequest,
) -> Result<DeviceTokenSuccess, String> {
    if req.grant_type != "urn:ietf:params:oauth:grant-type:device_code" {
        return Err("unsupported_grant_type".into());
    }

    let repo = OAuthDeviceRepository::new(state.db.clone());
    let grant = repo
        .get_by_device_code(&req.device_code)
        .await
        .map_err(|_| "expired_token".to_string())?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    if grant.expires_at < now {
        return Err("expired_token".into());
    }

    match grant.status {
        DeviceGrantStatus::Pending => Err("authorization_pending".into()),
        DeviceGrantStatus::Denied => Err("access_denied".into()),
        DeviceGrantStatus::Approved => {
            let user_id_str = grant.user_id.ok_or_else(|| "server_error".to_string())?;
            let user_uuid = uuid::Uuid::parse_str(&user_id_str)
                .map_err(|_| "server_error".to_string())?;

            let user = UserRepository::new(state.db.clone())
                .get(&UserId(user_uuid))
                .await
                .map_err(|_| "server_error".to_string())?;

            let tokens = issue_tokens(
                &user.id,
                &user.role,
                &user.email,
                &state.signer,
                &RefreshTokenRepository::new(state.db.clone()),
            )
            .await
            .map_err(|_| "server_error".to_string())?;

            Ok(DeviceTokenSuccess {
                access_token: tokens.access_token,
                token_type: "Bearer",
                expires_in: tokens.expires_in,
                refresh_token: tokens.refresh_token_jti,
            })
        }
    }
}

// ── Activation (browser calls this after authenticating) ──────────────────────

#[derive(Deserialize)]
struct ActivateRequest {
    user_code: String,
}

/// Called by the browser after the user authenticates and enters their user_code.
/// Returns a Datastar SSE stream confirming approval.
async fn activate_complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ActivateRequest>,
) -> Result<SseResponse, AppError> {
    let token = extract_bearer_or_cookie(&headers)?;
    let claims = verify_jwt(&token, &state.decoding_key)?;

    let user_code = req.user_code.trim().to_uppercase();
    let repo = OAuthDeviceRepository::new(state.db.clone());
    let grant = repo
        .get_by_user_code(&user_code)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired activation code".into()))?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    if grant.expires_at < now {
        return Err(AppError::BadRequest("activation code has expired".into()));
    }
    if grant.status != DeviceGrantStatus::Pending {
        return Err(AppError::BadRequest(
            "activation code has already been used".into(),
        ));
    }

    repo.approve(&grant.device_code, &claims.sub)
        .await
        .context("failed to approve device grant")?;

    Ok(SseResponse::new().patch_elements(
        r#"<div id="activate-status"><p>Device activated! You can close this window and return to your terminal.</p></div>"#,
    ))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_bearer_or_cookie(headers: &HeaderMap) -> Result<String, AppError> {
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                return Ok(token.to_string());
            }
        }
    }
    if let Some(cookie_hdr) = headers.get(axum::http::header::COOKIE) {
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
