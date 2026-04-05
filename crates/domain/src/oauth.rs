use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an OAuth 2.0 device authorization request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceGrantStatus {
    /// Waiting for user to authenticate.
    Pending,
    /// User authenticated and approved.
    Approved,
    /// User denied or grant expired.
    Denied,
}

/// An in-flight OAuth 2.0 Device Authorization Grant (RFC 8628).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceGrant {
    /// Opaque code sent to the device (CLI). Not shown to the user.
    pub device_code: String,
    /// Short, human-typable code displayed to the user.
    pub user_code: String,
    pub status: DeviceGrantStatus,
    /// Unix timestamp when this grant expires (also DynamoDB TTL).
    pub expires_at: i64,
    /// Populated once the user approves.
    pub user_id: Option<String>,
}

impl DeviceGrant {
    pub fn new(expires_at: i64) -> Self {
        Self {
            device_code: Uuid::new_v4().to_string(),
            user_code: Self::generate_user_code(),
            status: DeviceGrantStatus::Pending,
            expires_at,
            user_id: None,
        }
    }

    /// Generates an 8-character alphanumeric code in XXXX-XXXX format.
    fn generate_user_code() -> String {
        use std::fmt::Write;
        let id = Uuid::new_v4().simple().to_string();
        let chars: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).take(8)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        let mut code = String::new();
        let _ = write!(code, "{}-{}", &chars[..4], &chars[4..8]);
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_grant_starts_pending_with_no_user() {
        let grant = DeviceGrant::new(9999999999);
        assert_eq!(grant.status, DeviceGrantStatus::Pending);
        assert!(grant.user_id.is_none());
        assert_eq!(grant.expires_at, 9999999999);
    }

    #[test]
    fn user_code_matches_xxxx_xxxx_format() {
        let grant = DeviceGrant::new(0);
        let parts: Vec<&str> = grant.user_code.split('-').collect();
        assert_eq!(parts.len(), 2, "user_code should contain exactly one dash");
        assert_eq!(parts[0].len(), 4);
        assert_eq!(parts[1].len(), 4);
        assert!(parts[0].chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()));
        assert!(parts[1].chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()));
    }

    #[test]
    fn each_grant_has_unique_codes() {
        let a = DeviceGrant::new(0);
        let b = DeviceGrant::new(0);
        assert_ne!(a.device_code, b.device_code);
        assert_ne!(a.user_code, b.user_code);
    }

    #[test]
    fn device_grant_status_serde_round_trip() {
        for status in [
            DeviceGrantStatus::Pending,
            DeviceGrantStatus::Approved,
            DeviceGrantStatus::Denied,
        ] {
            let back: DeviceGrantStatus =
                serde_json::from_str(&serde_json::to_string(&status).unwrap()).unwrap();
            assert_eq!(status, back);
        }
    }
}
