//! Calendar checks for `YYYY-MM-DD` strings.

/// Why a date string was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DateError {
    /// The text is not `YYYY-MM-DD`.
    Format,
    /// The text is shaped like a date, but that day does not exist.
    Calendar,
}

/// Checks that `text` is a real Gregorian date in `YYYY-MM-DD` form.
pub(crate) fn validate_date(text: &str) -> Result<(), DateError> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(DateError::Format);
    }
    let year = parse_digits(&bytes[0..4])?;
    let month = parse_digits(&bytes[5..7])?;
    let day = parse_digits(&bytes[8..10])?;
    let Some(max_day) = days_in_month(year, month) else {
        return Err(DateError::Calendar);
    };
    if day == 0 || day > max_day {
        return Err(DateError::Calendar);
    }
    Ok(())
}

fn parse_digits(bytes: &[u8]) -> Result<u32, DateError> {
    let mut value = 0_u32;
    for byte in bytes {
        let Some(digit) = byte.checked_sub(b'0') else {
            return Err(DateError::Format);
        };
        if digit > 9 {
            return Err(DateError::Format);
        }
        value = value * 10 + u32::from(digit);
    }
    Ok(value)
}

fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    };
    Some(days)
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// POSIX seconds for the current UTC time.
///
/// Returns 0 when the clock is before 1970-01-01 UTC.
#[must_use]
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Formats a POSIX timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Returns [`None`] when `seconds` is negative or the year does not fit in
/// four digits.
#[must_use]
pub fn format_iso8601(seconds: i64) -> Option<String> {
    let seconds = u64::try_from(seconds).ok()?;
    let day = format_unix_days(seconds / 86_400)?;
    let clock = seconds % 86_400;
    let hour = clock / 3600;
    let minute = (clock % 3600) / 60;
    let second = clock % 60;
    Some(format!("{day}T{hour:02}:{minute:02}:{second:02}Z"))
}

/// UTC calendar day of a POSIX timestamp, as `YYYY-MM-DD`.
///
/// Returns [`None`] when `seconds` is negative or the year does not fit in
/// four digits.
#[must_use]
pub fn utc_day(seconds: i64) -> Option<String> {
    let seconds = u64::try_from(seconds).ok()?;
    format_unix_days(seconds / 86_400)
}

/// Whether `text` is a real `YYYY-MM-DD` calendar day.
#[must_use]
pub fn is_calendar_date(text: &str) -> bool {
    validate_date(text).is_ok()
}

/// Formats a POSIX timestamp as `YYYY-MM-DD HH:MM:SS UTC`.
///
/// Returns [`None`] when `seconds` is negative or the year does not fit in
/// four digits.
#[must_use]
pub fn format_unix_timestamp(seconds: i64) -> Option<String> {
    let seconds = u64::try_from(seconds).ok()?;
    let day = format_unix_days(seconds / 86_400)?;
    let clock = seconds % 86_400;
    let hour = clock / 3600;
    let minute = (clock % 3600) / 60;
    let second = clock % 60;
    Some(format!("{day} {hour:02}:{minute:02}:{second:02} UTC"))
}

/// Formats the UTC calendar day of `time` as `YYYY-MM-DD`.
///
/// `SystemTime::duration_since(UNIX_EPOCH)` is the POSIX second count and does
/// not include leap seconds. Dividing by 86400 is the UTC date. Returns
/// [`None`] when `time` is before 1970-01-01 UTC or the year does not fit in
/// four digits.
#[must_use]
pub fn utc_date(time: std::time::SystemTime) -> Option<String> {
    let seconds = time
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    format_unix_days(seconds / 86_400)
}

fn format_unix_days(mut remaining: u64) -> Option<String> {
    let mut year = 1970_u32;
    loop {
        let length: u64 = if is_leap_year(year) { 366 } else { 365 };
        if remaining < length {
            break;
        }
        remaining -= length;
        year = year.checked_add(1)?;
        if year > 9999 {
            return None;
        }
    }
    let mut month = 1_u32;
    loop {
        let length = days_in_month(year, month)?;
        if remaining < u64::from(length) {
            let day = u32::try_from(remaining).ok()? + 1;
            return Some(format!("{year:04}-{month:02}-{day:02}"));
        }
        remaining -= u64::from(length);
        month += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{DateError, utc_date, validate_date};

    #[test]
    fn accepts_leap_day_in_a_leap_year() {
        assert_eq!(validate_date("2024-02-29"), Ok(()));
        assert_eq!(validate_date("2000-02-29"), Ok(()));
    }

    #[test]
    fn rejects_leap_day_in_a_common_year() {
        assert_eq!(validate_date("2023-02-29"), Err(DateError::Calendar));
        assert_eq!(validate_date("1900-02-29"), Err(DateError::Calendar));
    }

    #[test]
    fn rejects_impossible_month_lengths() {
        assert_eq!(validate_date("2026-02-30"), Err(DateError::Calendar));
        assert_eq!(validate_date("2026-04-31"), Err(DateError::Calendar));
        assert_eq!(validate_date("2026-11-31"), Err(DateError::Calendar));
        assert_eq!(validate_date("2026-00-10"), Err(DateError::Calendar));
        assert_eq!(validate_date("2026-13-01"), Err(DateError::Calendar));
        assert_eq!(validate_date("2026-01-00"), Err(DateError::Calendar));
    }

    #[test]
    fn unix_epoch_is_the_first_utc_day() {
        assert_eq!(
            utc_date(SystemTime::UNIX_EPOCH).as_deref(),
            Some("1970-01-01")
        );
        let next = SystemTime::UNIX_EPOCH + Duration::from_hours(24);
        assert_eq!(utc_date(next).as_deref(), Some("1970-01-02"));
        // 1970 and 1971 are common years: day 730 is 1972-01-01, day 789 is 1972-02-29.
        let leap = SystemTime::UNIX_EPOCH + Duration::from_hours(789 * 24);
        assert_eq!(utc_date(leap).as_deref(), Some("1972-02-29"));
    }

    #[test]
    fn rejects_wrong_shapes() {
        assert_eq!(validate_date("2026-2-30"), Err(DateError::Format));
        assert_eq!(validate_date("2026/01/15"), Err(DateError::Format));
        assert_eq!(validate_date("15-01-2026"), Err(DateError::Format));
    }
}
