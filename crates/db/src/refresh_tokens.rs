use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;
use uuid::Uuid;

use domain::token::RefreshToken;
use domain::user::{Role, UserId};

use crate::attr::{Item, get_bool, get_n_i64, get_s};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error, map_update_error};

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(jti: &str) -> AttributeValue {
    AttributeValue::S(format!("TOKEN#{jti}"))
}

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn token_to_item(t: &RefreshToken) -> Item {
    let mut m = HashMap::new();
    m.insert("pk".into(), pk(&t.jti));
    m.insert("jti".into(), AttributeValue::S(t.jti.clone()));
    m.insert("user_id".into(), AttributeValue::S(t.user_id.to_string()));
    m.insert("role".into(), AttributeValue::S(role_to_str(&t.role).into()));
    m.insert("expires_at".into(), AttributeValue::N(t.expires_at.to_string()));
    m.insert("revoked".into(), AttributeValue::Bool(t.revoked));
    m
}

fn item_to_token(item: Item) -> Result<RefreshToken, DbError> {
    Ok(RefreshToken {
        jti: get_s(&item, "jti")?,
        user_id: UserId(
            Uuid::parse_str(&get_s(&item, "user_id")?)
                .map_err(|e| DbError::Serde(e.to_string()))?,
        ),
        role: str_to_role(&get_s(&item, "role")?)?,
        expires_at: get_n_i64(&item, "expires_at")?,
        revoked: get_bool(&item, "revoked")?,
    })
}

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

// ── Repository ────────────────────────────────────────────────────────────────

pub struct RefreshTokenRepository {
    pub db: DynamoClient,
}

impl RefreshTokenRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get(&self, jti: &str) -> Result<RefreshToken, DbError> {
        let resp = self.db.inner
            .get_item()
            .table_name(&self.db.refresh_tokens_table)
            .key("pk", pk(jti))
            .send()
            .await?;

        item_to_token(resp.item.ok_or(DbError::NotFound)?)
    }

    pub async fn put(&self, token: &RefreshToken) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.refresh_tokens_table)
            .set_item(Some(token_to_item(token)))
            // Prevent overwriting an existing token (e.g. replayed JTI).
            .condition_expression("attribute_not_exists(pk)")
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    pub async fn revoke(&self, jti: &str) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.refresh_tokens_table)
            .key("pk", pk(jti))
            .update_expression("SET revoked = :t")
            .expression_attribute_values(":t", AttributeValue::Bool(true))
            .condition_expression("attribute_exists(pk)")
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }

    /// Revoke all active refresh tokens for a user (e.g. on account suspension).
    pub async fn revoke_all_for_user(&self, user_id: &UserId) -> Result<(), DbError> {
        let resp = self.db.inner
            .query()
            .table_name(&self.db.refresh_tokens_table)
            .index_name("user-index")
            .key_condition_expression("user_id = :uid")
            .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
            .filter_expression("revoked = :f")
            .expression_attribute_values(":f", AttributeValue::Bool(false))
            .send()
            .await?;

        for item in resp.items.unwrap_or_default() {
            match item_to_token(item) {
                Ok(token) => {
                    if let Err(e) = self.revoke(&token.jti).await {
                        tracing::warn!(
                            jti = %token.jti,
                            error = %e,
                            "failed to revoke token during bulk revocation"
                        );
                    }
                }
                Err(e) => tracing::warn!(error = %e, "skipping malformed token during bulk revocation"),
            }
        }

        Ok(())
    }
}
