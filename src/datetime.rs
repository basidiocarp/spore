//! Shared datetime helpers for the ecosystem.
//!
//! These utilities keep timestamp formatting and parsing consistent across
//! repos that persist or exchange RFC3339 timestamps.
//!
//! # Examples
//!
//! ```rust
//! use spore::datetime::{now_utc, rfc3339_to_timestamp, timestamp_to_rfc3339};
//!
//! let now = now_utc();
//! let encoded = timestamp_to_rfc3339(now.timestamp_millis());
//! assert_eq!(rfc3339_to_timestamp(&encoded).unwrap(), now.timestamp_millis());
//! ```

use crate::Result;
use chrono::NaiveDateTime;

pub use chrono::{DateTime, Duration, NaiveDate, Utc};

/// Return the current UTC datetime.
#[must_use]
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

/// Convert an epoch-millisecond timestamp to an RFC3339 string.
#[must_use]
pub fn timestamp_to_rfc3339(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ts)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| {
            DateTime::<Utc>::from_timestamp_millis(0)
                .expect("unix epoch should be valid")
                .to_rfc3339()
        })
}

/// Parse an RFC3339 timestamp and return epoch milliseconds.
pub fn rfc3339_to_timestamp(s: &str) -> Result<i64> {
    parse_rfc3339_utc(s).map(|dt| dt.timestamp_millis())
}

/// Parse an RFC3339 timestamp into a UTC datetime.
pub fn parse_rfc3339_utc(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|error| {
            crate::SporeError::Other(format!("invalid RFC3339 timestamp '{s}': {error}"))
        })
}

/// Parse a SQLite-style UTC timestamp (`YYYY-MM-DD HH:MM:SS`).
pub fn parse_sqlite_utc(s: &str) -> Result<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc())
        .map_err(|error| {
            crate::SporeError::Other(format!("invalid SQLite timestamp '{s}': {error}"))
        })
}
