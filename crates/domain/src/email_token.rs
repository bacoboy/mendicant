use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A short-lived email validation token sent to a user during registration.
/// Stored in the regional email_tokens table with a TTL of ~15 minutes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailToken {
    pub id: String,
    pub email: String,
    pub display_name: String,
    /// Unix timestamp — also used as DynamoDB TTL attribute.
    pub expires_at: i64,
}

impl EmailToken {
    pub fn new(email: String, display_name: String, expires_at: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            email,
            display_name,
            expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_token_has_unique_id() {
        let a = EmailToken::new("test@example.com".into(), "Alice".into(), 9999999999);
        let b = EmailToken::new("test@example.com".into(), "Alice".into(), 9999999999);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn email_token_stores_email_and_display_name() {
        let token = EmailToken::new("alice@example.com".into(), "Alice".into(), 9999999999);
        assert_eq!(token.email, "alice@example.com");
        assert_eq!(token.display_name, "Alice");
    }
}
