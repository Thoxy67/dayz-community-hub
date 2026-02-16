use std::time::{SystemTime, UNIX_EPOCH};

/// Format a Unix timestamp (seconds) into a human-readable relative time string.
/// Example: "2 hours ago", "just now", "5 days ago"
pub fn format_relative_time(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff = now - timestamp;

    if diff < 0 {
        return "in the future".to_string();
    }

    match diff {
        0..=59 => "just now".to_string(),
        60..=119 => "1 minute ago".to_string(),
        120..=3599 => format!("{} minutes ago", diff / 60),
        3600..=7199 => "1 hour ago".to_string(),
        7200..=86399 => format!("{} hours ago", diff / 3600),
        86400..=172799 => "1 day ago".to_string(),
        172800..=604799 => format!("{} days ago", diff / 86400),
        604800..=1209599 => "1 week ago".to_string(),
        1209600..=2591999 => format!("{} weeks ago", diff / 604800),
        _ => "a long time ago".to_string(),
    }
}

/// Format a Unix timestamp into a simple date string (YYYY-MM-DD HH:MM).
/// Uses local time if chrono is available, otherwise UTC.
#[cfg(feature = "chrono")]
pub fn format_absolute_time(timestamp: i64) -> String {
    use chrono::{DateTime, NaiveDateTime, Utc};
    let naive = NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap_or_default();
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Fallback without chrono: use UTC time via std.
#[cfg(not(feature = "chrono"))]
pub fn format_absolute_time(timestamp: i64) -> String {
    // Simple UTC formatting using std
    let secs = timestamp as u64;
    let days = secs / 86400;
    let years = days / 365;
    let rem_days = days % 365;
    // Not accurate, just a fallback
    if years > 0 {
        format!("{} years {} days ago", years, rem_days)
    } else if days > 0 {
        format!("{} days ago", days)
    } else {
        format_relative_time(timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_relative_time() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        assert_eq!(format_relative_time(now), "just now");
        assert_eq!(format_relative_time(now - 30), "just now");
        assert_eq!(format_relative_time(now - 90), "1 minute ago");
        assert_eq!(format_relative_time(now - 150), "2 minutes ago");
        assert_eq!(format_relative_time(now - 4000), "1 hour ago");
        assert_eq!(format_relative_time(now - 100000), "1 day ago");
    }
}
