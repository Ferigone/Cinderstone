//! The `yyyy-MM-dd HH:mm:ss Z` instants of the progress file, as the original's
//! `DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss Z", Locale.ROOT)` reads and writes them.
//!
//! Reading is strict about the shape (`SignStyle.EXCEEDS_PAD` for the year, fixed two-digit
//! fields, `+HHMM` offsets) and resolves with the `SMART` style: a day past the end of the month
//! is clamped, `24:00:00` is the next midnight. Writing prints the instant in UTC (`+0000`), which
//! is what the original prints on a server whose time zone is UTC.

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Exactly two ASCII digits at `at`.
fn two_digits(bytes: &[u8], at: usize) -> Option<i64> {
    let pair = bytes.get(at..at + 2)?;
    pair.iter()
        .all(u8::is_ascii_digit)
        .then(|| i64::from(pair[0] - b'0') * 10 + i64::from(pair[1] - b'0'))
}

/// `LocalDate.MAX.toEpochDay()` and `LocalDate.MIN.toEpochDay()`.
const MAX_EPOCH_DAY: i64 = 365_241_780_471;
const MIN_EPOCH_DAY: i64 = -365_243_219_162;

/// Largest `YEAR_OF_ERA` the resolver accepts.
const MAX_YEAR_OF_ERA: i64 = 999_999_999;

/// Epoch seconds of `yyyy-MM-dd HH:mm:ss ±HHMM`, or `None` where `DateTimeFormatter` rejects it.
pub fn parse_obtained(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let mut at = 0;
    // `yyyy`: `appendValue(YEAR_OF_ERA, 4, 19, EXCEEDS_PAD)`. A sign is only legal (and then
    // required) when there are more than four digits; negative years fail the range check.
    let signed = match bytes.first() {
        Some(b'+') => {
            at = 1;
            true
        }
        Some(b'-') => return None,
        _ => false,
    };
    let digits_start = at;
    while at < bytes.len() && at - digits_start < 19 && bytes[at].is_ascii_digit() {
        at += 1;
    }
    let digit_count = at - digits_start;
    if digit_count < 4 || (signed && digit_count <= 4) || (!signed && digit_count > 4) {
        return None;
    }
    let mut year: i64 = 0;
    for &digit in &bytes[digits_start..at] {
        year = year.checked_mul(10)?.checked_add(i64::from(digit - b'0'))?;
    }
    if !(1..=MAX_YEAR_OF_ERA).contains(&year) {
        return None;
    }
    let rest = &bytes[at..];
    // `-MM-dd HH:mm:ss ±HHMM` is a fixed 21 bytes.
    if rest.len() != 21 {
        return None;
    }
    let literals = [
        (0, b'-'),
        (3, b'-'),
        (6, b' '),
        (9, b':'),
        (12, b':'),
        (15, b' '),
    ];
    if literals
        .iter()
        .any(|&(index, literal)| rest[index] != literal)
    {
        return None;
    }
    let month = two_digits(rest, 1)?;
    let day = two_digits(rest, 4)?;
    let hour = two_digits(rest, 7)?;
    let minute = two_digits(rest, 10)?;
    let second = two_digits(rest, 13)?;
    // `Z` is `appendOffset("+HHMM", "+0000")`: the zero offset text is tried first, then a sign
    // and exactly two plus two digits (hour at most 23, minute at most 59).
    let offset_seconds = if &rest[16..21] == b"+0000" {
        0
    } else {
        let sign = match rest[16] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let offset_hours = two_digits(rest, 17)?;
        let offset_minutes = two_digits(rest, 19)?;
        if offset_hours > 23 || offset_minutes > 59 {
            return None;
        }
        sign * (offset_hours * 3600 + offset_minutes * 60)
    };
    // `ZoneOffset.ofTotalSeconds` accepts +-18:00.
    if offset_seconds.abs() > 18 * 3600 {
        return None;
    }
    // The ISO resolver: month 1-12, day 1-31, then SMART clamps to the month's length.
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || minute > 59 {
        return None;
    }
    // `24:00:00` is the end of the day; any other hour above 23, or a leap second, is an error.
    let (hour, extra_days) = match hour {
        0..=23 if second <= 59 => (hour, 0),
        24 if minute == 0 && second == 0 => (0, 1),
        _ => return None,
    };
    let day = day.min(days_in_month(year, month));
    // `LocalDate` ends at +999999999-12-31; the extra day of `24:00:00` can step past it.
    let epoch_day = days_from_civil(year, month, day) + extra_days;
    if epoch_day > MAX_EPOCH_DAY {
        return None;
    }
    let local = epoch_day * 86_400 + hour * 3600 + minute * 60 + second;
    Some(local - offset_seconds)
}

/// `yyyy-MM-dd HH:mm:ss +0000` of an instant. `yyyy` is the year of the era (a year before 1 CE
/// prints as the BCE year, without an era), with a `+` once it needs more than four digits.
/// `None` where the original throws: the day is outside what `LocalDate` can hold.
pub fn format_obtained(epoch_seconds: i64) -> Option<String> {
    let days = epoch_seconds.div_euclid(86_400);
    if !(MIN_EPOCH_DAY..=MAX_EPOCH_DAY).contains(&days) {
        return None;
    }
    let seconds = epoch_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let year_of_era = if year > 0 { year } else { 1 - year };
    let year_text = if year_of_era >= 10_000 {
        format!("+{year_of_era}")
    } else {
        format!("{year_of_era:04}")
    };
    Some(format!(
        "{year_text}-{month:02}-{day:02} {:02}:{:02}:{:02} +0000",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    ))
}
