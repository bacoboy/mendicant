use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeType {
    Registration,
    Authentication,
    /// One-time token created by the bootstrap tool to enrol an admin YubiKey.
    /// Stored in the challenges table; consumed atomically on enrol begin.
    AdminEnrollment,
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

    /// Creates a one-time admin enrollment token. No webauthn state — `state_json`
    /// is empty. The token is consumed atomically when the admin clicks enrol,
    /// and the `user_id` is carried into the subsequent WebAuthn challenge.
    pub fn new_admin_enrollment(user_id: String, expires_at: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            challenge_type: ChallengeType::AdminEnrollment,
            state_json: String::new(),
            user_id: Some(user_id),
            expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_challenge_has_no_user_id() {
        let c = Challenge::new_registration("{}".into(), 9999999999);
        assert_eq!(c.challenge_type, ChallengeType::Registration);
        assert!(c.user_id.is_none());
        assert_eq!(c.state_json, "{}");
    }

    #[test]
    fn authentication_challenge_carries_user_id() {
        let uid = "user-abc".to_string();
        let c = Challenge::new_authentication(uid.clone(), "{}".into(), 9999999999);
        assert_eq!(c.challenge_type, ChallengeType::Authentication);
        assert_eq!(c.user_id.as_deref(), Some("user-abc"));
    }

    #[test]
    fn each_challenge_has_unique_id() {
        let a = Challenge::new_registration("{}".into(), 0);
        let b = Challenge::new_registration("{}".into(), 0);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn admin_enrollment_challenge_carries_user_id_and_nickname() {
        let uid = "user-xyz".to_string();
        let c = Challenge::new_admin_enrollment(uid.clone(), Some("Steve's key".into()), 9999999999);
        assert_eq!(c.challenge_type, ChallengeType::AdminEnrollment);
        assert_eq!(c.user_id.as_deref(), Some("user-xyz"));
        assert_eq!(c.state_json, "Steve's key");

        let c2 = Challenge::new_admin_enrollment(uid.clone(), None, 9999999999);
        assert!(c2.state_json.is_empty());
    }
}
