/// JWT access token and refresh token issuance.
use anyhow::{Context, Result};
use time::OffsetDateTime;

use db::refresh_tokens::RefreshTokenRepository;
use domain::token::{AccessTokenClaims, RefreshToken};
use domain::user::{Role, UserId};

use crate::signing::Signer;

pub const ACCESS_TOKEN_LIFETIME_SECS: i64 = 15 * 60;       // 15 minutes
pub const REFRESH_TOKEN_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60; // 30 days

pub struct IssuedTokens {
    pub access_token: String,
    pub refresh_token_jti: String,
    pub expires_in: i64,
}

/// Issue an access token + refresh token pair for the given user.
/// The refresh token is persisted to DynamoDB.
pub async fn issue_tokens(
    user_id: &UserId,
    role: &Role,
    email: &str,
    signer: &Signer,
    refresh_repo: &RefreshTokenRepository,
    client_hint: Option<String>,
) -> Result<IssuedTokens> {
    let now = OffsetDateTime::now_utc().unix_timestamp();

    let claims = AccessTokenClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + ACCESS_TOKEN_LIFETIME_SECS,
        jti: uuid::Uuid::new_v4().to_string(),
        email: email.to_string(),
        role: role.clone(),
    };

    let access_token = signer.sign_jwt(&claims).await
        .context("failed to sign access token")?;

    let refresh = RefreshToken::new(
        user_id.clone(),
        now + REFRESH_TOKEN_LIFETIME_SECS,
        client_hint,
    );

    refresh_repo.put(&refresh).await
        .context("failed to store refresh token")?;

    Ok(IssuedTokens {
        access_token,
        refresh_token_jti: refresh.jti,
        expires_in: ACCESS_TOKEN_LIFETIME_SECS,
    })
}

/// Parse a User-Agent header into a short human-readable session label.
/// The result is stored once at token creation time.
pub fn parse_ua(ua: &str) -> String {
    // CLI / non-browser clients
    if ua.is_empty()                { return "Unknown client".into(); }
    if ua.starts_with("curl/")      { return "curl".into(); }
    if ua.contains("python-httpx")
        || ua.contains("python-requests") { return "Python".into(); }
    if ua.to_lowercase().contains("go-http-client") { return "Go".into(); }
    // Our own CLI will set a recognisable UA eventually; catch generic API clients
    if !ua.contains("Mozilla/")     { return "API client".into(); }

    // OS / device
    let os = if ua.contains("iPhone") {
        "iPhone"
    } else if ua.contains("iPad") {
        "iPad"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        "macOS"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Unknown OS"
    };

    // Browser — order matters: Edge contains "Chrome", Chrome contains "Safari"
    let browser = if ua.contains("Edg/") || ua.contains("EdgA/") {
        "Edge"
    } else if ua.contains("OPR/") || ua.contains("Opera/") {
        "Opera"
    } else if ua.contains("CriOS/") || ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("FxiOS/") || ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        "Browser"
    };

    format!("{} · {}", browser, os)
}
