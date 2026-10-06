//! GPX timestamps. GPX uses ISO 8601 in UTC, like 2024-05-01T07:02:03Z.

/// Seconds since 1970-01-01 UTC. Kept as f64 because some devices record
/// fractions of a second.
pub type Seconds = f64;

/// Days from 1970-01-01 to the given date in the proleptic Gregorian calendar.
/// Howard Hinnant's days_from_civil, which needs no tables or leap year loops.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn num<T: std::str::FromStr>(s: &str, range: std::ops::Range<usize>) -> Option<T> {
    s.get(range)?.parse().ok()
}

/// Parses "YYYY-MM-DDTHH:MM:SSZ".
pub fn parse_time(s: &str) -> Option<Seconds> {
    let s = s.trim();
    if s.len() < 20 || &s[4..5] != "-" || &s[7..8] != "-" || !matches!(&s[10..11], "T" | " ") || !s.ends_with('Z') {
        return None;
    }
    let (y, mo, d): (i64, u32, u32) = (num(s, 0..4)?, num(s, 5..7)?, num(s, 8..10)?);
    let (h, mi, sec): (i64, i64, i64) = (num(s, 11..13)?, num(s, 14..16)?, num(s, 17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    Some((days * 86_400 + h * 3600 + mi * 60 + sec) as f64)
}
