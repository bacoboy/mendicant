use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::user::{Role, UserId};

/// Claims embedded in a signed JWT access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// Subject — the user's ID.
    pub sub: String,
    /// Issued-at (Unix timestamp).
    pub iat: i64,
    /// Expiry (Unix timestamp).
    pub exp: i64,
    /// JWT ID — unique per token, used for revocation lookup.
    pub jti: String,
    pub email: String,
    pub role: Role,
}

/// A refresh token record stored in DynamoDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    /// The JWT ID this refresh token is bound to.
    pub jti: String,
    pub user_id: UserId,
    pub role: Role,
    /// Unix timestamp when this token expires (also the DynamoDB TTL value).
    pub expires_at: i64,
    pub revoked: bool,
}

impl RefreshToken {
    pub fn new(user_id: UserId, role: Role, expires_at: i64) -> Self {
        Self {
            jti: Uuid::new_v4().to_string(),
            user_id,
            role,
            expires_at,
            revoked: false,
        }
    }
}
