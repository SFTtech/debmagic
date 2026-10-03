use std::time::{Duration, SystemTime};

use anyhow::anyhow;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

/// When something cached is refreshed again: before every use, only the first
/// time, or once its last refresh is older than a maximum age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum RefreshPolicy {
    /// Refresh before every use.
    Now,
    /// Only refresh the first time.
    Never,
    /// Refresh again once the last one is older than this.
    OlderThan(Duration),
}

impl Default for RefreshPolicy {
    fn default() -> Self {
        Self::OlderThan(Duration::from_secs(24 * 60 * 60))
    }
}

impl RefreshPolicy {
    fn parse(value: &str) -> anyhow::Result<Self> {
        if value == "now" {
            return Ok(Self::Now);
        }
        if value == "never" {
            return Ok(Self::Never);
        }

        let invalid = || {
            anyhow!(
                "invalid refresh policy '{value}': expected 'now', 'never' or a duration like '1d', '12h', '30m'"
            )
        };
        let Some((digits, unit)) = value.split_at_checked(value.len().saturating_sub(1)) else {
            return Err(invalid());
        };
        let amount: u64 = digits.parse().map_err(|_| invalid())?;
        let seconds = match unit {
            "s" => Some(amount),
            "m" => amount.checked_mul(60),
            "h" => amount.checked_mul(60 * 60),
            "d" => amount.checked_mul(24 * 60 * 60),
            _ => return Err(invalid()),
        }
        .ok_or_else(|| anyhow!("refresh policy '{value}' is too large"))?;
        Ok(Self::OlderThan(Duration::from_secs(seconds)))
    }
}

impl std::fmt::Display for RefreshPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Now => f.write_str("now"),
            Self::Never => f.write_str("never"),
            Self::OlderThan(age) => {
                let seconds = age.as_secs();
                let (amount, unit) = if seconds.is_multiple_of(24 * 60 * 60) {
                    (seconds / (24 * 60 * 60), "d")
                } else if seconds.is_multiple_of(60 * 60) {
                    (seconds / (60 * 60), "h")
                } else if seconds.is_multiple_of(60) {
                    (seconds / 60, "m")
                } else {
                    (seconds, "s")
                };
                write!(f, "{amount}{unit}")
            }
        }
    }
}

impl From<RefreshPolicy> for String {
    fn from(policy: RefreshPolicy) -> Self {
        policy.to_string()
    }
}

impl TryFrom<String> for RefreshPolicy {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        Self::parse(&value)
    }
}

impl std::str::FromStr for RefreshPolicy {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> anyhow::Result<Self> {
        Self::parse(value)
    }
}

/// UTC timestamp in RFC 3339 with second precision
pub fn iso_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Parse a timestamp written by [`iso_timestamp`]
pub fn parse_iso_timestamp(value: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|time| time.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_now_and_never() {
        assert_eq!(RefreshPolicy::parse("now").unwrap(), RefreshPolicy::Now);
        assert_eq!(RefreshPolicy::parse("never").unwrap(), RefreshPolicy::Never);
    }

    #[test]
    fn parses_durations() {
        let age = |seconds| RefreshPolicy::OlderThan(Duration::from_secs(seconds));
        assert_eq!(RefreshPolicy::parse("45s").unwrap(), age(45));
        assert_eq!(RefreshPolicy::parse("30m").unwrap(), age(30 * 60));
        assert_eq!(RefreshPolicy::parse("12h").unwrap(), age(12 * 60 * 60));
        assert_eq!(RefreshPolicy::parse("1d").unwrap(), age(24 * 60 * 60));
    }

    #[test]
    fn rejects_invalid_values() {
        for value in ["", "1", "d", "1x", "now!", "1.5d"] {
            assert!(
                RefreshPolicy::parse(value).is_err(),
                "{value} should not parse"
            );
        }
    }

    #[test]
    fn defaults_to_one_day() {
        assert_eq!(
            RefreshPolicy::default(),
            RefreshPolicy::OlderThan(Duration::from_secs(24 * 60 * 60))
        );
    }

    #[test]
    fn displays_roundtrippably() {
        assert_eq!(RefreshPolicy::Now.to_string(), "now");
        assert_eq!(RefreshPolicy::Never.to_string(), "never");
        assert_eq!(
            RefreshPolicy::OlderThan(Duration::from_secs(24 * 60 * 60)).to_string(),
            "1d"
        );
        assert_eq!(
            RefreshPolicy::OlderThan(Duration::from_secs(90 * 60)).to_string(),
            "90m"
        );
        assert_eq!(
            RefreshPolicy::OlderThan(Duration::from_secs(37)).to_string(),
            "37s"
        );
    }

    #[test]
    fn iso_timestamps_roundtrip() {
        let parsed = parse_iso_timestamp(&iso_timestamp()).unwrap();
        let age = SystemTime::now().duration_since(parsed).unwrap();
        assert!(age < Duration::from_secs(120));
    }

    #[test]
    fn iso_timestamp_parsing_rejects_garbage() {
        assert_eq!(parse_iso_timestamp("not a timestamp"), None);
        assert_eq!(parse_iso_timestamp(""), None);
    }
}
