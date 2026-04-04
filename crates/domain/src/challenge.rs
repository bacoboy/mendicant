use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeType {
    Registration,
    Authentication,
}

/// A short-lived WebAuthn challenge stored in the regional challenges table.
///
/// `state_json` is the opaque JSON-serialized webauthn-rs state
/// (`PasskeyRegistration` or `PasskeyAuthentication`). It is produced and
/// consumed exclusively by `auth-lambda` — the `db` crate treats it as an
/// opaque string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub challenge_type: ChallengeType,
    /// JSON-serialized webauthn-rs ceremony state.
    pub state_json: String,
    /// Present for authentication challenges (user is already known).
    /// Absent for registration challenges (user may not exist yet).
    pub user_id: Option<String>,
    /// Unix timestamp — also used as DynamoDB TTL attribute.
    pub expires_at: i64,
}

impl Challenge {
    pub fn new_registration(state_json: String, expires_at: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            challenge_type: ChallengeType::Registration,
            state_json,
            user_id: None,
            expires_at,
        }
    }

    pub fn new_authentication(user_id: String, state_json: String, expires_at: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            challenge_type: ChallengeType::Authentication,
            state_json,
            user_id: Some(user_id),
            expires_at,
        }
    }
}
