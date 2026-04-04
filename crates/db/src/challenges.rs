use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

use domain::challenge::{Challenge, ChallengeType};

use crate::attr::{Item, get_n_i64, get_s, get_s_opt};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error};

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(id: &str) -> AttributeValue {
    AttributeValue::S(format!("CHALLENGE#{id}"))
}

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn challenge_to_item(c: &Challenge) -> Item {
    let mut m = HashMap::new();
    m.insert("pk".into(), pk(&c.id));
    m.insert(
        "challenge_type".into(),
        AttributeValue::S(match c.challenge_type {
            ChallengeType::Registration => "registration".into(),
            ChallengeType::Authentication => "authentication".into(),
        }),
    );
    m.insert("state_json".into(), AttributeValue::S(c.state_json.clone()));
    m.insert("expires_at".into(), AttributeValue::N(c.expires_at.to_string()));
    if let Some(ref uid) = c.user_id {
        m.insert("user_id".into(), AttributeValue::S(uid.clone()));
    }
    m
}

fn item_to_challenge(item: Item) -> Result<Challenge, DbError> {
    let pk_val = get_s(&item, "pk")?;
    let id = pk_val
        .strip_prefix("CHALLENGE#")
        .ok_or_else(|| DbError::Serde("malformed challenge pk".into()))?
        .to_string();

    let challenge_type = match get_s(&item, "challenge_type")?.as_str() {
        "registration" => ChallengeType::Registration,
        "authentication" => ChallengeType::Authentication,
        other => return Err(DbError::Serde(format!("unknown challenge_type: {other}"))),
    };

    Ok(Challenge {
        id,
        challenge_type,
        state_json: get_s(&item, "state_json")?,
        user_id: get_s_opt(&item, "user_id")?,
        expires_at: get_n_i64(&item, "expires_at")?,
    })
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct ChallengeRepository {
    pub db: DynamoClient,
}

impl ChallengeRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &str) -> Result<Challenge, DbError> {
        let resp = self.db.inner
            .get_item()
            .table_name(&self.db.challenges_table)
            .key("pk", pk(id))
            .send()
            .await?;

        item_to_challenge(resp.item.ok_or(DbError::NotFound)?)
    }

    pub async fn put(&self, challenge: &Challenge) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.challenges_table)
            .set_item(Some(challenge_to_item(challenge)))
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    /// Atomically read and delete a challenge (prevents replay attacks).
    /// Returns NotFound if the challenge has expired or never existed.
    pub async fn take(&self, id: &str) -> Result<Challenge, DbError> {
        let challenge = self.get(id).await?;
        self.db.inner
            .delete_item()
            .table_name(&self.db.challenges_table)
            .key("pk", pk(id))
            .send()
            .await?;
        Ok(challenge)
    }
}
