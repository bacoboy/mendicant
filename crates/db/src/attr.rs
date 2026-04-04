/// Convenience helpers for extracting typed values from DynamoDB AttributeValue maps.
///
/// DynamoDB's SDK returns `HashMap<String, AttributeValue>` — these functions
/// reduce the boilerplate of matching each variant and converting to Rust types.
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

use crate::error::DbError;

pub type Item = HashMap<String, AttributeValue>;

// ── Extractors ────────────────────────────────────────────────────────────────

pub fn get_s(item: &Item, key: &str) -> Result<String, DbError> {
    item.get(key)
        .and_then(|v| v.as_s().ok())
        .map(|s| s.clone())
        .ok_or_else(|| DbError::Serde(format!("missing or non-string field: {key}")))
}

pub fn get_s_opt(item: &Item, key: &str) -> Result<Option<String>, DbError> {
    match item.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_s()
            .map(|s| Some(s.clone()))
            .map_err(|_| DbError::Serde(format!("non-string field: {key}"))),
    }
}

pub fn get_n_u32(item: &Item, key: &str) -> Result<u32, DbError> {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or_else(|| DbError::Serde(format!("missing or non-numeric field: {key}")))
}

pub fn get_n_i64(item: &Item, key: &str) -> Result<i64, DbError> {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse::<i64>().ok())
        .ok_or_else(|| DbError::Serde(format!("missing or non-numeric field: {key}")))
}

pub fn get_bool(item: &Item, key: &str) -> Result<bool, DbError> {
    item.get(key)
        .and_then(|v| v.as_bool().ok())
        .copied()
        .ok_or_else(|| DbError::Serde(format!("missing or non-bool field: {key}")))
}

// ── UTC timestamp helpers ─────────────────────────────────────────────────────

/// Extract an RFC 3339 timestamp field and parse it as UTC.
pub fn get_utc(item: &Item, key: &str) -> Result<time::OffsetDateTime, DbError> {
    let s = get_s(item, key)?;
    crate::time_util::parse_utc_rfc3339(&s)
        .map_err(|e| DbError::Serde(e))
}
