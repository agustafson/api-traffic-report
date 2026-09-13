//! Parses and validates one JSON Lines record into a `RequestRecord`.

use serde_json::Value;
use std::ops::RangeInclusive;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::BUCKET_SECONDS;
use super::ids::{ClientId, Endpoint, StatusCode};

const HTTP_STATUS_CODES: RangeInclusive<u64> = 100..=599;

/// A single validated log line, with its timestamp reduced to a UTC rate-limit bucket.
pub(super) struct RequestRecord {
    pub(super) client_id: ClientId,
    pub(super) endpoint: Endpoint,
    pub(super) status_code: StatusCode,
    pub(super) bucket: i64,
}

impl RequestRecord {
    pub(super) fn parse(line: &[u8]) -> Option<Self> {
        let value: Value = serde_json::from_slice(line).ok()?;
        let object = value.as_object()?;
        let request_id = object.get("request_id")?.as_str()?;
        let timestamp = object.get("timestamp")?.as_str()?;
        let client_id = object.get("client_id")?.as_str()?;
        let endpoint = object.get("endpoint")?.as_str()?;
        let status_code = object.get("status_code")?.as_u64()?;

        if request_id.is_empty()
            || timestamp.is_empty()
            || client_id.is_empty()
            || endpoint.is_empty()
            || !HTTP_STATUS_CODES.contains(&status_code)
        {
            return None;
        }

        let timestamp = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
        Some(Self {
            client_id: ClientId::new(client_id),
            endpoint: Endpoint::new(endpoint),
            status_code: StatusCode::new(status_code.try_into().ok()?),
            bucket: timestamp.unix_timestamp().div_euclid(BUCKET_SECONDS),
        })
    }
}

pub(super) fn is_blank(line: &[u8]) -> bool {
    std::str::from_utf8(line).is_ok_and(|text| text.trim().is_empty())
}
