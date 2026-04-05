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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_token_new_not_revoked() {
        let uid = UserId::new();
        let t = RefreshToken::new(uid.clone(), Role::Free, 9999999999);
        assert!(!t.revoked);
        assert_eq!(t.user_id, uid);
        assert_eq!(t.role, Role::Free);
        assert_eq!(t.expires_at, 9999999999);
    }

    #[test]
    fn refresh_token_jti_is_unique() {
        let uid = UserId::new();
        let a = RefreshToken::new(uid.clone(), Role::Free, 0);
        let b = RefreshToken::new(uid, Role::Free, 0);
        assert_ne!(a.jti, b.jti);
    }
}
