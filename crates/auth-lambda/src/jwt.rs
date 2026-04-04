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
        role.clone(),
        now + REFRESH_TOKEN_LIFETIME_SECS,
    );

    refresh_repo.put(&refresh).await
        .context("failed to store refresh token")?;

    Ok(IssuedTokens {
        access_token,
        refresh_token_jti: refresh.jti,
        expires_in: ACCESS_TOKEN_LIFETIME_SECS,
    })
}
