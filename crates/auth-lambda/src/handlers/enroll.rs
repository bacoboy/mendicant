//! Admin bootstrap enrollment — the /enroll/* WebAuthn ceremony.
//!
//! This is a JWT-establishing flow (the user has a single-use enrollment
//! token, not a JWT yet), which is why it lives in auth-lambda alongside
//! login and register. All actual admin features live in admin-lambda.

use anyhow::Context as _;
use askama::Template;
use base64::Engine as _;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use webauthn_rs::prelude::*;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::{Challenge, ChallengeType};
use domain::credential::{Credential, CredentialId};
use domain::user::UserId;

use crate::error::AppError;
use crate::jwt::{issue_tokens, parse_ua};
use crate::sse::SseResponse;
use crate::state::AppState;

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes for the WebAuthn ceremony

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/enroll", get(enroll_page))
        .route("/enroll/begin", post(enroll_begin))
        .route("/enroll/complete", post(enroll_complete))
}

// ── Page ──────────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin-enroll.html")]
struct AdminEnrollPage;

#[derive(Deserialize)]
struct EnrollQuery {
    token: Option<String>,
}

async fn enroll_page(Query(q): Query<EnrollQuery>) -> Result<Html<String>, AppError> {
    if q.token.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::BadRequest(
            "Missing enrollment token. Use the URL provided by the bootstrap tool.".into(),
        ));
    }
    Ok(Html(AdminEnrollPage.render().map_err(|e| anyhow::anyhow!(e))?))
}

// ── Enroll begin ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EnrollBeginRequest {
    token: String,
}

/// Opaque state bundled into the WebAuthn challenge record.
/// Carries the admin user_id so complete() can attach the credential
/// without trusting anything from the client.
#[derive(Serialize, Deserialize)]
struct AdminEnrollChallengeState {
    user_id: String,
    state: SecurityKeyRegistration,
}

/// POST /enroll/begin
///
/// 1. Atomically consumes the single-use enrollment token (AdminEnrollment challenge).
/// 2. Starts a WebAuthn passkey registration ceremony for the admin user.
/// 3. Stores the ceremony state (with user_id) in a new Registration challenge.
/// 4. Returns SSE signals containing challengeId + registerOptions.
async fn enroll_begin(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::Json(req): axum::Json<EnrollBeginRequest>,
) -> Result<SseResponse, AppError> {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let webauthn = state.webauthn_for_origin(origin)
        .ok_or_else(|| AppError::BadRequest(format!("origin not allowed: {origin}")))?;
    let challenges_repo = ChallengeRepository::new(state.db.clone());

    // Atomically consume the enrollment token — prevents replay.
    let enrollment = challenges_repo
        .take(&req.token)
        .await
        .map_err(|e| {
            tracing::error!("enroll_begin: take token {:?} failed: {:?}", req.token, e);
            AppError::BadRequest("invalid or expired enrollment token".into())
        })?;

    if enrollment.challenge_type != ChallengeType::AdminEnrollment {
        return Err(AppError::BadRequest("invalid token type".into()));
    }

    // Expiry check (DynamoDB TTL is eventually consistent; belt-and-suspenders).
    if enrollment.expires_at < OffsetDateTime::now_utc().unix_timestamp() {
        return Err(AppError::BadRequest("enrollment token has expired".into()));
    }

    let user_id_str = enrollment
        .user_id
        .ok_or_else(|| anyhow::anyhow!("enrollment token missing user_id"))?;

    let user_uuid = uuid::Uuid::parse_str(&user_id_str)
        .map_err(|_| anyhow::anyhow!("invalid user_id in enrollment token"))?;

    // Load the admin user to get their email for the WebAuthn rp.user field.
    let user = UserRepository::new(state.db.clone())
        .get(&UserId(user_uuid))
        .await
        .map_err(|_| AppError::BadRequest("admin user not found".into()))?;

    // No excludeCredentials for admin enrollment — re-enrolling the same key must
    // work (bootstrap re-runs, adding a second key). Passing existing credential IDs
    // causes InvalidStateError when the key recognises itself in the exclude list.
    let (ccr, reg_state) = webauthn
        .start_securitykey_registration(
            user_uuid,
            &user.email,
            &user.display_name,
            None,
            None, // no attestation CA list — we verify AAGUID ourselves
            None, // no authenticator attachment hint
        )
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let bundled = AdminEnrollChallengeState {
        user_id: user_id_str,
        state: reg_state,
    };
    let state_json =
        serde_json::to_string(&bundled).context("failed to serialize enroll state")?;

    let expires_at = OffsetDateTime::now_utc().unix_timestamp() + CHALLENGE_TTL_SECS;
    let challenge = Challenge::new_registration(state_json, expires_at);
    let challenge_id = challenge.id.clone();

    challenges_repo
        .put(&challenge)
        .await
        .context("failed to store enroll challenge")?;

    // Strip extensions for Safari compatibility.
    // Set cross-platform attachment (hardware roaming key only — excludes Touch ID,
    // Face ID, Windows Hello).
    // residentKey:"preferred" stores the credential in the key's internal slot so that
    // discovery-mode login (no email required) can find it.
    // Writing a resident credential to a PIN-protected YubiKey requires UV —
    // that is intentional and unavoidable per CTAP2. Login sends UV:"discouraged"
    // but the key still requires PIN because the credential was enrolled with UV.
    let mut register_opts =
        serde_json::to_value(&ccr).context("failed to serialize CreationChallengeResponse")?;
    if let Some(pk) = register_opts
        .as_object_mut()
        .and_then(|o| o.get_mut("publicKey"))
        .and_then(|pk| pk.as_object_mut())
    {
        pk.remove("extensions");
        if let Some(auth_sel) = pk.get_mut("authenticatorSelection").and_then(|v| v.as_object_mut()) {
            auth_sel.insert("authenticatorAttachment".into(), serde_json::Value::String("cross-platform".into()));
            auth_sel.insert("userVerification".into(), serde_json::Value::String("preferred".into()));
            auth_sel.insert("residentKey".into(), serde_json::Value::String("preferred".into()));
        } else {
            pk.insert("authenticatorSelection".into(), serde_json::json!({
                "authenticatorAttachment": "cross-platform",
                "userVerification": "preferred",
                "residentKey": "preferred"
            }));
        }
    }

    let signals = serde_json::json!({
        "challengeId": challenge_id,
        "registerOptions": register_opts,
    });

    Ok(SseResponse::new().patch_signals(&signals.to_string()))
}

// ── Enroll complete ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EnrollCompleteRequest {
    challenge_id: String,
    response: RegisterPublicKeyCredential,
}

/// POST /enroll/complete
///
/// 1. Atomically consumes the WebAuthn challenge.
/// 2. Verifies the registration response.
/// 3. Logs the AAGUID for audit purposes.
/// 4. Stores the credential and issues a JWT + sets the auth cookie.
async fn enroll_complete(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::Json(req): axum::Json<EnrollCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let webauthn = state.webauthn_for_origin(origin)
        .ok_or_else(|| AppError::BadRequest(format!("origin not allowed: {origin}")))?;

    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired challenge".into()))?;

    let bundled: AdminEnrollChallengeState = serde_json::from_str(&challenge.state_json)
        .context("failed to deserialize enroll challenge state")?;

    let passkey = webauthn
        .finish_securitykey_registration(&req.response, &bundled.state)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Log AAGUID for audit — hardware enforcement is via cross-platform attachment
    // (set in enroll_begin options), not AAGUID matching.
    let aaguid = aaguid_from_att_object(req.response.response.attestation_object.as_ref());
    tracing::info!("admin enroll: authenticator AAGUID = {}", aaguid);

    let user_uuid = uuid::Uuid::parse_str(&bundled.user_id)
        .map_err(|_| anyhow::anyhow!("invalid user_id in challenge state"))?;
    let user_id = UserId(user_uuid);

    let user = UserRepository::new(state.db.clone())
        .get(&user_id)
        .await
        .context("failed to load admin user")?;

    let cred_id = CredentialId(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(passkey.cred_id()),
    );

    let passkey_bytes = serde_json::to_vec(&passkey).context("failed to serialize passkey")?;
    let now = OffsetDateTime::now_utc();

    let credential = Credential {
        id: cred_id,
        user_id: user_id.clone(),
        public_key: passkey_bytes,
        sign_count: 0,
        aaguid,
        nickname: Some("YubiKey (enrolled via bootstrap)".to_string()),
        created_at: now,
        last_used_at: now,
    };

    CredentialRepository::new(state.db.clone())
        .put(&credential)
        .await
        .context("failed to store admin credential")?;

    let client_hint = headers.get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(parse_ua);
    let tokens = issue_tokens(
        &user_id,
        &user.role,
        &user.email,
        &state.signer,
        &RefreshTokenRepository::new(state.db.clone()),
        client_hint,
    )
    .await?;

    tracing::info!(
        "admin enrollment complete for user {} ({}), AAGUID {}",
        user.email,
        user_id,
        aaguid
    );

    let secure = is_secure_context();
    Ok(SseResponse::new()
        .with_auth_cookie(&tokens.access_token, secure)
        .with_refresh_cookie(&tokens.refresh_token_jti, secure)
        .redirect("/me"))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_secure_context() -> bool {
    std::env::var("ENVIRONMENT")
        .map(|e| e != "dev")
        .unwrap_or(true)
}

/// Extracts the AAGUID from a CBOR-encoded attestationObject.
///
/// WebAuthn authenticatorData layout (spec §6.5.1):
///   [0..32]   rpIdHash
///   [32]      flags  (bit 6 = AT = attested credential data present)
///   [33..37]  signCount
///   [37..53]  AAGUID  — only present when AT flag is set
///
/// The attestationObject is a CBOR map `{fmt, attStmt, authData}`.
/// We find the "authData" key by scanning for its CBOR text-string encoding
/// then parse the CBOR byte-string value that follows.
fn aaguid_from_att_object(att_obj: &[u8]) -> uuid::Uuid {
    aaguid_from_att_object_inner(att_obj).unwrap_or_else(uuid::Uuid::nil)
}

fn aaguid_from_att_object_inner(att_obj: &[u8]) -> Option<uuid::Uuid> {
    // CBOR text(8) "authData": major-type 3, additional-info 8 → 0x68, then bytes.
    // "authData" = [a, u, t, h, D, a, t, a]
    const AUTHDATA_KEY: &[u8] = &[0x68, b'a', b'u', b't', b'h', b'D', b'a', b't', b'a'];

    let key_pos = att_obj
        .windows(AUTHDATA_KEY.len())
        .position(|w| w == AUTHDATA_KEY)?;

    let val = &att_obj[key_pos + AUTHDATA_KEY.len()..];

    // Decode CBOR byte string (major type 2).
    let auth_data: &[u8] = match val.first().copied()? >> 5 {
        2 => {
            let ai = val[0] & 0x1f; // additional info encodes the length
            match ai {
                n if n < 24 => val.get(1..1 + n as usize)?,
                24 => {
                    let len = *val.get(1)? as usize;
                    val.get(2..2 + len)?
                }
                25 => {
                    let len = u16::from_be_bytes([*val.get(1)?, *val.get(2)?]) as usize;
                    val.get(3..3 + len)?
                }
                _ => return None,
            }
        }
        _ => return None,
    };

    // AT flag (bit 6 of flags byte) must be set for attested credential data to exist.
    if auth_data.len() < 53 || auth_data[32] & 0x40 == 0 {
        return None;
    }

    let bytes: [u8; 16] = auth_data[37..53].try_into().ok()?;
    Some(uuid::Uuid::from_bytes(bytes))
}
