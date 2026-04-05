use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Free,
    Member,
    Administrator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
    PendingVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub status: UserStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl User {
    pub fn new(email: String, display_name: String) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: UserId::new(),
            email,
            display_name,
            role: Role::Free,
            status: UserStatus::PendingVerification,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_user_defaults() {
        let u = User::new("alice@example.com".into(), "Alice".into());
        assert_eq!(u.email, "alice@example.com");
        assert_eq!(u.display_name, "Alice");
        assert_eq!(u.role, Role::Free);
        assert_eq!(u.status, UserStatus::PendingVerification);
        assert_eq!(u.created_at, u.updated_at);
    }

    #[test]
    fn user_id_display_is_hyphenated_uuid() {
        let id = UserId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
        assert_eq!(s.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn user_id_new_is_unique() {
        assert_ne!(UserId::new(), UserId::new());
    }

    #[test]
    fn role_serde_round_trip() {
        for role in [Role::Free, Role::Member, Role::Administrator] {
            let back: Role = serde_json::from_str(&serde_json::to_string(&role).unwrap()).unwrap();
            assert_eq!(role, back);
        }
    }

    #[test]
    fn role_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&Role::Free).unwrap(), "\"free\"");
        assert_eq!(serde_json::to_string(&Role::Member).unwrap(), "\"member\"");
        assert_eq!(serde_json::to_string(&Role::Administrator).unwrap(), "\"administrator\"");
    }

    #[test]
    fn user_status_serde_round_trip() {
        for status in [UserStatus::Active, UserStatus::Suspended, UserStatus::PendingVerification] {
            let back: UserStatus =
                serde_json::from_str(&serde_json::to_string(&status).unwrap()).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn user_status_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&UserStatus::PendingVerification).unwrap(),
            "\"pending_verification\""
        );
    }
}
