use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

use domain::email_token::EmailToken;

use crate::attr::{Item, get_s, get_n_i64};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error};

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(id: &str) -> AttributeValue {
    AttributeValue::S(format!("EMAIL_TOKEN#{id}"))
}

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn email_token_to_item(t: &EmailToken) -> Item {
    let mut m = HashMap::new();
    m.insert("pk".into(), pk(&t.id));
    m.insert("email".into(), AttributeValue::S(t.email.clone()));
    m.insert("expires_at".into(), AttributeValue::N(t.expires_at.to_string()));
    m
}

fn item_to_email_token(item: Item) -> Result<EmailToken, DbError> {
    let pk_val = get_s(&item, "pk")?;
    let id = pk_val
        .strip_prefix("EMAIL_TOKEN#")
        .ok_or_else(|| DbError::Serde("malformed email token pk".into()))?
        .to_string();

    Ok(EmailToken {
        id,
        email: get_s(&item, "email")?,
        expires_at: get_n_i64(&item, "expires_at")?,
    })
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct EmailTokenRepository {
    pub db: DynamoClient,
}

impl EmailTokenRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &str) -> Result<EmailToken, DbError> {
        let resp = self.db.inner
            .get_item()
            .table_name(&self.db.email_tokens_table)
            .key("pk", pk(id))
            .send()
            .await?;

        item_to_email_token(resp.item.ok_or(DbError::NotFound)?)
    }

    pub async fn put(&self, token: &EmailToken) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.email_tokens_table)
            .set_item(Some(email_token_to_item(token)))
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    /// Atomically read and delete a token (prevents replay attacks).
    /// Returns NotFound if the token has expired or never existed.
    pub async fn take(&self, id: &str) -> Result<EmailToken, DbError> {
        let token = self.get(id).await?;
        self.db.inner
            .delete_item()
            .table_name(&self.db.email_tokens_table)
            .key("pk", pk(id))
            .send()
            .await?;
        Ok(token)
    }
}
