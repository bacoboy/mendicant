use askama::Template;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use anyhow::Context as _;
use webauthn_rs::prelude::*;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::{Challenge, ChallengeType};
use domain::credential::{Credential, CredentialId};
use domain::user::UserId;

use crate::error::AppError;
use crate::jwt::issue_tokens;
use crate::sse::SseResponse;
use crate::state::AppState;

// ── Yubico AAGUID allow-list ──────────────────────────────────────────────────
//
// These were sourced from the FIDO Alliance Metadata Service (MDS3).
// Run in dev mode (ENVIRONMENT=dev) to log your key's AAGUID, then update this
// list or set the ALLOWED_AAGUIDS env var (comma-separated UUIDs) to override.
//
// Full list: https://support.yubico.com/hc/en-us/articles/360016648959
const DEFAULT_YUBIKEY_AAGUIDS: &[&str] = &[
    "2fc0579f-8113-47ea-b116-bb5a8db9202a", // YubiKey 5 Series
    "fa2b99dc-9e39-4257-8f92-4a30d23c4118", // YubiKey 5 NFC
    "73bb0cd4-e502-49b8-9c6f-b59445bf720b", // YubiKey 5C NFC
    "c1f9a0bc-1dd2-404a-b27f-8e29047a43fd", // YubiKey 5Ci
    "cb69481e-8ff7-4039-93ec-0a2729a154a8", // YubiKey 5 Nano
    "0bb43545-fd2c-4185-87dd-feb0b2916ace", // YubiKey 5C Nano
    "b92c3f9a-c014-4056-887f-140a2501163b", // YubiKey 5C
    "6d44ba9b-f6ec-2e49-b930-0c8fe920cb73", // Security Key NFC by Yubico
    "f8a011f3-8c0a-4d15-8006-17111f9edc7d", // Security Key by Yubico
    "ee882879-721c-4913-9775-3dfcce97072a", // YubiKey 5.4 Series
    "d8522d9f-575b-4866-88a9-ba99fa02f35b", // YubiKey Bio Series
];

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes for the WebAuthn ceremony

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/enroll", get(enroll_page))
        .route("/admin/enroll/begin", post(enroll_begin))
        .route("/admin/enroll/complete", post(enroll_complete))
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
    state: PasskeyRegistration,
}

/// POST /admin/enroll/begin
///
/// 1. Atomically consumes the single-use enrollment token (AdminEnrollment challenge).
/// 2. Starts a WebAuthn passkey registration ceremony for the admin user.
/// 3. Stores the ceremony state (with user_id) in a new Registration challenge.
/// 4. Returns SSE signals containing challengeId + registerOptions.
async fn enroll_begin(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<EnrollBeginRequest>,
) -> Result<SseResponse, AppError> {
    let challenges_repo = ChallengeRepository::new(state.db.clone());

    // Atomically consume the enrollment token — prevents replay.
    let enrollment = challenges_repo
        .take(&req.token)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired enrollment token".into()))?;

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

    // Exclude any credentials already registered for this user.
    let exclude = CredentialRepository::new(state.db.clone())
        .list_for_user(&user.id)
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
        .start_passkey_registration(user_uuid, &user.email, &user.display_name, exclude)
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

    // Strip extensions for Safari compatibility (same as regular registration).
    let mut register_opts =
        serde_json::to_value(&ccr).context("failed to serialize CreationChallengeResponse")?;
    if let Some(pk) = register_opts
        .as_object_mut()
        .and_then(|o| o.get_mut("publicKey"))
        .and_then(|pk| pk.as_object_mut())
    {
        pk.remove("extensions");
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

/// POST /admin/enroll/complete
///
/// 1. Atomically consumes the WebAuthn challenge.
/// 2. Verifies the registration response.
/// 3. In non-dev environments, rejects any authenticator whose AAGUID is not
///    in the Yubico allow-list (or ALLOWED_AAGUIDS env var override).
/// 4. Stores the credential and issues a JWT + sets the auth cookie.
async fn enroll_complete(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<EnrollCompleteRequest>,
) -> Result<SseResponse, AppError> {
    let challenge = ChallengeRepository::new(state.db.clone())
        .take(&req.challenge_id)
        .await
        .map_err(|_| AppError::BadRequest("invalid or expired challenge".into()))?;

    let bundled: AdminEnrollChallengeState = serde_json::from_str(&challenge.state_json)
        .context("failed to deserialize enroll challenge state")?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.response, &bundled.state)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // ── AAGUID enforcement ────────────────────────────────────────────────────
    // webauthn-rs 0.5 doesn't expose aaguid() on Passkey, so we parse it
    // directly from the authenticatorData bytes inside the attestationObject.
    let aaguid = aaguid_from_att_object(req.response.response.attestation_object.as_ref());
    tracing::info!("admin enroll: authenticator AAGUID = {}", aaguid);

    if !is_allowed_aaguid(&aaguid) {
        tracing::warn!(
            "admin enroll rejected: AAGUID {} is not in the YubiKey allow-list",
            aaguid
        );
        return Err(AppError::BadRequest(
            "Only hardware YubiKeys are accepted for administrator enrollment. \
             If you are using a valid YubiKey, add its AAGUID to the ALLOWED_AAGUIDS \
             environment variable."
                .into(),
        ));
    }

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
        nickname: Some("YubiKey (enrolled via bootstrap)".into()),
        created_at: now,
        last_used_at: now,
    };

    CredentialRepository::new(state.db.clone())
        .put(&credential)
        .await
        .context("failed to store admin credential")?;

    let tokens = issue_tokens(
        &user_id,
        &user.role,
        &user.email,
        &state.signer,
        &RefreshTokenRepository::new(state.db.clone()),
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
        .redirect("/me"))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if `aaguid` is acceptable for admin enrollment.
///
/// In local dev (ENVIRONMENT=dev) the check is skipped so you can enrol a
/// software authenticator for testing. In all other environments, only
/// AAGUIDs from `ALLOWED_AAGUIDS` (comma-separated env var) or the built-in
/// Yubico list are accepted.
fn is_allowed_aaguid(aaguid: &uuid::Uuid) -> bool {
    if !is_secure_context() {
        tracing::warn!(
            "AAGUID check skipped (dev environment). \
             Authenticator AAGUID: {}. Add this to ALLOWED_AAGUIDS for production.",
            aaguid
        );
        return true;
    }

    let aaguid_str = aaguid.to_string();

    if let Ok(allowed_env) = std::env::var("ALLOWED_AAGUIDS") {
        return allowed_env
            .split(',')
            .map(|s| s.trim())
            .any(|s| s == aaguid_str);
    }

    DEFAULT_YUBIKEY_AAGUIDS.iter().any(|&s| s == aaguid_str)
}

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
