/// Returns the current time as a UTC RFC 3339 string (always ending in `Z`).
///
/// All timestamps stored in DynamoDB go through this function so there is
/// never any local-timezone offset in persisted data.
pub fn now_utc_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    // OffsetDateTime::now_utc() always produces a UTC instant (offset = 0).
    // Formatting with Rfc3339 produces e.g. "2024-01-15T10:30:00Z".
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC OffsetDateTime always formats successfully")
}

/// Parse an RFC 3339 string and ensure it represents a UTC instant.
/// Returns an error string if parsing fails or the offset is non-zero.
pub fn parse_utc_rfc3339(s: &str) -> Result<time::OffsetDateTime, String> {
    use time::format_description::well_known::Rfc3339;
    let dt = time::OffsetDateTime::parse(s, &Rfc3339)
        .map_err(|e| format!("invalid RFC 3339 timestamp '{s}': {e}"))?;
    // Convert to UTC regardless of what offset was stored (defensive).
    Ok(dt.to_offset(time::UtcOffset::UTC))
}
