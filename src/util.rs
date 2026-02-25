/// Date/time utilities for normalizing courier-provided timestamps.
///
/// All dates stored in the database MUST be in one of two formats:
///   - **Timestamps**: RFC 3339 UTC — `2026-02-25T11:26:00Z`
///   - **Date-only**:  ISO 8601 date — `2026-03-02`
///
/// Courier APIs return dates in varied formats. Use the helpers here to
/// normalize them before returning a `CourierStatus`. If a courier provides
/// date components (year, month, day, hour, minute, second), use
/// [`format_rfc3339_utc`]. If the API returns a compact `YYYYMMDD` string,
/// use [`parse_date_yyyymmdd`].
///
/// The frontend parses these via `new Date()` and formats them with
/// `Intl.DateTimeFormat` in the browser's local timezone.

/// Parse a compact `YYYYMMDD` date string into ISO 8601 date format (`YYYY-MM-DD`).
///
/// Returns `None` if the input is not exactly 8 characters.
pub fn parse_date_yyyymmdd(s: &str) -> Option<String> {
    if s.len() != 8 {
        return None;
    }
    Some(format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8]))
}

/// Format date/time components as an RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn format_rfc3339_utc(year: u32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yyyymmdd_valid() {
        assert_eq!(parse_date_yyyymmdd("20260302"), Some("2026-03-02".into()));
    }

    #[test]
    fn parse_yyyymmdd_too_short() {
        assert_eq!(parse_date_yyyymmdd("202603"), None);
    }

    #[test]
    fn parse_yyyymmdd_already_formatted() {
        assert_eq!(parse_date_yyyymmdd("2026-03-02"), None);
    }

    #[test]
    fn format_rfc3339() {
        assert_eq!(
            format_rfc3339_utc(2026, 2, 25, 11, 26, 0),
            "2026-02-25T11:26:00Z"
        );
    }

    #[test]
    fn format_rfc3339_pads_single_digits() {
        assert_eq!(
            format_rfc3339_utc(2026, 1, 5, 3, 4, 9),
            "2026-01-05T03:04:09Z"
        );
    }
}
