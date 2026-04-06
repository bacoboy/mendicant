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
use db::email_tokens::EmailTokenRepository;
use db::error::DbError;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::Challenge;
use domain::credential::{Credential, CredentialId};
use domain::email_token::EmailToken;
use domain::user::User;

use crate::error::AppError;
use crate::jwt::issue_tokens;
use crate::middleware::AuthUser;
use crate::sse::SseResponse;
use crate::state::AppState;

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes
const EMAIL_TOKEN_TTL_SECS: i64 = 900; // 15 minutes

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register/email", post(register_email))
        .route("/auth/register/begin", post(register_begin))
        .route("/auth/register/complete", post(register_complete))
        .route("/auth/login/begin", post(login_begin))
        .route("/auth/login/complete", post(login_complete))
        .route("/auth/passkey/add/begin", post(add_passkey_begin))
        .route("/auth/passkey/add/complete", post(add_passkey_complete))
}

// ── Email Validation ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterEmailRequest {
    email: String,
}

#[derive(Serialize)]
struct RegisterEmailResponse {
    token: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// POST /auth/register/email — validates email availability and creates a validation token.
/// In dev, returns the token in response. In production, sends via SES email.
async fn register_email(
    State(state): State<AppState>,
    Json(req): Json<RegisterEmailRequest>,
) -> Result<Json<RegisterEmailResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    // Check if email is already registered
    let users_repo = UserRepository::new(state.db.clone());
    if users_repo.get_by_email(&req.email).await.is_ok() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "An account with this email already exists".into(),
            }),
        ));
    }

    // Create email token (display name will be provided later on confirmation page)
    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + EMAIL_TOKEN_TTL_SECS;
    let token = EmailToken::new(req.email.clone(), expires_at);
    let token_id = token.id.clone();

    EmailTokenRepository::new(state.db.clone())
        .put(&token)
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create validation token. Please try again.".into(),
                }),
            )
        })?;

    // TODO: Send email via AWS SES with link: /register-confirm?token={token_id}
    // For now, return token in response for testing
    tracing::info!("email validation token created for {}: {}", req.email, token_id);

    Ok(Json(RegisterEmailResponse {
        token: token_id,
    }))
}

// ── Registration ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterBeginRequest {
    token: String,
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
/// Requires a valid email token from the email validation step and a display name.
async fn register_begin(
    State(state): State<AppState>,
    Json(req): Json<RegisterBeginRequest>,
) -> Result<SseResponse, AppError> {
    // Look up and consume the email token
    let email_token = EmailTokenRepository::new(state.db.clone())
        .take(&req.token)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired email token".into()))?;

    let email = email_token.email.clone();
    let display_name = req.display_name.clone();

    // Double-check email is not already registered (as of now)
    let users_repo = UserRepository::new(state.db.clone());
    if users_repo.get_by_email(&email).await.is_ok() {
        return Err(AppError::BadRequest(
            "An account with this email already exists".into()
        ));
    }

    let user_uuid = uuid::Uuid::new_v4();
    let exclude = existing_cred_ids(&state, &email).await;

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_uuid, &email, &display_name, exclude)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = RegChallengeState {
        email,
        display_name,
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

    // Convert ccr to JSON and remove extensions (Safari compatibility)
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

    // Check if email is already registered
    if let Ok(_) = users_repo.get_by_email(&bundled.email).await {
        return Err(AppError::BadRequest(
            format!("Email {} is already registered", bundled.email)
        ));
    }

    // Create new user
    let user = User::new(bundled.email.clone(), bundled.display_name.clone());
    users_repo.put(&user).await.context("failed to create user")?;

    let cred_id = CredentialId(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(passkey.cred_id()),
    );
    tracing::info!("register_complete: stored credential ID: {}", cred_id.0);

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
        .redirect("/me"))
}

#[derive(Deserialize)]
struct RegisterCompleteRequest {
    challenge_id: String,
    response: RegisterPublicKeyCredential,
}

// ── Authentication ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct AuthChallengeState {
    state: PasskeyAuthentication,
}

/// Returns a WebAuthn authentication challenge as a Datastar SSE stream.
/// Always uses discovery mode (no email required) — the authenticator shows all
/// passkeys registered for the domain.
async fn login_begin(
    State(state): State<AppState>,
) -> Result<SseResponse, AppError> {
    // Discovery mode: empty passkey list lets the authenticator show all passkeys
    // for this domain without requiring the user to enter an email first.
    let passkeys = vec![];

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = AuthChallengeState { state: auth_state };
    let state_json = serde_json::to_string(&bundled)
        .context("failed to serialize auth state")?;

    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + CHALLENGE_TTL_SECS;
    let challenge = Challenge::new_registration(state_json, expires_at);
    let challenge_id = challenge.id.clone();

    ChallengeRepository::new(state.db.clone())
        .put(&challenge)
        .await
        .context("failed to store challenge")?;

    // Convert rcr to JSON and remove extensions (Safari compatibility)
    let mut login_opts = serde_json::to_value(&rcr)
        .context("failed to serialize RequestChallengeResponse")?;
    if let Some(obj) = login_opts.as_object_mut() {
        if let Some(pk) = obj.get_mut("publicKey") {
            if let Some(pk_obj) = pk.as_object_mut() {
                pk_obj.remove("extensions");
            }
        }
    }

    let signals = serde_json::json!({
        "challengeId": challenge_id,
        "loginOptions": login_opts,
    });

    Ok(SseResponse::new().patch_signals(&signals.to_string()))
}

#[derive(Deserialize)]
struct LoginCompleteRequest {
    challenge_id: String,
    response: PublicKeyCredential,
}

/// Verifies the WebAuthn authentication assertion and issues tokens.
/// In discovery mode, finds the user from the authenticated credential.
async fn login_complete(
    State(state): State<AppState>,
    Json(req): Json<LoginCompleteRequest>,
) -> Result<SseResponse, AppError> {
    tracing::info!("login_complete called with challenge_id: {}", req.challenge_id);

    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|e| {
            tracing::error!("challenge lookup failed: {:?}", e);
            AppError::BadRequest("invalid or expired challenge".into())
        })?;

    // Extract credential ID from response
    let response_cred_id_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(req.response.raw_id.clone());

    tracing::info!("extracted credential ID from response: {}", response_cred_id_b64);

    // Look up the credential from the database
    let creds_repo = CredentialRepository::new(state.db.clone());
    let cred = match creds_repo
        .get(&CredentialId(response_cred_id_b64.clone()))
        .await
    {
        Ok(c) => {
            tracing::info!("credential found for user: {}", c.user_id);
            c
        }
        Err(DbError::NotFound) => {
            tracing::error!("credential not found in database. Tried ID: {}", response_cred_id_b64);
            return Err(AppError::BadRequest("credential not found".into()));
        }
        Err(e) => {
            tracing::error!("database error during credential lookup: {:?}", e);
            return Err(AppError::Internal(anyhow::anyhow!("credential lookup failed: {:?}", e)));
        }
    };

    // Verify the challenge nonce from client data matches what we stored
    let response_client_data_json = &req.response.response.client_data_json;
    let client_data_str = String::from_utf8_lossy(&response_client_data_json);

    #[derive(Deserialize)]
    struct ClientDataJSON {
        challenge: String,
    }
    let client_data: ClientDataJSON = serde_json::from_str(&client_data_str)
        .context("failed to parse client data JSON")?;

    // Extract challenge from stored auth state
    #[derive(Deserialize)]
    struct AuthStateSnapshot {
        state: serde_json::Value,
    }
    let auth_snapshot: AuthStateSnapshot = serde_json::from_str(&challenge.state_json)
        .context("failed to parse auth state")?;

    let stored_challenge = auth_snapshot
        .state
        .get("ast")
        .and_then(|ast| ast.get("challenge"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("challenge not found in auth state"))?;

    if client_data.challenge != stored_challenge {
        tracing::error!(
            "challenge mismatch: response has {}, expected {}",
            client_data.challenge,
            stored_challenge
        );
        return Err(AppError::BadRequest("challenge verification failed".into()));
    }

    tracing::info!("challenge verification passed, logging in user: {}", cred.user_id);

    // Extract counter from authenticator data (bytes 33-36, big-endian)
    let authenticator_data = &req.response.response.authenticator_data;
    let counter = if authenticator_data.len() >= 37 {
        u32::from_be_bytes([
            authenticator_data[33],
            authenticator_data[34],
            authenticator_data[35],
            authenticator_data[36],
        ])
    } else {
        tracing::warn!("authenticator data too short to extract counter, using 0");
        0
    };

    let user_id = cred.user_id.clone();
    let cred_id = cred.id.clone();

    creds_repo
        .update_sign_count(&user_id, &cred_id, counter)
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
        .redirect("/me"))
}

// ── Add Passkey (for authenticated users) ─────────────────────────────────────

/// POST /auth/passkey/add/begin — start a WebAuthn registration for an existing user
async fn add_passkey_begin(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<SseResponse, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("invalid user ID in token".into()))?;

    // Get existing credentials to exclude them from new registration
    let exclude = CredentialRepository::new(state.db.clone())
        .list_for_user(&domain::user::UserId(user_id))
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

    let (ccr, reg_state) = state
        .webauthn
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

    // Convert ccr to JSON and remove extensions (Safari compatibility)
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

/// POST /auth/passkey/add/complete — complete passkey registration for authenticated user
async fn add_passkey_complete(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<AddPasskeyCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map(domain::user::UserId)
        .map_err(|_| AppError::BadRequest("invalid user ID in token".into()))?;

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
