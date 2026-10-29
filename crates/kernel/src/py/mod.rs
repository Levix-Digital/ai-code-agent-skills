//! The parts of Python the scripts relied on, reproduced exactly: what they print and how they
//! read their input must not change when a script is translated (IMP-005).

pub mod argparse;
pub mod json;
pub mod num;
pub mod path;
pub mod textwrap;
pub mod unicode;
pub mod value;
pub mod yaml;

use std::fmt;
use std::path::Path;

pub use value::{Dict, EMPTY, NONE, Value};

/// An exception the Python script did not catch: Python printed a traceback and exited with 1.
/// The translation prints the exception's last line and exits with 1 too.
#[derive(Debug, Clone, PartialEq)]
pub struct Crash(pub String);

impl Crash {
    pub fn new(msg: impl Into<String>) -> Self {
        Crash(msg.into())
    }

    /// Writes the exception to stderr; the exit code is 1.
    pub fn report(&self) -> u8 {
        crate::out::err(&format!("Traceback (most recent call last):\n{}\n", self.0));
        1
    }
}

impl fmt::Display for Crash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A lone surrogate (`"\ud800"` in JSON) is a valid Python string but not a Rust one: it is
/// kept as a character of the last private-use plane, U+10F800 to U+10FFFF, so that printing
/// or writing it fails as it does in Python, and `repr` shows it as Python does.
pub fn surrogate_char(code: u32) -> char {
    char::from_u32(0x10f800 + (code & 0x7ff)).unwrap_or('\u{fffd}')
}

pub fn as_surrogate(c: char) -> Option<u32> {
    let u = c as u32;
    (u >= 0x10f800).then(|| 0xd800 + (u - 0x10f800))
}

/// `str.encode("utf-8")` fails on a lone surrogate: the exception Python raises.
pub fn encode_check(text: &str) -> Result<(), Crash> {
    match text
        .chars()
        .enumerate()
        .find_map(|(i, c)| as_surrogate(c).map(|s| (i, s)))
    {
        None => Ok(()),
        Some((pos, s)) => Err(Crash::new(format!(
            "UnicodeEncodeError: 'utf-8' codec can't encode character '\\u{s:04x}' in position {pos}: surrogates not allowed"
        ))),
    }
}

/// `open(path, encoding="utf-8").read()`: universal newlines (`\r\n` and `\r` become `\n`).
pub fn read_text(path: &Path) -> Result<String, Crash> {
    let bytes = std::fs::read(path).map_err(|e| io_crash(&e, path))?;
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            return Err(Crash::new(format!(
                "UnicodeDecodeError: {}",
                utf8_error_text(e.as_bytes(), &e.utf8_error())
            )));
        }
    };
    Ok(universal_newlines(&text))
}

/// `open(path, "w", encoding="utf-8", newline="\n").write(text)`: the file is emptied
/// before a write that may fail on a lone surrogate.
pub fn write_text(path: &Path, text: &str) -> Result<(), Crash> {
    let bytes = if encode_check(text).is_ok() { text } else { "" };
    std::fs::write(path, bytes).map_err(|e| io_crash(&e, path))?;
    encode_check(text)
}

/// `str(UnicodeDecodeError)` for bytes that are not UTF-8.
pub fn utf8_error_text(bytes: &[u8], e: &std::str::Utf8Error) -> String {
    let start = e.valid_up_to();
    let (end, reason) = match e.error_len() {
        None => (bytes.len(), "unexpected end of data"),
        Some(1) if !(0xc2..=0xf4).contains(&bytes[start]) => (start + 1, "invalid start byte"),
        Some(n) => (start + n, "invalid continuation byte"),
    };
    if end - start == 1 {
        format!(
            "'utf-8' codec can't decode byte 0x{:02x} in position {start}: {reason}",
            bytes[start]
        )
    } else {
        format!(
            "'utf-8' codec can't decode bytes in position {start}-{}: {reason}",
            end - 1
        )
    }
}

/// `datetime.date.today().isoformat()`: the local date.
pub fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

pub fn universal_newlines(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    }
}

/// The exception Python raises for a failed file operation.
pub fn io_crash(e: &std::io::Error, path: &Path) -> Crash {
    use std::io::ErrorKind::*;
    let (name, errno, text) = match e.kind() {
        NotFound => ("FileNotFoundError", 2, "No such file or directory"),
        PermissionDenied => ("PermissionError", 13, "Permission denied"),
        IsADirectory => ("IsADirectoryError", 21, "Is a directory"),
        AlreadyExists => ("FileExistsError", 17, "File exists"),
        NotADirectory => ("NotADirectoryError", 20, "Not a directory"),
        _ => ("OSError", e.raw_os_error().unwrap_or(0), "I/O error"),
    };
    Crash::new(format!(
        "{name}: [Errno {errno}] {text}: {}",
        repr_str(&path.to_string_lossy())
    ))
}

/// `repr(float)`: the shortest text that reads back as the same number, in Python's layout.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.into();
    }
    // Rust's `{:e}` gives the same shortest digits; only the layout differs.
    let sci = format!("{f:e}");
    let (sign, sci) = match sci.strip_prefix('-') {
        Some(rest) => ("-", rest.to_string()),
        None => ("", sci),
    };
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1;
    let n = digits.len() as i32;
    let body = if decpt <= -4 || decpt > 16 {
        let mut m = digits[..1].to_string();
        if n > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        let e = decpt - 1;
        format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else if decpt <= 0 {
        format!("0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt >= n {
        format!("{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        format!(
            "{}.{}",
            &digits[..decpt as usize],
            &digits[decpt as usize..]
        )
    };
    format!("{sign}{body}")
}

/// `str.isprintable()` for one character, close enough for the text the scripts handle:
/// control and format characters, surrogates, private use and separators other than space
/// are not printable.
pub fn is_printable(c: char) -> bool {
    let u = c as u32;
    if c == ' ' {
        return true;
    }
    if c.is_control() || c.is_whitespace() {
        return false;
    }
    !matches!(u,
        0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x890..=0x891 | 0x8e2 | 0x180e
        | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x2064 | 0x2066..=0x206f
        | 0xfeff | 0xfff9..=0xfffb | 0x110bd | 0x110cd | 0x13430..=0x1343f
        | 0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f
        | 0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd
        | 0xfdd0..=0xfdef)
        && (u & 0xfffe) != 0xfffe
}

/// `repr(str)`.
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        let u = c as u32;
        match c {
            c if as_surrogate(c).is_some() => {
                out.push_str(&format!("\\u{:04x}", as_surrogate(c).unwrap_or(0)));
            }
            c if c == quote || c == '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if u < 0x20 || u == 0x7f => out.push_str(&format!("\\x{u:02x}")),
            c if u < 0x7f => out.push(c),
            c if is_printable(c) => out.push(c),
            _ if u <= 0xff => out.push_str(&format!("\\x{u:02x}")),
            _ if u <= 0xffff => out.push_str(&format!("\\u{u:04x}")),
            _ => out.push_str(&format!("\\U{u:08x}")),
        }
    }
    out.push(quote);
    out
}

/// `sep.join(value)`: a list of strings, the characters of a string, or the keys of a dict.
pub fn join(sep: &str, value: &Value) -> Result<String, Crash> {
    let parts: Vec<String> = match value {
        Value::Str(s) => s.chars().map(String::from).collect(),
        Value::Dict(d) => return join_items(sep, &d.keys().cloned().collect::<Vec<_>>()),
        Value::List(items) | Value::Tuple(items) | Value::Set(items) => {
            return join_items(sep, items);
        }
        Value::Bytes(b) if !b.is_empty() => {
            return Err(Crash::new(
                "TypeError: sequence item 0: expected str instance, int found",
            ));
        }
        Value::Bytes(_) => Vec::new(),
        _ => return Err(Crash::new("TypeError: can only join an iterable")),
    };
    Ok(parts.join(sep))
}

fn join_items(sep: &str, items: &[Value]) -> Result<String, Crash> {
    let parts: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Value::Str(s) => Ok(s.clone()),
            other => Err(Crash::new(format!(
                "TypeError: sequence item {i}: expected str instance, {} found",
                other.type_name()
            ))),
        })
        .collect::<Result<_, _>>()?;
    Ok(parts.join(sep))
}

pub use unicode::{is_word, lower};

/// `re.match(pattern + "$", s)` for a pattern checked by `full`: Python's `$` also matches
/// before a newline that ends the string.
pub fn matches_to_end(s: &str, full: impl Fn(&str) -> bool) -> bool {
    full(s) || s.strip_suffix('\n').is_some_and(full)
}

/// The items of `for x in value`.
pub fn iterate(value: &Value) -> Result<Vec<Value>, Crash> {
    match value {
        Value::List(items) | Value::Tuple(items) | Value::Set(items) => Ok(items.clone()),
        Value::Dict(d) => Ok(d.keys().cloned().collect()),
        Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Bytes(b) => Ok(b.iter().map(|x| Value::int(i64::from(*x))).collect()),
        other => Err(Crash::new(format!(
            "TypeError: '{}' object is not iterable",
            other.type_name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_repr_matches_python() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (2.5, "2.5"),
            (0.1, "0.1"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345680.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1e100, "1e+100"),
            (-12.75, "-12.75"),
        ];
        for (f, want) in cases {
            assert_eq!(float_repr(f), want, "{f}");
        }
    }

    #[test]
    fn repr_str_matches_python() {
        assert_eq!(repr_str("abc"), "'abc'");
        assert_eq!(repr_str("it's"), "\"it's\"");
        assert_eq!(repr_str("a'b\"c"), "'a\\'b\"c'");
        assert_eq!(repr_str("é\n\u{7f}\u{a0}"), "'é\\n\\x7f\\xa0'");
    }
}
