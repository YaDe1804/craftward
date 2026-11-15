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

#[cfg(test)]
mod tests {
    use super::{DateError, validate_date};

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
    fn rejects_wrong_shapes() {
        assert_eq!(validate_date("2026-2-30"), Err(DateError::Format));
        assert_eq!(validate_date("2026/01/15"), Err(DateError::Format));
        assert_eq!(validate_date("15-01-2026"), Err(DateError::Format));
    }
}
