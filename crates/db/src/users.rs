use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

use domain::user::{Role, User, UserId, UserStatus};

use crate::attr::{Item, get_s, get_utc};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error, map_update_error};
use crate::time_util::now_utc_rfc3339;

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(user_id: &UserId) -> AttributeValue {
    AttributeValue::S(format!("USER#{user_id}"))
}

const SK: &str = "PROFILE";

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn user_to_item(user: &User) -> Item {
    use time::format_description::well_known::Rfc3339;
    let mut m = HashMap::new();
    m.insert("pk".into(), AttributeValue::S(format!("USER#{}", user.id)));
    m.insert("sk".into(), AttributeValue::S(SK.into()));
    m.insert("user_id".into(), AttributeValue::S(user.id.to_string()));
    m.insert("email".into(), AttributeValue::S(user.email.clone()));
    m.insert("display_name".into(), AttributeValue::S(user.display_name.clone()));
    m.insert("role".into(), AttributeValue::S(role_to_str(&user.role).into()));
    m.insert("status".into(), AttributeValue::S(status_to_str(&user.status).into()));
    m.insert(
        "created_at".into(),
        AttributeValue::S(user.created_at.format(&Rfc3339).unwrap_or_default()),
    );
    m.insert(
        "updated_at".into(),
        AttributeValue::S(user.updated_at.format(&Rfc3339).unwrap_or_default()),
    );
    m
}

fn item_to_user(item: Item) -> Result<User, DbError> {
    use uuid::Uuid;

    let user_id_str = get_s(&item, "user_id")?;
    Ok(User {
        id: UserId(Uuid::parse_str(&user_id_str).map_err(|e| DbError::Serde(e.to_string()))?),
        email: get_s(&item, "email")?,
        display_name: get_s(&item, "display_name")?,
        role: str_to_role(&get_s(&item, "role")?)?,
        status: str_to_status(&get_s(&item, "status")?)?,
        created_at: get_utc(&item, "created_at")?,
        updated_at: get_utc(&item, "updated_at")?,
    })
}

// ── Role / Status ─────────────────────────────────────────────────────────────

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::Free => "free",
        Role::Member => "member",
        Role::Administrator => "administrator",
    }
}

fn str_to_role(s: &str) -> Result<Role, DbError> {
    match s {
        "free" => Ok(Role::Free),
        "member" => Ok(Role::Member),
        "administrator" => Ok(Role::Administrator),
        other => Err(DbError::Serde(format!("unknown role: {other}"))),
    }
}

fn status_to_str(status: &UserStatus) -> &'static str {
    match status {
        UserStatus::Active => "active",
        UserStatus::Suspended => "suspended",
        UserStatus::PendingVerification => "pending_verification",
    }
}

fn str_to_status(s: &str) -> Result<UserStatus, DbError> {
    match s {
        "active" => Ok(UserStatus::Active),
        "suspended" => Ok(UserStatus::Suspended),
        "pending_verification" => Ok(UserStatus::PendingVerification),
        other => Err(DbError::Serde(format!("unknown status: {other}"))),
    }
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct UserRepository {
    pub db: DynamoClient,
}

impl UserRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &UserId) -> Result<User, DbError> {
        let resp = self.db.inner
            .get_item()
            .table_name(&self.db.users_table)
            .key("pk", pk(id))
            .key("sk", AttributeValue::S(SK.into()))
            .send()
            .await?;

        item_to_user(resp.item.ok_or(DbError::NotFound)?)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<User, DbError> {
        let resp = self.db.inner
            .query()
            .table_name(&self.db.users_table)
            .index_name("sk-email-index")
            .key_condition_expression("sk = :sk AND email = :email")
            .expression_attribute_values(":sk", AttributeValue::S(SK.into()))
            .expression_attribute_values(":email", AttributeValue::S(email.into()))
            .limit(1)
            .send()
            .await?;

        let item = resp.items
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or(DbError::NotFound)?;

        item_to_user(item)
    }

    pub async fn put(&self, user: &User) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.users_table)
            .set_item(Some(user_to_item(user)))
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    pub async fn update_display_name(&self, id: &UserId, display_name: &str) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.users_table)
            .key("pk", pk(id))
            .key("sk", AttributeValue::S(SK.into()))
            .update_expression("SET display_name = :name, updated_at = :now")
            .expression_attribute_values(":name", AttributeValue::S(display_name.into()))
            .expression_attribute_values(":now", AttributeValue::S(now_utc_rfc3339()))
            .condition_expression("attribute_exists(pk)")
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }

    pub async fn update_role(&self, id: &UserId, role: &Role) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.users_table)
            .key("pk", pk(id))
            .key("sk", AttributeValue::S(SK.into()))
            .update_expression("SET #role = :role, updated_at = :now")
            .expression_attribute_names("#role", "role") // reserved word
            .expression_attribute_values(":role", AttributeValue::S(role_to_str(role).into()))
            .expression_attribute_values(":now", AttributeValue::S(now_utc_rfc3339()))
            .condition_expression("attribute_exists(pk)")
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }

    pub async fn update_status(&self, id: &UserId, status: &UserStatus) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.users_table)
            .key("pk", pk(id))
            .key("sk", AttributeValue::S(SK.into()))
            .update_expression("SET #status = :status, updated_at = :now")
            .expression_attribute_names("#status", "status") // reserved word
            .expression_attribute_values(":status", AttributeValue::S(status_to_str(status).into()))
            .expression_attribute_values(":now", AttributeValue::S(now_utc_rfc3339()))
            .condition_expression("attribute_exists(pk)")
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }

    /// Delete a user record. Call `CredentialRepository::delete_all_for_user`
    /// and `RefreshTokenRepository::revoke_all_for_user` first.
    pub async fn delete(&self, id: &UserId) -> Result<(), DbError> {
        self.db.inner
            .delete_item()
            .table_name(&self.db.users_table)
            .key("pk", pk(id))
            .key("sk", AttributeValue::S(SK.into()))
            .send()
            .await?;
        Ok(())
    }

    /// Returns up to `limit` users with cursor-based pagination using GSI queries
    /// (no table scans).
    ///
    /// Routing logic:
    /// - role filter present  → `role-index` (pk=role, sk=email)
    /// - no role filter       → `sk-email-index` (pk=sk="PROFILE", sk=email)
    ///
    /// In both cases an email prefix (`begins_with`) is pushed into the key
    /// condition when provided. Status is always a post-read FilterExpression.
    pub async fn list(
        &self,
        limit: u32,
        cursor: Option<String>,
        email_query: Option<&str>,
        role_filter: Option<&Role>,
        status_filter: Option<&UserStatus>,
    ) -> Result<(Vec<User>, Option<String>), DbError> {
        let mut req = self.db.inner
            .query()
            .table_name(&self.db.users_table)
            .limit(limit as i32);

        if let Some(token) = cursor {
            req = req.set_exclusive_start_key(Some(decode_cursor(&token)?));
        }

        // Choose GSI and build key condition.
        if let Some(role) = role_filter {
            // role-index: hash=role, range=email
            req = req
                .index_name("role-index")
                .expression_attribute_values(":role", AttributeValue::S(role_to_str(role).into()));

            if let Some(prefix) = email_query {
                req = req
                    .key_condition_expression("#role = :role AND begins_with(email, :pfx)")
                    .expression_attribute_names("#role", "role")
                    .expression_attribute_values(":pfx", AttributeValue::S(prefix.into()));
            } else {
                req = req
                    .key_condition_expression("#role = :role")
                    .expression_attribute_names("#role", "role");
            }
        } else {
            // sk-email-index: hash=sk("PROFILE"), range=email
            req = req
                .index_name("sk-email-index")
                .expression_attribute_values(":sk", AttributeValue::S(SK.into()));

            if let Some(prefix) = email_query {
                req = req
                    .key_condition_expression("sk = :sk AND begins_with(email, :pfx)")
                    .expression_attribute_values(":pfx", AttributeValue::S(prefix.into()));
            } else {
                req = req.key_condition_expression("sk = :sk");
            }
        }

        // Status filter is always a post-read filter expression.
        if let Some(status) = status_filter {
            req = req
                .filter_expression("#status = :status")
                .expression_attribute_names("#status", "status")
                .expression_attribute_values(":status", AttributeValue::S(status_to_str(status).into()));
        }

        let resp = req.send().await?;

        let next_cursor = resp.last_evaluated_key
            .map(encode_cursor)
            .transpose()?;

        let users = resp.items
            .unwrap_or_default()
            .into_iter()
            .map(item_to_user)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((users, next_cursor))
    }
}

// ── Cursor helpers ────────────────────────────────────────────────────────────

fn encode_cursor(key: Item) -> Result<String, DbError> {
    use base64::Engine as _;
    let simple: HashMap<String, String> = key
        .into_iter()
        .filter_map(|(k, v)| v.as_s().ok().map(|s| (k, s.clone())))
        .collect();
    let json = serde_json::to_string(&simple)
        .map_err(|e| DbError::Serde(e.to_string()))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
}

fn decode_cursor(cursor: &str) -> Result<Item, DbError> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|e| DbError::Serde(format!("invalid cursor: {e}")))?;
    let simple: HashMap<String, String> = serde_json::from_slice(&bytes)
        .map_err(|e| DbError::Serde(format!("invalid cursor: {e}")))?;
    Ok(simple
        .into_iter()
        .map(|(k, v)| (k, AttributeValue::S(v)))
        .collect())
}
