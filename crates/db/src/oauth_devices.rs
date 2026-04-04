use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

use domain::oauth::{DeviceGrant, DeviceGrantStatus};

use crate::attr::{Item, get_n_i64, get_s, get_s_opt};
use crate::client::DynamoClient;
use crate::error::{DbError, map_put_error, map_update_error};

// ── Key helpers ───────────────────────────────────────────────────────────────

fn pk(device_code: &str) -> AttributeValue {
    AttributeValue::S(format!("DEVICE#{device_code}"))
}

// ── Item ↔ Domain conversions ─────────────────────────────────────────────────

fn grant_to_item(g: &DeviceGrant) -> Item {
    let mut m = HashMap::new();
    m.insert("pk".into(), pk(&g.device_code));
    m.insert("device_code".into(), AttributeValue::S(g.device_code.clone()));
    m.insert("user_code".into(), AttributeValue::S(g.user_code.clone()));
    m.insert("status".into(), AttributeValue::S(status_to_str(&g.status).into()));
    m.insert("expires_at".into(), AttributeValue::N(g.expires_at.to_string()));
    if let Some(ref uid) = g.user_id {
        m.insert("user_id".into(), AttributeValue::S(uid.clone()));
    }
    m
}

fn item_to_grant(item: Item) -> Result<DeviceGrant, DbError> {
    Ok(DeviceGrant {
        device_code: get_s(&item, "device_code")?,
        user_code: get_s(&item, "user_code")?,
        status: str_to_status(&get_s(&item, "status")?)?,
        expires_at: get_n_i64(&item, "expires_at")?,
        user_id: get_s_opt(&item, "user_id")?,
    })
}

fn status_to_str(s: &DeviceGrantStatus) -> &'static str {
    match s {
        DeviceGrantStatus::Pending => "pending",
        DeviceGrantStatus::Approved => "approved",
        DeviceGrantStatus::Denied => "denied",
    }
}

fn str_to_status(s: &str) -> Result<DeviceGrantStatus, DbError> {
    match s {
        "pending" => Ok(DeviceGrantStatus::Pending),
        "approved" => Ok(DeviceGrantStatus::Approved),
        "denied" => Ok(DeviceGrantStatus::Denied),
        other => Err(DbError::Serde(format!("unknown device grant status: {other}"))),
    }
}

// ── Repository ────────────────────────────────────────────────────────────────

pub struct OAuthDeviceRepository {
    pub db: DynamoClient,
}

impl OAuthDeviceRepository {
    pub fn new(db: DynamoClient) -> Self {
        Self { db }
    }

    pub async fn get_by_device_code(&self, device_code: &str) -> Result<DeviceGrant, DbError> {
        let resp = self.db.inner
            .get_item()
            .table_name(&self.db.oauth_devices_table)
            .key("pk", pk(device_code))
            .send()
            .await?;

        item_to_grant(resp.item.ok_or(DbError::NotFound)?)
    }

    pub async fn get_by_user_code(&self, user_code: &str) -> Result<DeviceGrant, DbError> {
        let resp = self.db.inner
            .query()
            .table_name(&self.db.oauth_devices_table)
            .index_name("user-code-index")
            .key_condition_expression("user_code = :code")
            .expression_attribute_values(":code", AttributeValue::S(user_code.into()))
            .limit(1)
            .send()
            .await?;

        let item = resp.items
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or(DbError::NotFound)?;

        item_to_grant(item)
    }

    pub async fn put(&self, grant: &DeviceGrant) -> Result<(), DbError> {
        self.db.inner
            .put_item()
            .table_name(&self.db.oauth_devices_table)
            .set_item(Some(grant_to_item(grant)))
            .condition_expression("attribute_not_exists(pk)")
            .send()
            .await
            .map_err(map_put_error)?;
        Ok(())
    }

    pub async fn approve(&self, device_code: &str, user_id: &str) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.oauth_devices_table)
            .key("pk", pk(device_code))
            .update_expression("SET #status = :approved, user_id = :uid")
            .expression_attribute_names("#status", "status")
            .expression_attribute_values(":approved", AttributeValue::S("approved".into()))
            .expression_attribute_values(":uid", AttributeValue::S(user_id.into()))
            .condition_expression("#status = :pending")
            .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }

    pub async fn deny(&self, device_code: &str) -> Result<(), DbError> {
        self.db.inner
            .update_item()
            .table_name(&self.db.oauth_devices_table)
            .key("pk", pk(device_code))
            .update_expression("SET #status = :denied")
            .expression_attribute_names("#status", "status")
            .expression_attribute_values(":denied", AttributeValue::S("denied".into()))
            .condition_expression("#status = :pending")
            .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
            .send()
            .await
            .map_err(map_update_error)?;
        Ok(())
    }
}
