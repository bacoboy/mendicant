use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::user::UserId;

/// Opaque credential ID as issued by the authenticator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialId(pub String);

/// A stored WebAuthn passkey credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: CredentialId,
    pub user_id: UserId,
    /// CBOR-encoded public key (stored as returned by webauthn-rs).
    pub public_key: Vec<u8>,
    pub sign_count: u32,
    /// AAGUID identifies the authenticator model.
    pub aaguid: Uuid,
    /// Human-readable name the user gave this key (e.g. "iPhone", "YubiKey").
    pub nickname: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_used_at: OffsetDateTime,
}
