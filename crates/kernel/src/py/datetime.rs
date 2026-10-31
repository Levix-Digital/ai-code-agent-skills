//! `datetime.datetime.fromisoformat(text)` as CPython 3.13's C code parses it (byte by byte,
//! including its quirks) and `isoformat()`.

/// A naive or aware datetime; `offset` is the UTC offset in microseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub microsecond: u32,
    pub offset: Option<i64>,
}

/// The UTF-8 bytes of the text, read as C reads its NUL-terminated buffer.
struct Buf(Vec<u8>);

impl Buf {
    fn at(&self, i: usize) -> u8 {
        self.0.get(i).copied().unwrap_or(0)
    }

    /// `parse_digits`: `n` digits from `p`, or `None`.
    fn digits(&self, p: &mut usize, n: usize, var: &mut i64) -> Option<()> {
        for _ in 0..n {
            let c = self.at(*p);
            *p += 1;
            if !c.is_ascii_digit() {
                return None;
            }
            *var = *var * 10 + i64::from(c - b'0');
        }
        Some(())
    }
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

/// `ymd_to_ord`: day 1 is 0001-01-01.
fn ymd_to_ord(year: i64, month: i64, day: i64) -> i64 {
    let y = year - 1;
    let before_month: i64 = (1..month).map(|m| days_in_month(year, m)).sum();
    y * 365 + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) + before_month + day
}

/// `ord_to_ymd`.
fn ord_to_ymd(ordinal: i64) -> (i64, i64, i64) {
    // Days since 1970-01-01, then Howard Hinnant's civil_from_days.
    let z = ordinal - 719_163 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `iso_to_ymd`.
fn iso_to_ymd(year: i64, week: i64, day: i64) -> Option<(i64, i64, i64)> {
    if !(1..=9999).contains(&year) {
        return None;
    }
    if week <= 0 || week >= 53 {
        let first_weekday = (ymd_to_ord(year, 1, 1) + 6) % 7;
        let ok = week == 53 && (first_weekday == 3 || (first_weekday == 2 && is_leap(year)));
        if !ok {
            return None;
        }
    }
    if day <= 0 || day >= 8 {
        return None;
    }
    let first_day = ymd_to_ord(year, 1, 1);
    let first_weekday = (first_day + 6) % 7;
    let mut week1_monday = first_day - first_weekday;
    if first_weekday > 3 {
        week1_monday += 7;
    }
    Some(ord_to_ymd(week1_monday + (week - 1) * 7 + day - 1))
}

/// `_find_isoformat_datetime_separator`.
fn separator(b: &Buf, len: usize) -> Option<usize> {
    if len == 7 {
        return Some(7);
    }
    if b.at(4) == b'-' {
        if b.at(5) == b'W' {
            if len < 8 {
                return None;
            }
            if len > 8 && b.at(8) == b'-' {
                if len == 9 {
                    return None;
                }
                if len > 10 && b.at(10).is_ascii_digit() {
                    return Some(8);
                }
                return Some(10);
            }
            return Some(8);
        }
        return Some(10);
    }
    if b.at(4) == b'W' {
        let mut idx = 7;
        while idx < len && b.at(idx).is_ascii_digit() {
            idx += 1;
        }
        if idx < 9 {
            return Some(idx);
        }
        return Some(if idx % 2 == 0 { 7 } else { 8 });
    }
    Some(8)
}

/// `parse_isoformat_date`.
fn parse_date(b: &Buf, len: usize) -> Option<(i64, i64, i64)> {
    let mut p = 0;
    let (mut year, mut month, mut day) = (0, 0, 0);
    b.digits(&mut p, 4, &mut year)?;
    let sep = b.at(p) == b'-';
    if sep {
        p += 1;
    }
    if b.at(p) == b'W' {
        p += 1;
        let (mut week, mut wday) = (0, 0);
        b.digits(&mut p, 2, &mut week)?;
        if p < len {
            if sep {
                let c = b.at(p);
                p += 1;
                if c != b'-' {
                    return None;
                }
            }
            b.digits(&mut p, 1, &mut wday)?;
        } else {
            wday = 1;
        }
        return iso_to_ymd(year, week, wday);
    }
    b.digits(&mut p, 2, &mut month)?;
    if sep {
        let c = b.at(p);
        p += 1;
        if c != b'-' {
            return None;
        }
    }
    b.digits(&mut p, 2, &mut day)?;
    Some((year, month, day))
}

/// `parse_hh_mm_ss_ff` between `p` and `end`: the four fields and whether text follows
/// (`1`), or `None` for an error.
fn parse_hms(b: &Buf, mut p: usize, end: usize) -> Option<([i64; 4], bool)> {
    let mut vals = [0i64; 4];
    let mut has_sep = true;
    for i in 0..3 {
        b.digits(&mut p, 2, &mut vals[i])?;
        let c = b.at(p);
        p += 1;
        if i == 0 {
            has_sep = c == b':';
        }
        if c == b'.' || c == b',' {
            if p >= end {
                return None;
            }
            break;
        } else if p >= end {
            return Some((vals, c != 0));
        } else if has_sep && c == b':' {
            continue;
        } else if !has_sep {
            p -= 1;
        } else {
            return None;
        }
    }
    let to_parse = (end - p).min(6);
    b.digits(&mut p, to_parse, &mut vals[3])?;
    if to_parse < 6 {
        vals[3] *= 10i64.pow((6 - to_parse) as u32);
    }
    while b.at(p).is_ascii_digit() {
        p += 1;
    }
    Some((vals, b.at(p) != 0))
}

/// `parse_isoformat_time` from byte `start`: the time and the offset in microseconds.
fn parse_time(b: &Buf, start: usize, end: usize) -> Option<([i64; 4], Option<i64>)> {
    let mut tz = start;
    loop {
        if matches!(b.at(tz), b'Z' | b'+' | b'-') {
            break;
        }
        tz += 1;
        if tz >= end {
            break;
        }
    }
    let (time, more) = parse_hms(b, start, tz)?;
    if tz == end {
        return if more { None } else { Some((time, None)) };
    }
    if b.at(tz) == b'Z' {
        return if b.at(tz + 1) != 0 {
            None
        } else {
            Some((time, Some(0)))
        };
    }
    let sign = if b.at(tz) == b'-' { -1 } else { 1 };
    let (t, more) = parse_hms(b, tz + 1, end)?;
    if more {
        return None;
    }
    let secs = sign * (t[0] * 3600 + t[1] * 60 + t[2]);
    Some((time, Some(secs * 1_000_000 + sign * t[3])))
}

/// `datetime.fromisoformat(text)`, `None` where Python raises `ValueError`.
pub fn fromisoformat(text: &str) -> Option<DateTime> {
    if text.chars().count() < 7 {
        return None;
    }
    let b = Buf(text.as_bytes().to_vec());
    let len = b.0.len();
    let sep = separator(&b, len)?;
    let (year, month, day) = parse_date(&b, sep)?;
    let (time, offset) = if len > sep {
        let lead = b.at(sep);
        let width = if lead & 0x80 == 0 {
            1
        } else {
            match lead & 0xf0 {
                0xe0 => 3,
                0xf0 => 4,
                _ => 2,
            }
        };
        parse_time(&b, sep + width, len.max(sep + width))?
    } else {
        ([0; 4], None)
    };
    if offset.is_some_and(|off| off.abs() >= 86_400_000_000) {
        return None;
    }
    let valid = (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && (0..24).contains(&time[0])
        && (0..60).contains(&time[1])
        && (0..60).contains(&time[2]);
    valid.then(|| DateTime {
        year: year as u32,
        month: month as u32,
        day: day as u32,
        hour: time[0] as u32,
        minute: time[1] as u32,
        second: time[2] as u32,
        microsecond: time[3] as u32,
        offset,
    })
}

impl DateTime {
    /// `isoformat()`.
    pub fn isoformat(&self) -> String {
        let mut out = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        );
        if self.microsecond != 0 {
            out.push_str(&format!(".{:06}", self.microsecond));
        }
        if let Some(off) = self.offset {
            let sign = if off < 0 { '-' } else { '+' };
            let off = off.unsigned_abs();
            let (secs, us) = (off / 1_000_000, off % 1_000_000);
            let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
            out.push_str(&format!("{sign}{h:02}:{m:02}"));
            if s != 0 || us != 0 {
                out.push_str(&format!(":{s:02}"));
                if us != 0 {
                    out.push_str(&format!(".{us:06}"));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::fromisoformat;

    fn iso(s: &str) -> Option<String> {
        fromisoformat(s).map(|d| d.isoformat())
    }

    #[test]
    fn parses_like_python() {
        assert_eq!(
            iso("2025-03-01T10:00:00.5").as_deref(),
            Some("2025-03-01T10:00:00.500000")
        );
        assert_eq!(iso("2025-03-01").as_deref(), Some("2025-03-01T00:00:00"));
        assert_eq!(
            iso("2025-03-01 10:00Z").as_deref(),
            Some("2025-03-01T10:00:00+00:00")
        );
        assert_eq!(
            iso("20250301T1000-0230").as_deref(),
            Some("2025-03-01T10:00:00-02:30")
        );
        assert_eq!(iso("2025-02-30"), None);
        assert_eq!(iso("not a date"), None);
        assert_eq!(iso("2025-W01-1").as_deref(), Some("2024-12-30T00:00:00"));
    }
}
