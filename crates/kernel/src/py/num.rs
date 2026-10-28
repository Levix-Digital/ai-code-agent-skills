//! Python's `int(text, base)` and `float(text)`, and the unlimited integers they return.

use super::{Crash, repr_str};

/// `str.isspace()`.
pub fn is_space(c: char) -> bool {
    matches!(c as u32,
        0x09..=0x0d | 0x1c..=0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029
        | 0x202f | 0x205f | 0x3000)
}

/// `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// The first code point of each run of ten Unicode decimal digits (Python 3.13, Unicode 15.1).
const DIGIT_ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
    0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
    0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10,
    0x104a0, 0x10d30, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0, 0x11650,
    0x116c0, 0x11730, 0x118e0, 0x11950, 0x11c50, 0x11d50, 0x11da0, 0x11f50, 0x16a60, 0x16ac0,
    0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e950,
    0x1fbf0,
];

/// `unicodedata.decimal(c)`.
pub fn decimal_value(c: char) -> Option<u32> {
    let u = c as u32;
    let i = DIGIT_ZEROS.partition_point(|&z| z <= u);
    (i > 0 && u - DIGIT_ZEROS[i - 1] < 10).then(|| u - DIGIT_ZEROS[i - 1])
}

/// What `int()` and `float()` read: Unicode digits as ASCII, Unicode spaces as `' '`, any
/// other non-ASCII character as one that no number accepts.
fn to_ascii(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                c
            } else if is_space(c) {
                ' '
            } else if let Some(d) = decimal_value(c) {
                char::from(b'0' + d as u8)
            } else {
                '?'
            }
        })
        .collect()
}

/// An integer of any size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    negative: bool,
    /// Base 10^9, least significant first, no trailing zero limbs (zero is empty).
    limbs: Vec<u32>,
}

const LIMB: u64 = 1_000_000_000;

impl BigInt {
    pub fn zero() -> Self {
        BigInt {
            negative: false,
            limbs: Vec::new(),
        }
    }

    pub fn from_i64(v: i64) -> Self {
        let mut b = BigInt::zero();
        let mut m = v.unsigned_abs();
        while m > 0 {
            b.limbs.push((m % LIMB) as u32);
            m /= LIMB;
        }
        b.negative = v < 0;
        b
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.negative = false;
        }
    }

    /// `self * m + a` on the magnitude.
    fn mul_add_small(&mut self, m: u32, a: u32) {
        let mut carry = a as u64;
        for limb in self.limbs.iter_mut() {
            let v = *limb as u64 * m as u64 + carry;
            *limb = (v % LIMB) as u32;
            carry = v / LIMB;
        }
        while carry > 0 {
            self.limbs.push((carry % LIMB) as u32);
            carry /= LIMB;
        }
    }

    pub fn negated(mut self) -> Self {
        if !self.is_zero() {
            self.negative = !self.negative;
        }
        self
    }

    fn cmp_mag(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
        a.len()
            .cmp(&b.len())
            .then_with(|| a.iter().rev().cmp(b.iter().rev()))
    }

    pub fn add(&self, other: &BigInt) -> BigInt {
        use std::cmp::Ordering::*;
        if self.negative == other.negative {
            let mut limbs = Vec::new();
            let mut carry = 0u64;
            for i in 0..self.limbs.len().max(other.limbs.len()) {
                let v = *self.limbs.get(i).unwrap_or(&0) as u64
                    + *other.limbs.get(i).unwrap_or(&0) as u64
                    + carry;
                limbs.push((v % LIMB) as u32);
                carry = v / LIMB;
            }
            if carry > 0 {
                limbs.push(carry as u32);
            }
            let mut r = BigInt {
                negative: self.negative,
                limbs,
            };
            r.trim();
            return r;
        }
        let (big, small) = match Self::cmp_mag(&self.limbs, &other.limbs) {
            Equal => return BigInt::zero(),
            Greater => (self, other),
            Less => (other, self),
        };
        let mut limbs = Vec::new();
        let mut borrow = 0i64;
        for i in 0..big.limbs.len() {
            let mut v = big.limbs[i] as i64 - *small.limbs.get(i).unwrap_or(&0) as i64 - borrow;
            borrow = 0;
            if v < 0 {
                v += LIMB as i64;
                borrow = 1;
            }
            limbs.push(v as u32);
        }
        let mut r = BigInt {
            negative: big.negative,
            limbs,
        };
        r.trim();
        r
    }

    pub fn mul(&self, other: &BigInt) -> BigInt {
        let mut acc = vec![0u64; self.limbs.len() + other.limbs.len() + 1];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.limbs.iter().enumerate() {
                let v = acc[i + j] + a as u64 * b as u64 + carry;
                acc[i + j] = v % LIMB;
                carry = v / LIMB;
            }
            let mut k = i + other.limbs.len();
            while carry > 0 {
                let v = acc[k] + carry;
                acc[k] = v % LIMB;
                carry = v / LIMB;
                k += 1;
            }
        }
        let mut r = BigInt {
            negative: self.negative != other.negative,
            limbs: acc.into_iter().map(|v| v as u32).collect(),
        };
        r.trim();
        r
    }

    /// `float(self)`: correctly rounded, `OverflowError` when too large.
    pub fn to_f64(&self) -> Result<f64, Crash> {
        let f: f64 = self.to_string().parse().unwrap_or(f64::INFINITY);
        if f.is_infinite() {
            return Err(Crash::new(
                "OverflowError: int too large to convert to float",
            ));
        }
        Ok(f)
    }
}

impl std::fmt::Display for BigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.limbs.is_empty() {
            return f.write_str("0");
        }
        let mut s = String::new();
        if self.negative {
            s.push('-');
        }
        let mut iter = self.limbs.iter().rev();
        if let Some(top) = iter.next() {
            s.push_str(&top.to_string());
        }
        for limb in iter {
            s.push_str(&format!("{limb:09}"));
        }
        f.write_str(&s)
    }
}

/// `int(text, base)` for a `str`.
pub fn int(text: &str, base: u32) -> Result<BigInt, Crash> {
    let invalid = || {
        let r = repr_str(text);
        let r: String = r.chars().take(200).collect();
        Crash::new(format!(
            "ValueError: invalid literal for int() with base {base}: {r}"
        ))
    };
    let ascii = to_ascii(text);
    let s = ascii.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b');
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let lower = s.to_ascii_lowercase();
    let prefix = match base {
        16 => "0x",
        8 => "0o",
        2 => "0b",
        _ => "",
    };
    let (s, prefixed) = if !prefix.is_empty() && lower.starts_with(prefix) {
        (&s[2..], true)
    } else {
        (s, false)
    };
    // One underscore may follow the base prefix or sit between two digits.
    let s = if prefixed {
        s.strip_prefix('_').unwrap_or(s)
    } else {
        s
    };
    if s.is_empty() || s.starts_with('_') || s.ends_with('_') || s.contains("__") {
        return Err(invalid());
    }
    let mut n = BigInt::zero();
    let mut digits = 0usize;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(base).ok_or_else(invalid)?;
        n.mul_add_small(base, d);
        digits += 1;
    }
    if base == 10 && digits > 4300 {
        return Err(Crash::new(format!(
            "ValueError: Exceeds the limit (4300 digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
        )));
    }
    n.trim();
    Ok(if negative { n.negated() } else { n })
}

/// `float(text)` for a `str`.
pub fn float(text: &str) -> Result<f64, Crash> {
    let invalid = || {
        Crash::new(format!(
            "ValueError: could not convert string to float: {}",
            repr_str(text)
        ))
    };
    let ascii = to_ascii(text);
    let s = ascii.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b');
    // Underscores only between digits.
    let b = s.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'_'
            && !(i > 0 && b[i - 1].is_ascii_digit() && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            return Err(invalid());
        }
    }
    let s: String = s.chars().filter(|&c| c != '_').collect();
    let body = s.strip_prefix(['+', '-']).unwrap_or(&s);
    let lower = body.to_ascii_lowercase();
    let named = matches!(lower.as_str(), "inf" | "infinity" | "nan");
    let numeric = !body.is_empty()
        && body
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !named && !numeric {
        return Err(invalid());
    }
    s.parse::<f64>().map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints_read_like_python() {
        assert_eq!(int("0x1f", 16).unwrap().to_string(), "31");
        assert_eq!(int(" -12 ", 10).unwrap().to_string(), "-12");
        assert_eq!(int("١٢", 10).unwrap().to_string(), "12");
        assert_eq!(int("0b", 16).unwrap().to_string(), "11");
        assert!(int("09", 8).is_err());
        assert_eq!(
            int("123456789012345678901234567890", 10)
                .unwrap()
                .to_string(),
            "123456789012345678901234567890"
        );
        let a = int("1000000000000", 10).unwrap();
        let b = BigInt::from_i64(-1);
        assert_eq!(a.add(&b).to_string(), "999999999999");
        assert_eq!(a.mul(&b).to_string(), "-1000000000000");
    }

    #[test]
    fn floats_read_like_python() {
        assert_eq!(float("1e3").unwrap(), 1000.0);
        assert!(float("Infinity").unwrap().is_infinite());
        assert!(float("1e").is_err());
        assert!(float("0x10").is_err());
    }
}
