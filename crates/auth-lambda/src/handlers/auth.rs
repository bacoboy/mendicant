use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use anyhow::Context as _;
use webauthn_rs::prelude::*;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use db::error::DbError;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::Challenge;
use domain::credential::{Credential, CredentialId};
use domain::user::{User, UserId};

use crate::error::AppError;
use crate::jwt::issue_tokens;
use crate::sse::SseResponse;
use crate::state::AppState;

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register/begin", post(register_begin))
        .route("/auth/register/complete", post(register_complete))
        .route("/auth/login/begin", post(login_begin))
        .route("/auth/login/complete", post(login_complete))
}

// ── Registration ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterBeginRequest {
    email: String,
    display_name: String,
}

/// State bundled into the challenge record. Binding email to the challenge
/// prevents a client from swapping the identity between begin and complete.
#[derive(Serialize, Deserialize)]
struct RegChallengeState {
    email: String,
    display_name: String,
    state: PasskeyRegistration,
}

/// Returns a WebAuthn registration challenge as a Datastar SSE stream
/// that patches the page signals with the challenge options.
async fn register_begin(
    State(state): State<AppState>,
    Json(req): Json<RegisterBeginRequest>,
) -> Result<SseResponse, AppError> {
    let user_uuid = uuid::Uuid::new_v4();
    let exclude = existing_cred_ids(&state, &req.email).await;

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_uuid, &req.email, &req.display_name, exclude)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = RegChallengeState {
        email: req.email,
        display_name: req.display_name,
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
        .map_err(|e| {
            eprintln!("Challenge storage error: {:?}", e);
            anyhow::anyhow!("failed to store challenge: {:?}", e)
        })?;

    let signals = serde_json::json!({
        "challengeId": challenge_id,
        "registerOptions": ccr,
    });

    Ok(SseResponse::new().patch_signals(&signals.to_string()))
}

/// Verifies the WebAuthn registration response, stores the credential,
/// creates the user if needed, and issues tokens.
async fn register_complete(
    State(state): State<AppState>,
    Json(req): Json<RegisterCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired challenge".into()))?;

    let bundled: RegChallengeState = serde_json::from_str(&challenge.state_json)
        .context("failed to deserialize registration state")?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.response, &bundled.state)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let users_repo = UserRepository::new(state.db.clone());
    let user = match users_repo.get_by_email(&bundled.email).await {
        Ok(u) => u,
        Err(DbError::NotFound) => {
            let u = User::new(bundled.email.clone(), bundled.display_name.clone());
            users_repo.put(&u).await.context("failed to create user")?;
            u
        }
        Err(e) => return Err(AppError::Internal(e.into())),
    };

    let cred_id = CredentialId(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(passkey.cred_id()),
    );
    let passkey_bytes = serde_json::to_vec(&passkey)
        .context("failed to serialize passkey")?;
    let now = OffsetDateTime::now_utc();

    let credential = Credential {
        id: cred_id,
        user_id: user.id.clone(),
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

    let tokens = issue_tokens(
        &user.id,
        &user.role,
        &user.email,
        &state.signer,
        &RefreshTokenRepository::new(state.db.clone()),
    )
    .await?;

    let secure = is_secure_context();
    Ok(SseResponse::new()
        .with_auth_cookie(&tokens.access_token, secure)
        .redirect("/"))
}

#[derive(Deserialize)]
struct RegisterCompleteRequest {
    challenge_id: String,
    response: RegisterPublicKeyCredential,
}

// ── Authentication ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginBeginRequest {
    email: String,
}

#[derive(Serialize, Deserialize)]
struct AuthChallengeState {
    state: PasskeyAuthentication,
}

/// Returns a WebAuthn authentication challenge as a Datastar SSE stream.
async fn login_begin(
    State(state): State<AppState>,
    Json(req): Json<LoginBeginRequest>,
) -> Result<SseResponse, AppError> {
    let users_repo = UserRepository::new(state.db.clone());
    let user = users_repo
        .get_by_email(&req.email)
        .await
        .map_err(|_| AppError::BadRequest("no account found for that email".into()))?;

    let credentials = CredentialRepository::new(state.db.clone())
        .list_for_user(&user.id)
        .await
        .context("failed to load credentials")?;

    if credentials.is_empty() {
        return Err(AppError::BadRequest(
            "no passkeys registered for that account".into(),
        ));
    }

    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| serde_json::from_slice(&c.public_key).ok())
        .collect();

    if passkeys.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "failed to deserialize stored passkeys"
        )));
    }

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = AuthChallengeState { state: auth_state };
    let state_json = serde_json::to_string(&bundled)
        .context("failed to serialize auth state")?;

    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + CHALLENGE_TTL_SECS;
    let challenge =
        Challenge::new_authentication(user.id.to_string(), state_json, expires_at);
    let challenge_id = challenge.id.clone();

    ChallengeRepository::new(state.db.clone())
        .put(&challenge)
        .await
        .context("failed to store challenge")?;

    let signals = serde_json::json!({
        "challengeId": challenge_id,
        "loginOptions": rcr,
    });

    Ok(SseResponse::new().patch_signals(&signals.to_string()))
}

#[derive(Deserialize)]
struct LoginCompleteRequest {
    challenge_id: String,
    response: PublicKeyCredential,
}

/// Verifies the WebAuthn authentication assertion and issues tokens.
async fn login_complete(
    State(state): State<AppState>,
    Json(req): Json<LoginCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired challenge".into()))?;

    let user_id_str = challenge
        .user_id
        .ok_or_else(|| AppError::BadRequest("missing user_id in challenge".into()))?;

    let user_uuid = uuid::Uuid::parse_str(&user_id_str)
        .map_err(|_| AppError::BadRequest("malformed user_id in challenge".into()))?;
    let user_id = UserId(user_uuid);

    let bundled: AuthChallengeState = serde_json::from_str(&challenge.state_json)
        .context("failed to deserialize auth state")?;

    let auth_result = state
        .webauthn
        .finish_passkey_authentication(&req.response, &bundled.state)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let used_cred_id_b64 =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(auth_result.cred_id());

    let creds_repo = CredentialRepository::new(state.db.clone());
    let credentials = creds_repo
        .list_for_user(&user_id)
        .await
        .context("failed to load credentials")?;

    let cred = credentials
        .iter()
        .find(|c| c.id.0 == used_cred_id_b64)
        .ok_or_else(|| AppError::BadRequest("credential not found".into()))?;

    creds_repo
        .update_sign_count(&user_id, &cred.id, auth_result.counter())
        .await
        .context("failed to update sign count")?;

    let user = UserRepository::new(state.db.clone())
        .get(&user_id)
        .await
        .context("failed to load user")?;

    let tokens = issue_tokens(
        &user.id,
        &user.role,
        &user.email,
        &state.signer,
        &RefreshTokenRepository::new(state.db.clone()),
    )
    .await?;

    let secure = is_secure_context();
    Ok(SseResponse::new()
        .with_auth_cookie(&tokens.access_token, secure)
        .redirect("/"))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the credential IDs already registered for an email, so they can be
/// excluded from a new registration (prevents duplicate passkeys).
async fn existing_cred_ids(state: &AppState, email: &str) -> Option<Vec<CredentialID>> {
    let user = UserRepository::new(state.db.clone())
        .get_by_email(email)
        .await
        .ok()?;
    let creds = CredentialRepository::new(state.db.clone())
        .list_for_user(&user.id)
        .await
        .ok()?;
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
}

fn is_secure_context() -> bool {
    std::env::var("ENVIRONMENT")
        .map(|e| e != "dev")
        .unwrap_or(true)
}
