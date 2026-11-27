//! Request times are parsed here, not by serde, so a time without a timezone
//! offset can get `422` while a value that is not a time gets `400` (ADR-010).

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::core::error::AppError;
use crate::model::dtos::RequestTime;

/// Converts a request time to UTC.
///
/// - RFC 3339 with an offset (`2099-12-01T10:00:00-06:00`, `...Z`): accepted.
/// - A date and time without an offset (`2099-12-01T10:00:00`): `422`.
/// - Anything else: `400`.
///
/// `field` names the request field in the error message.
pub fn parse_request_time(value: &RequestTime, field: &str) -> Result<DateTime<Utc>, AppError> {
    let raw = value.0.trim();

    if let Ok(time) = DateTime::parse_from_rfc3339(raw) {
        return Ok(time.with_timezone(&Utc));
    }

    let without_offset = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f").is_ok();
    if without_offset {
        return Err(AppError::UnprocessableEntity(format!(
            "{field} must include a timezone offset, such as 2026-12-01T10:00:00-06:00 or 2026-12-01T16:00:00Z"
        )));
    }

    Err(AppError::BadRequest(format!(
        "{field} is not a valid RFC 3339 time"
    )))
}
