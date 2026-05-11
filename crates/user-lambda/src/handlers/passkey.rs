use anyhow::Context as _;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use webauthn_rs::prelude::*;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use domain::challenge::Challenge;
use domain::credential::{Credential, CredentialId};
use domain::user::UserId;

use crate::error::AppError;
use crate::middleware::AuthUser;
use crate::sse::SseResponse;
use crate::state::AppState;

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/passkey/add/begin", post(add_passkey_begin))
        .route("/me/passkey/add/complete", post(add_passkey_complete))
}

/// Challenge state bundled into the registration record. Shape mirrors
/// auth-lambda's RegChallengeState so a future deduplication is mechanical.
#[derive(Serialize, Deserialize)]
struct RegChallengeState {
    email: String,
    display_name: String,
    state: PasskeyRegistration,
}

/// POST /me/passkey/add/begin — start a WebAuthn registration for an existing user.
async fn add_passkey_begin(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AuthUser(claims): AuthUser,
) -> Result<SseResponse, AppError> {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let webauthn = state.webauthn_for_origin(origin)
        .ok_or_else(|| AppError::BadRequest(format!("origin not allowed: {origin}")))?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("invalid user ID in token".into()))?;

    // Exclude already-registered credentials so the authenticator doesn't
    // silently register a duplicate.
    let exclude = CredentialRepository::new(state.db.clone())
        .list_for_user(&UserId(user_id))
        .await
        .ok()
        .and_then(|creds| {
            let ids: Vec<CredentialID> = creds
                .iter()
                .filter_map(|c| {
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(&c.id.0)
                        .ok()
                        .map(CredentialID::from)
                })
                .collect();
            if ids.is_empty() { None } else { Some(ids) }
        });

    let (ccr, reg_state) = webauthn
        .start_passkey_registration(user_id, &claims.email, &claims.email, exclude)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = RegChallengeState {
        email: claims.email.clone(),
        display_name: claims.email.clone(),
        state: reg_state,
    };
    let state_json = serde_json::to_string(&bundled)
        .context("failed to serialize registration state")?;

    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + CHALLENGE_TTL_SECS;
    let challenge = Challenge::new_registration(state_json, expires_at);
    let challenge_id = challenge.id.clone();

    ChallengeRepository::new(state.db.clone())
        .put(&challenge)
        .await
        .context("failed to store challenge")?;

    // Strip extensions for Safari compatibility (it rejects non-standard ones).
    let mut register_opts = serde_json::to_value(&ccr)
        .context("failed to serialize CreationChallengeResponse")?;
    if let Some(obj) = register_opts.as_object_mut() {
        if let Some(pk) = obj.get_mut("publicKey") {
            if let Some(pk_obj) = pk.as_object_mut() {
                pk_obj.remove("extensions");
            }
        }
    }

    let signals = serde_json::json!({
        "challengeId": challenge_id,
        "registerOptions": register_opts,
    });

    Ok(SseResponse::new().patch_signals(&signals.to_string()))
}

#[derive(Deserialize)]
struct AddPasskeyCompleteRequest {
    challenge_id: String,
    response: RegisterPublicKeyCredential,
}

/// POST /me/passkey/add/complete — verify the WebAuthn response and store the new passkey.
async fn add_passkey_complete(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AuthUser(claims): AuthUser,
    Json(req): Json<AddPasskeyCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let webauthn = state.webauthn_for_origin(origin)
        .ok_or_else(|| AppError::BadRequest(format!("origin not allowed: {origin}")))?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| AppError::BadRequest("invalid user ID in token".into()))?;

    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired challenge".into()))?;

    let bundled: RegChallengeState = serde_json::from_str(&challenge.state_json)
        .context("failed to deserialize registration state")?;

    let passkey = webauthn
        .finish_passkey_registration(&req.response, &bundled.state)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let cred_id = CredentialId(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(passkey.cred_id()),
    );

    let passkey_bytes = serde_json::to_vec(&passkey)
        .context("failed to serialize passkey")?;
    let now = OffsetDateTime::now_utc();

    let credential = Credential {
        id: cred_id,
        user_id,
        public_key: passkey_bytes,
        sign_count: 0,
        aaguid: uuid::Uuid::nil(),
        nickname: None,
        created_at: now,
        last_used_at: now,
    };

    CredentialRepository::new(state.db.clone())
        .put(&credential)
        .await
        .context("failed to store credential")?;

    Ok(SseResponse::new()
        .patch_signals(r#"{"addPasskeySuccess": true}"#)
        .redirect("/me"))
}
