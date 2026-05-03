use askama::Template;
use aws_sdk_dynamodb::types::AttributeValue;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use anyhow::Context as _;
use webauthn_rs::prelude::*;

use db::challenges::ChallengeRepository;
use db::credentials::CredentialRepository;
use db::refresh_tokens::RefreshTokenRepository;
use db::users::UserRepository;
use domain::challenge::{Challenge, ChallengeType};
use domain::credential::{Credential, CredentialId};
use domain::user::{Role, UserId};

use crate::error::AppError;
use crate::jwt::issue_tokens;
use crate::middleware::AuthUser;
use crate::sse::SseResponse;
use crate::state::AppState;

// ── Hardware key enforcement ──────────────────────────────────────────────────
//
// Admin enrollment requires a physical roaming authenticator (USB/NFC security
// key). We enforce this by:
//   1. Requesting authenticatorAttachment:"cross-platform" — excludes Touch ID,
//      Face ID, and Windows Hello (platform authenticators).
//   2. Requesting direct attestation — gives us the real AAGUID from the key.
//   3. Rejecting a nil AAGUID (00000000-…) — a nil AAGUID means the browser
//      zeroed it out (privacy-preserving no-attestation path), which we don't
//      accept for admin enrollment.
//
// This allows any hardware roaming key (YubiKey, Feitian, etc.) without
// maintaining a specific allow-list. Set ALLOWED_AAGUIDS (comma-separated) to
// restrict to specific models if needed.

const CHALLENGE_TTL_SECS: i64 = 300; // 5 minutes for the WebAuthn ceremony

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(admin_page))
        .route("/admin/tables/{table}", get(table_page))
        .route("/admin/enroll", get(enroll_page))
        .route("/admin/enroll/begin", post(enroll_begin))
        .route("/admin/enroll/complete", post(enroll_complete))
}

// ── Admin landing page ────────────────────────────────────────────────────────

pub struct TableInfo {
    pub slug: &'static str,
    pub name: String,
    pub scope: &'static str,
    pub status: String,
    pub item_count: String,
    pub size: String,
    pub billing_mode: String,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminPage {
    tables: Vec<TableInfo>,
}

async fn admin_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Html<String>, AppError> {
    if claims.role != Role::Administrator {
        return Err(AppError::Forbidden);
    }

    let tables_config: &[(&str, &'static str, &'static str)] = &[
        (&state.db.users_table, "users", "Global"),
        (&state.db.credentials_table, "credentials", "Global"),
        (&state.db.refresh_tokens_table, "refresh-tokens", "Global"),
        (&state.db.challenges_table, "challenges", "Regional"),
        (&state.db.email_tokens_table, "email-tokens", "Regional"),
        (&state.db.oauth_devices_table, "oauth-devices", "Regional"),
    ];

    let mut tables = Vec::with_capacity(tables_config.len());
    for (table_name, slug, scope) in tables_config {
        let info = match state.db.inner.describe_table().table_name(*table_name).send().await {
            Ok(resp) => {
                let td = resp.table();
                let item_count = td.and_then(|t| t.item_count()).unwrap_or(0);
                let size_bytes = td.and_then(|t| t.table_size_bytes()).unwrap_or(0);
                let status = td
                    .and_then(|t| t.table_status())
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into());
                let billing_mode = td
                    .and_then(|t| t.billing_mode_summary())
                    .and_then(|b| b.billing_mode())
                    .map(|m| match m.as_str() {
                        "PAY_PER_REQUEST" => "On-demand".into(),
                        "PROVISIONED" => "Provisioned".into(),
                        other => other.to_string(),
                    })
                    .unwrap_or_else(|| "unknown".into());

                TableInfo {
                    slug,
                    name: table_name.to_string(),
                    scope,
                    status,
                    item_count: format_number(item_count),
                    size: format_bytes(size_bytes),
                    billing_mode,
                }
            }
            Err(e) => {
                tracing::error!("describe_table failed for {}: {}", table_name, e);
                TableInfo {
                    slug,
                    name: table_name.to_string(),
                    scope,
                    status: "error".into(),
                    item_count: "—".into(),
                    size: "—".into(),
                    billing_mode: "—".into(),
                }
            }
        };
        tables.push(info);
    }

    Ok(Html(
        AdminPage { tables }.render().map_err(|e| anyhow::anyhow!(e))?,
    ))
}

// ── Table browse ──────────────────────────────────────────────────────────────

const PAGE_SIZE: i32 = 25;

#[derive(Deserialize)]
struct TableBrowseQuery {
    cursor: Option<String>,
}

struct UserDetail {
    uuid: String,
    credential_rows: Vec<Vec<String>>,
}

struct TableRow {
    cells: Vec<String>,
    detail: Option<UserDetail>,
}

#[derive(Template)]
#[template(path = "admin-table.html")]
struct AdminTableTemplate {
    table_name: String,
    table_slug: String,
    scope: &'static str,
    has_details: bool,
    headers: Vec<&'static str>,
    rows: Vec<TableRow>,
    next_cursor: Option<String>,
    item_count: usize,
}

async fn table_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(slug): Path<String>,
    Query(q): Query<TableBrowseQuery>,
) -> Result<Html<String>, AppError> {
    if claims.role != Role::Administrator {
        return Err(AppError::Forbidden);
    }

    let (ddb_table, scope, headers): (&str, &'static str, Vec<&'static str>) = match slug.as_str() {
        "users" => (
            &state.db.users_table,
            "Global",
            vec!["UUID", "Email", "Display Name", "Role", "Status", "Created"],
        ),
        "credentials" => (
            &state.db.credentials_table,
            "Global",
            vec!["User ID", "Nickname", "Key Type", "Sign Count", "Last Used"],
        ),
        "refresh-tokens" => (
            &state.db.refresh_tokens_table,
            "Global",
            vec!["JTI", "User ID", "Role", "Expires", "Revoked"],
        ),
        "challenges" => (
            &state.db.challenges_table,
            "Regional",
            vec!["ID", "Type", "User ID", "Expires"],
        ),
        "email-tokens" => (
            &state.db.email_tokens_table,
            "Regional",
            vec!["ID", "Email", "Expires"],
        ),
        "oauth-devices" => (
            &state.db.oauth_devices_table,
            "Regional",
            vec!["User Code", "Status", "User ID", "Expires"],
        ),
        _ => return Err(AppError::NotFound),
    };

    let mut req = state.db.inner
        .scan()
        .table_name(ddb_table)
        .limit(PAGE_SIZE);

    if let Some(ref cursor) = q.cursor {
        req = req.set_exclusive_start_key(Some(decode_browse_cursor(cursor)?));
    }

    let resp = req.send().await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let next_cursor = resp.last_evaluated_key
        .map(|k| encode_browse_cursor(&k))
        .transpose()
        .map_err(|e: anyhow::Error| AppError::Internal(e))?;

    let items = resp.items.unwrap_or_default();
    let item_count = items.len();

    let mut rows: Vec<TableRow> = Vec::with_capacity(items.len());
    for item in &items {
        let cells = match slug.as_str() {
            "users" => row_user(item),
            "credentials" => row_credential(item),
            "refresh-tokens" => row_refresh_token(item),
            "challenges" => row_challenge(item),
            "email-tokens" => row_email_token(item),
            "oauth-devices" => row_oauth_device(item),
            _ => vec![],
        };

        let detail = if slug.as_str() == "users" {
            let uuid = val_s(item, "user_id");
            let creds_resp = state.db.inner
                .query()
                .table_name(&state.db.credentials_table)
                .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
                .expression_attribute_values(":pk", AttributeValue::S(format!("USER#{uuid}")))
                .expression_attribute_values(":prefix", AttributeValue::S("CRED#".into()))
                .send()
                .await
                .ok()
                .and_then(|r| r.items)
                .unwrap_or_default();
            let credential_rows: Vec<Vec<String>> = creds_resp
                .iter()
                .map(|c| row_credential_detail(c))
                .collect();
            Some(UserDetail { uuid, credential_rows })
        } else {
            None
        };

        rows.push(TableRow { cells, detail });
    }

    let has_details = slug.as_str() == "users";

    Ok(Html(AdminTableTemplate {
        table_name: ddb_table.to_string(),
        table_slug: slug,
        scope,
        has_details,
        headers,
        rows,
        next_cursor,
        item_count,
    }.render().map_err(|e| anyhow::anyhow!(e))?))
}

// ── Row mappers ───────────────────────────────────────────────────────────────

type DdbItem = HashMap<String, AttributeValue>;

fn val_s(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_s().ok())
        .map(|s| s.as_str())
        .unwrap_or("—")
        .to_string()
}

fn val_n(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .map(|s| s.as_str())
        .unwrap_or("—")
        .to_string()
}

fn val_bool(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_bool().ok())
        .map(|b| if *b { "Yes" } else { "No" })
        .unwrap_or("—")
        .to_string()
}

fn fmt_unix(n_str: &str) -> String {
    n_str.parse::<i64>()
        .ok()
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
        .map(|dt| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} UTC",
                dt.year(), dt.month() as u8, dt.day(),
                dt.hour(), dt.minute()
            )
        })
        .unwrap_or_else(|| n_str.to_string())
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn aaguid_display(aaguid: &str) -> String {
    match aaguid {
        "2fc0579f-8113-47ea-b116-bb5a8db9202a" => "YubiKey 5 Series".into(),
        "fa2b99dc-9e39-4257-8f92-4a30d23c4118" => "YubiKey 5 NFC".into(),
        "73bb0cd4-e502-49b8-9c6f-b59445bf720b" => "YubiKey 5C NFC".into(),
        "c1f9a0bc-1dd2-404a-b27f-8e29047a43fd" => "YubiKey 5Ci".into(),
        "cb69481e-8ff7-4039-93ec-0a2729a154a8" => "YubiKey 5 Nano".into(),
        "0bb43545-fd2c-4185-87dd-feb0b2916ace" => "YubiKey 5C Nano (fw <5.7)".into(),
        "ff4dac45-ede8-4ec2-aced-cf66103f4335" => "YubiKey 5C Nano (fw 5.7+)".into(),
        "b92c3f9a-c014-4056-887f-140a2501163b" => "YubiKey 5C".into(),
        "6d44ba9b-f6ec-2e49-b930-0c8fe920cb73" => "Security Key NFC".into(),
        "f8a011f3-8c0a-4d15-8006-17111f9edc7d" => "Security Key".into(),
        "ee882879-721c-4913-9775-3dfcce97072a" => "YubiKey 5.4 Series".into(),
        "d8522d9f-575b-4866-88a9-ba99fa02f35b" => "YubiKey Bio".into(),
        "00000000-0000-0000-0000-000000000000" => "Security Key".into(),
        other => trunc(other, 18),
    }
}

fn row_user(item: &DdbItem) -> Vec<String> {
    let user_id = val_s(item, "user_id");
    vec![
        user_id,
        val_s(item, "email"),
        val_s(item, "display_name"),
        title_case(&val_s(item, "role")),
        title_case(&val_s(item, "status")),
        trunc(&val_s(item, "created_at"), 19), // drop sub-second + tz
    ]
}

fn row_credential(item: &DdbItem) -> Vec<String> {
    let user_id = val_s(item, "user_id");
    let aaguid = val_s(item, "aaguid");
    vec![
        trunc(&user_id, 8),
        val_s(item, "nickname"),
        aaguid_display(&aaguid),
        val_n(item, "sign_count"),
        trunc(&val_s(item, "last_used_at"), 19),
    ]
}

fn row_credential_detail(item: &DdbItem) -> Vec<String> {
    let aaguid = val_s(item, "aaguid");
    vec![
        val_s(item, "nickname"),
        aaguid_display(&aaguid),
        val_n(item, "sign_count"),
        trunc(&val_s(item, "last_used_at"), 19),
    ]
}

fn row_refresh_token(item: &DdbItem) -> Vec<String> {
    let jti = val_s(item, "jti");
    let user_id = val_s(item, "user_id");
    let expires_n = val_n(item, "expires_at");
    vec![
        trunc(&jti, 8),
        trunc(&user_id, 8),
        title_case(&val_s(item, "role")),
        fmt_unix(&expires_n),
        val_bool(item, "revoked"),
    ]
}

fn row_challenge(item: &DdbItem) -> Vec<String> {
    let pk_val = val_s(item, "pk");
    let id = pk_val.strip_prefix("CHALLENGE#").unwrap_or(&pk_val);
    let expires_n = val_n(item, "expires_at");
    vec![
        trunc(id, 8),
        title_case(&val_s(item, "challenge_type")),
        val_s(item, "user_id"),
        fmt_unix(&expires_n),
    ]
}

fn row_email_token(item: &DdbItem) -> Vec<String> {
    let pk_val = val_s(item, "pk");
    let id = pk_val.strip_prefix("EMAIL_TOKEN#").unwrap_or(&pk_val);
    let expires_n = val_n(item, "expires_at");
    vec![
        trunc(id, 8),
        val_s(item, "email"),
        fmt_unix(&expires_n),
    ]
}

fn row_oauth_device(item: &DdbItem) -> Vec<String> {
    let expires_n = val_n(item, "expires_at");
    vec![
        val_s(item, "user_code"),
        title_case(&val_s(item, "status")),
        val_s(item, "user_id"),
        fmt_unix(&expires_n),
    ]
}

// ── Cursor helpers ────────────────────────────────────────────────────────────

fn encode_browse_cursor(key: &DdbItem) -> Result<String, anyhow::Error> {
    // All table PKs/SKs in this schema are strings — filter out anything else.
    let simple: HashMap<String, String> = key.iter()
        .filter_map(|(k, v)| v.as_s().ok().map(|s| (k.clone(), s.clone())))
        .collect();
    let json = serde_json::to_string(&simple)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
}

fn decode_browse_cursor(cursor: &str) -> Result<DdbItem, AppError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|e| AppError::BadRequest(format!("invalid cursor: {e}")))?;
    let simple: HashMap<String, String> = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("invalid cursor: {e}")))?;
    Ok(simple.into_iter().map(|(k, v)| (k, AttributeValue::S(v))).collect())
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
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

/// POST /admin/enroll/begin
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
    // Writing a resident credential to a PIN-protected YubiKey requires UV once —
    // that is intentional and unavoidable per CTAP2. After enrollment, every login
    // is a single touch (userVerification:"discouraged" in login_begin).
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

/// POST /admin/enroll/complete
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
