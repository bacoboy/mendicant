//! Formatting + DDB row-mapping helpers shared by the admin handler modules.

use aws_sdk_dynamodb::types::AttributeValue;
use base64::Engine as _;
use std::collections::HashMap;
use time::OffsetDateTime;

use crate::error::AppError;

pub(super) type DdbItem = HashMap<String, AttributeValue>;

pub(super) fn val_s(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_s().ok())
        .map(|s| s.as_str())
        .unwrap_or("—")
        .to_string()
}

pub(super) fn val_n(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .map(|s| s.as_str())
        .unwrap_or("—")
        .to_string()
}

pub(super) fn val_bool(item: &DdbItem, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_bool().ok())
        .map(|b| if *b { "Yes" } else { "No" })
        .unwrap_or("—")
        .to_string()
}

pub(super) fn fmt_unix(n_str: &str) -> String {
    n_str.parse::<i64>()
        .ok()
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
        .map(|dt| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} UTC",
                dt.year(), dt.month() as u8, dt.day(),
                dt.hour(), dt.minute()
            )
        })
        .unwrap_or_else(|| n_str.to_string())
}

pub(super) fn fmt_dt_short(dt: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        dt.year(), dt.month() as u8, dt.day(), dt.hour(), dt.minute()
    )
}

pub(super) fn trunc(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

pub(super) fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn aaguid_display(aaguid: &str) -> String {
    match aaguid {
        "2fc0579f-8113-47ea-b116-bb5a8db9202a" => "YubiKey 5 Series".into(),
        "fa2b99dc-9e39-4257-8f92-4a30d23c4118" => "YubiKey 5 NFC".into(),
        "73bb0cd4-e502-49b8-9c6f-b59445bf720b" => "YubiKey 5C NFC".into(),
        "c1f9a0bc-1dd2-404a-b27f-8e29047a43fd" => "YubiKey 5Ci".into(),
        "cb69481e-8ff7-4039-93ec-0a2729a154a8" => "YubiKey 5 Nano".into(),
        "0bb43545-fd2c-4185-87dd-feb0b2916ace" => "YubiKey 5C Nano (fw <5.7)".into(),
        "ff4dac45-ede8-4ec2-aced-cf66103f4335" => "YubiKey 5C Nano (fw 5.7+)".into(),
        "b92c3f9a-c014-4056-887f-140a2501163b" => "YubiKey 5C".into(),
        "6d44ba9b-f6ec-2e49-b930-0c8fe920cb73" => "Security Key NFC".into(),
        "f8a011f3-8c0a-4d15-8006-17111f9edc7d" => "Security Key".into(),
        "ee882879-721c-4913-9775-3dfcce97072a" => "YubiKey 5.4 Series".into(),
        "d8522d9f-575b-4866-88a9-ba99fa02f35b" => "YubiKey Bio".into(),
        "00000000-0000-0000-0000-000000000000" => "Security Key".into(),
        other => trunc(other, 18),
    }
}

pub(super) fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub(super) fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

pub(super) fn format_role(role: &domain::user::Role) -> String {
    match role {
        domain::user::Role::Free => "free".into(),
        domain::user::Role::Member => "member".into(),
        domain::user::Role::Administrator => "administrator".into(),
    }
}

pub(super) fn format_status(status: &domain::user::UserStatus) -> String {
    match status {
        domain::user::UserStatus::Active => "active".into(),
        domain::user::UserStatus::Suspended => "suspended".into(),
        domain::user::UserStatus::PendingVerification => "pending_verification".into(),
    }
}

pub(super) fn parse_role_filter(s: &str) -> Result<domain::user::Role, AppError> {
    match s {
        "free" => Ok(domain::user::Role::Free),
        "member" => Ok(domain::user::Role::Member),
        "administrator" => Ok(domain::user::Role::Administrator),
        other => Err(AppError::BadRequest(format!("unknown role: {other}"))),
    }
}

pub(super) fn parse_status_filter(s: &str) -> Result<domain::user::UserStatus, AppError> {
    match s {
        "active" => Ok(domain::user::UserStatus::Active),
        "suspended" => Ok(domain::user::UserStatus::Suspended),
        "pending_verification" => Ok(domain::user::UserStatus::PendingVerification),
        other => Err(AppError::BadRequest(format!("unknown status: {other}"))),
    }
}

pub(super) fn encode_browse_cursor(key: &DdbItem) -> Result<String, anyhow::Error> {
    let simple: HashMap<String, String> = key.iter()
        .filter_map(|(k, v)| v.as_s().ok().map(|s| (k.clone(), s.clone())))
        .collect();
    let json = serde_json::to_string(&simple)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
}

pub(super) fn decode_browse_cursor(cursor: &str) -> Result<DdbItem, AppError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|e| AppError::BadRequest(format!("invalid cursor: {e}")))?;
    let simple: HashMap<String, String> = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("invalid cursor: {e}")))?;
    Ok(simple.into_iter().map(|(k, v)| (k, AttributeValue::S(v))).collect())
}
