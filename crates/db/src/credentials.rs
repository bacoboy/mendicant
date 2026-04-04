use aws_sdk_dynamodb::types::AttributeValue;
use base64::Engine as _;
use std::collections::HashMap;
use uuid::Uuid;

use domain::credential::{Credential, CredentialId};
use domain::user::UserId;

use crate::attr::{Item, get_n_u32, get_s, get_s_opt, get_utc};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error, map_update_error};
use crate::time_util::now_utc_rfc3339;

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(user_id: &UserId) -> AttributeValue {
    AttributeValue::S(format!("USER#{user_id}"))
}

fn sk(cred_id: &CredentialId) -> AttributeValue {
    AttributeValue::S(format!("CRED#{}", cred_id.0))
}

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn credential_to_item(c: &Credential) -> Item {
    use time::format_description::well_known::Rfc3339;
    let mut m = HashMap::new();
    m.insert("pk".into(), AttributeValue::S(format!("USER#{}", c.user_id)));
    m.insert("sk".into(), AttributeValue::S(format!("CRED#{}", c.id.0)));
    // Stored flat for the credential-id-index GSI.
    m.insert("credential_id".into(), AttributeValue::S(c.id.0.clone()));
    m.insert("user_id".into(), AttributeValue::S(c.user_id.to_string()));
    m.insert(
        "public_key".into(),
        AttributeValue::S(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&c.public_key),
        ),
    );
    m.insert("sign_count".into(), AttributeValue::N(c.sign_count.to_string()));
    m.insert("aaguid".into(), AttributeValue::S(c.aaguid.to_string()));
    if let Some(ref nick) = c.nickname {
        m.insert("nickname".into(), AttributeValue::S(nick.clone()));
    }
    m.insert(
        "created_at".into(),
        AttributeValue::S(c.created_at.format(&Rfc3339).unwrap_or_default()),
    );
    m.insert(
        "last_used_at".into(),
        AttributeValue::S(c.last_used_at.format(&Rfc3339).unwrap_or_default()),
    );
    m
}

fn item_to_credential(item: Item) -> Result<Credential, DbError> {
    let public_key = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(get_s(&item, "public_key")?)
        .map_err(|e| DbError::Serde(e.to_string()))?;

    Ok(Credential {
        id: CredentialId(get_s(&item, "credential_id")?),
        user_id: UserId(
            Uuid::parse_str(&get_s(&item, "user_id")?)
                .map_err(|e| DbError::Serde(e.to_string()))?,
        ),
        public_key,
        sign_count: get_n_u32(&item, "sign_count")?,
        aaguid: Uuid::parse_str(&get_s(&item, "aaguid")?)
            .map_err(|e| DbError::Serde(e.to_string()))?,
        nickname: get_s_opt(&item, "nickname")?,
        created_at: get_utc(&item, "created_at")?,
        last_used_at: get_utc(&item, "last_used_at")?,
    })
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct CredentialRepository {
    pub db: DynamoClient,
}

impl CredentialRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &CredentialId) -> Result<Credential, DbError> {
        let resp = self.db.inner
            .query()
            .table_name(&self.db.credentials_table)
            .index_name("credential-id-index")
            .key_condition_expression("credential_id = :cid")
            .expression_attribute_values(":cid", AttributeValue::S(id.0.clone()))
            .limit(1)
            .send()
            .await?;

        let item = resp.items
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or(DbError::NotFound)?;

        item_to_credential(item)
    }

    pub async fn list_for_user(&self, user_id: &UserId) -> Result<Vec<Credential>, DbError> {
        let resp = self.db.inner
            .query()
            .table_name(&self.db.credentials_table)
            .key_condition_expression("pk = :pk AND begins_with(sk, :prefix)")
            .expression_attribute_values(":pk", pk(user_id))
            .expression_attribute_values(":prefix", AttributeValue::S("CRED#".into()))
            .send()
            .await?;

        resp.items
            .unwrap_or_default()
            .into_iter()
            .map(item_to_credential)
            .collect()
    }

    pub async fn put(&self, credential: &Credential) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.credentials_table)
            .set_item(Some(credential_to_item(credential)))
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    /// Conditionally update sign_count only if the new value is greater.
    /// Tolerates counter regression (logs warning rather than erroring) to
    /// accommodate eventual consistency across Global Table replicas.
    pub async fn update_sign_count(
        &self,
        user_id: &UserId,
        id: &CredentialId,
        new_count: u32,
    ) -> Result<(), DbError> {
        let result = self.db.inner
            .update_item()
            .table_name(&self.db.credentials_table)
            .key("pk", pk(user_id))
            .key("sk", sk(id))
            .update_expression("SET sign_count = :new, last_used_at = :now")
            .condition_expression("sign_count < :new")
            .expression_attribute_values(":new", AttributeValue::N(new_count.to_string()))
            .expression_attribute_values(":now", AttributeValue::S(now_utc_rfc3339()))
            .send()
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let mapped = map_update_error(e);
                if matches!(mapped, DbError::ConditionalCheckFailed) {
                    tracing::warn!(
                        credential_id = %id.0,
                        new_count,
                        "sign_count regression — tolerating (eventual consistency)"
                    );
                    Ok(())
                } else {
                    Err(mapped)
                }
            }
        }
    }

    pub async fn delete(&self, user_id: &UserId, id: &CredentialId) -> Result<(), DbError> {
        self.db.inner
            .delete_item()
            .table_name(&self.db.credentials_table)
            .key("pk", pk(user_id))
            .key("sk", sk(id))
            .send()
            .await?;
        Ok(())
    }
}
