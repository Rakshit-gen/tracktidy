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

/// Parses "YYYY-MM-DDTHH:MM:SS", with optional fractional seconds and an
/// optional zone: "Z", "+02:00" or "-0530". No zone is read as UTC, which is
/// what the GPX spec requires anyway; some phone apps just leave the Z off.
pub fn parse_time(s: &str) -> Option<Seconds> {
    let s = s.trim();
    if s.len() < 19 || &s[4..5] != "-" || &s[7..8] != "-" || !matches!(&s[10..11], "T" | " ") {
        return None;
    }
    let (y, mo, d): (i64, u32, u32) = (num(s, 0..4)?, num(s, 5..7)?, num(s, 8..10)?);
    let (h, mi, sec): (i64, i64, i64) = (num(s, 11..13)?, num(s, 14..16)?, num(s, 17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut frac = 0.0;
    if let Some(after_dot) = rest.strip_prefix('.') {
        let digits = after_dot.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        frac = format!("0.{}", &after_dot[..digits]).parse().ok()?;
        rest = &after_dot[digits..];
    }
    let offset = match rest {
        "" | "Z" | "z" => 0,
        _ => {
            let sign = match &rest[..1] {
                "+" => 1,
                "-" => -1,
                _ => return None,
            };
            let digits: String = rest[1..].chars().filter(|c| *c != ':').collect();
            if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let oh: i64 = digits[..2].parse().ok()?;
            let om: i64 = digits[2..].parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };
    let days = days_from_civil(y, mo, d);
    Some((days * 86_400 + h * 3600 + mi * 60 + sec - offset) as f64 + frac)
}
