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
///
/// Out-of-range timestamps are logged at `warn` level before falling back to
/// the Unix epoch. Callers that need to distinguish invalid input should use
/// [`DateTime::<Utc>::from_timestamp_millis`] directly.
///
/// # Panics
///
/// Panics if the Unix epoch (timestamp 0) cannot be represented as a
/// `DateTime<Utc>`, which should never happen in practice.
#[must_use]
pub fn timestamp_to_rfc3339(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ts) {
        dt.to_rfc3339()
    } else {
        tracing::warn!(
            ts,
            "timestamp_to_rfc3339: out-of-range timestamp; falling back to Unix epoch"
        );
        DateTime::<Utc>::from_timestamp_millis(0)
            .expect("unix epoch should be valid")
            .to_rfc3339()
    }
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
