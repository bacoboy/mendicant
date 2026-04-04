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
