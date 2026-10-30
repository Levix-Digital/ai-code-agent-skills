//! Python's `json` module: `json.loads` (CPython's C scanner, with its error messages and
//! positions) and `json.dumps(..., ensure_ascii=False)`, with or without `indent`.

use super::value::{Dict, Value};
use super::{Crash, float_repr};
use std::fmt;

/// `json.JSONDecodeError`: the message and the position, counted in characters.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeError {
    pub msg: String,
    pub lineno: usize,
    pub colno: usize,
    pub pos: usize,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: line {} column {} (char {})",
            self.msg, self.lineno, self.colno, self.pos
        )
    }
}

#[derive(Debug)]
pub enum LoadError {
    Decode(DecodeError),
    Crash(Crash),
}

impl From<Crash> for LoadError {
    fn from(c: Crash) -> Self {
        LoadError::Crash(c)
    }
}

/// Python's recursion limit reached while decoding nested containers.
const MAX_DEPTH: usize = 990;

enum Fail {
    /// `StopIteration(idx)`: reported as "Expecting value" at `idx`.
    Stop(usize),
    Error(&'static str, usize),
    Crash(Crash),
}

struct Scanner {
    s: Vec<char>,
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

impl Scanner {
    fn at(&self, i: usize) -> char {
        self.s[i]
    }

    fn len(&self) -> usize {
        self.s.len()
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        while i < self.len() && is_ws(self.at(i)) {
            i += 1;
        }
        i
    }

    fn matches(&self, i: usize, word: &str) -> bool {
        let w: Vec<char> = word.chars().collect();
        i + w.len() <= self.len() && self.s[i..i + w.len()] == w[..]
    }

    fn scan_once(&self, idx: usize, depth: usize) -> Result<(Value, usize), Fail> {
        let len = self.len();
        if idx >= len {
            return Err(Fail::Stop(idx));
        }
        match self.at(idx) {
            '"' => return self.scan_string(idx + 1).map(|(s, e)| (Value::Str(s), e)),
            '{' => return self.parse_object(idx + 1, depth + 1),
            '[' => return self.parse_array(idx + 1, depth + 1),
            'n' if idx + 3 < len && self.matches(idx, "null") => return Ok((Value::Null, idx + 4)),
            't' if idx + 3 < len && self.matches(idx, "true") => {
                return Ok((Value::Bool(true), idx + 4));
            }
            'f' if idx + 4 < len && self.matches(idx, "false") => {
                return Ok((Value::Bool(false), idx + 5));
            }
            'N' if idx + 2 < len && self.matches(idx, "NaN") => {
                return Ok((Value::Float(f64::NAN), idx + 3));
            }
            'I' if idx + 7 < len && self.matches(idx, "Infinity") => {
                return Ok((Value::Float(f64::INFINITY), idx + 8));
            }
            '-' if idx + 8 < len && self.matches(idx, "-Infinity") => {
                return Ok((Value::Float(f64::NEG_INFINITY), idx + 9));
            }
            _ => {}
        }
        self.match_number(idx)
    }

    fn is_digit(&self, i: usize) -> bool {
        self.at(i).is_ascii_digit()
    }

    fn match_number(&self, start: usize) -> Result<(Value, usize), Fail> {
        let len = self.len();
        let end_idx = len - 1;
        let mut idx = start;
        if self.at(idx) == '-' {
            idx += 1;
            if idx > end_idx {
                return Err(Fail::Stop(start));
            }
        }
        let c = self.at(idx);
        if ('1'..='9').contains(&c) {
            idx += 1;
            while idx <= end_idx && self.is_digit(idx) {
                idx += 1;
            }
        } else if c == '0' {
            idx += 1;
        } else {
            return Err(Fail::Stop(start));
        }
        let mut is_float = false;
        if idx < end_idx && self.at(idx) == '.' && self.is_digit(idx + 1) {
            is_float = true;
            idx += 2;
            while idx <= end_idx && self.is_digit(idx) {
                idx += 1;
            }
        }
        if idx < end_idx && matches!(self.at(idx), 'e' | 'E') {
            let e_start = idx;
            idx += 1;
            if idx < end_idx && matches!(self.at(idx), '-' | '+') {
                idx += 1;
            }
            while idx <= end_idx && self.is_digit(idx) {
                idx += 1;
            }
            if self.is_digit(idx - 1) {
                is_float = true;
            } else {
                idx = e_start;
            }
        }
        let text: String = self.s[start..idx].iter().collect();
        let value = if is_float {
            Value::Float(text.parse::<f64>().unwrap_or(f64::NAN))
        } else {
            Value::int_from_text(&text)
        };
        Ok((value, idx))
    }

    fn hex(c: char) -> Option<u32> {
        c.to_digit(16)
    }

    /// `scanstring`: `end` is just after the opening quote.
    fn scan_string(&self, mut end: usize) -> Result<(String, usize), Fail> {
        let len = self.len();
        let begin = end - 1;
        let mut out = String::new();
        loop {
            let mut next = end;
            let mut c = '\0';
            while next < len {
                c = self.at(next);
                if c == '"' || c == '\\' {
                    break;
                }
                if (c as u32) <= 0x1f {
                    return Err(Fail::Error("Invalid control character at", next));
                }
                next += 1;
            }
            if next >= len || !(c == '"' || c == '\\') {
                return Err(Fail::Error("Unterminated string starting at", begin));
            }
            out.extend(&self.s[end..next]);
            next += 1;
            if c == '"' {
                end = next;
                break;
            }
            if next == len {
                return Err(Fail::Error("Unterminated string starting at", begin));
            }
            let c = self.at(next);
            if c != 'u' {
                end = next + 1;
                let ch = match c {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return Err(Fail::Error("Invalid \\escape", end - 2)),
                };
                out.push(ch);
            } else {
                next += 1;
                end = next + 4;
                if end >= len {
                    return Err(Fail::Error("Invalid \\uXXXX escape", next - 1));
                }
                let mut code: u32 = 0;
                while next < end {
                    match Self::hex(self.at(next)) {
                        Some(d) => code = (code << 4) | d,
                        None => return Err(Fail::Error("Invalid \\uXXXX escape", end - 5)),
                    }
                    next += 1;
                }
                if (0xd800..=0xdbff).contains(&code)
                    && end + 6 < len
                    && self.at(next) == '\\'
                    && self.at(next + 1) == 'u'
                {
                    next += 2;
                    end += 6;
                    let mut c2: u32 = 0;
                    while next < end {
                        match Self::hex(self.at(next)) {
                            Some(d) => c2 = (c2 << 4) | d,
                            None => return Err(Fail::Error("Invalid \\uXXXX escape", end - 5)),
                        }
                        next += 1;
                    }
                    if (0xdc00..=0xdfff).contains(&c2) {
                        code = 0x10000 + (((code - 0xd800) << 10) | (c2 - 0xdc00));
                    } else {
                        end -= 6;
                    }
                }
                out.push(char::from_u32(code).unwrap_or_else(|| super::surrogate_char(code)));
            }
        }
        Ok((out, end))
    }

    fn parse_object(&self, mut idx: usize, depth: usize) -> Result<(Value, usize), Fail> {
        if depth > MAX_DEPTH {
            return Err(Fail::Crash(Crash::new(
                "RecursionError: maximum recursion depth exceeded while decoding a JSON object from a unicode string",
            )));
        }
        let len = self.len();
        let mut map = Dict::new();
        idx = self.skip_ws(idx);
        if idx >= len || self.at(idx) != '}' {
            loop {
                if idx >= len || self.at(idx) != '"' {
                    return Err(Fail::Error(
                        "Expecting property name enclosed in double quotes",
                        idx,
                    ));
                }
                let (key, next) = self.scan_string(idx + 1)?;
                idx = self.skip_ws(next);
                if idx >= len || self.at(idx) != ':' {
                    return Err(Fail::Error("Expecting ':' delimiter", idx));
                }
                idx = self.skip_ws(idx + 1);
                let (val, next) = self.scan_once(idx, depth)?;
                map.insert(key, val);
                idx = self.skip_ws(next);
                if idx < len && self.at(idx) == '}' {
                    break;
                }
                if idx >= len || self.at(idx) != ',' {
                    return Err(Fail::Error("Expecting ',' delimiter", idx));
                }
                let comma = idx;
                idx = self.skip_ws(idx + 1);
                if idx < len && self.at(idx) == '}' {
                    return Err(Fail::Error(
                        "Illegal trailing comma before end of object",
                        comma,
                    ));
                }
            }
        }
        Ok((Value::Dict(map), idx + 1))
    }

    fn parse_array(&self, mut idx: usize, depth: usize) -> Result<(Value, usize), Fail> {
        if depth > MAX_DEPTH {
            return Err(Fail::Crash(Crash::new(
                "RecursionError: maximum recursion depth exceeded while decoding a JSON array from a unicode string",
            )));
        }
        let len = self.len();
        let mut items = Vec::new();
        idx = self.skip_ws(idx);
        if idx >= len || self.at(idx) != ']' {
            loop {
                let (val, next) = self.scan_once(idx, depth)?;
                items.push(val);
                idx = self.skip_ws(next);
                if idx < len && self.at(idx) == ']' {
                    break;
                }
                if idx >= len || self.at(idx) != ',' {
                    return Err(Fail::Error("Expecting ',' delimiter", idx));
                }
                let comma = idx;
                idx = self.skip_ws(idx + 1);
                if idx < len && self.at(idx) == ']' {
                    return Err(Fail::Error(
                        "Illegal trailing comma before end of array",
                        comma,
                    ));
                }
            }
        }
        Ok((Value::List(items), idx + 1))
    }

    fn error(&self, msg: &str, pos: usize) -> DecodeError {
        let before = &self.s[..pos.min(self.len())];
        let lineno = before.iter().filter(|&&c| c == '\n').count() + 1;
        let colno = match before.iter().rposition(|&c| c == '\n') {
            Some(nl) => pos - nl,
            None => pos + 1,
        };
        DecodeError {
            msg: msg.to_string(),
            lineno,
            colno,
            pos,
        }
    }
}

/// `json.loads(text)`.
pub fn loads(text: &str) -> Result<Value, LoadError> {
    let sc = Scanner {
        s: text.chars().collect(),
    };
    if sc.s.first() == Some(&'\u{feff}') {
        return Err(LoadError::Decode(
            sc.error("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0),
        ));
    }
    let start = sc.skip_ws(0);
    let (value, end) = match sc.scan_once(start, 0) {
        Ok(v) => v,
        Err(Fail::Stop(i)) => return Err(LoadError::Decode(sc.error("Expecting value", i))),
        Err(Fail::Error(m, i)) => return Err(LoadError::Decode(sc.error(m, i))),
        Err(Fail::Crash(c)) => return Err(LoadError::Crash(c)),
    };
    let end = sc.skip_ws(end);
    if end != sc.len() {
        return Err(LoadError::Decode(sc.error("Extra data", end)));
    }
    Ok(value)
}

/// `json.load(open(path, encoding="utf-8"))`: text mode turns `\r\n` and `\r` into `\n`.
pub fn load_file(path: &std::path::Path) -> Result<Value, LoadError> {
    let text = super::read_text(path)?;
    loads(&text)
}

/// A JSON string literal as `json.dumps(..., ensure_ascii=False)` writes it.
pub fn encode_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn encode_float(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        float_repr(f)
    }
}

/// `json.dumps(value, indent=indent, ensure_ascii=False)`.
pub fn dumps(value: &Value, indent: Option<usize>) -> Result<String, Crash> {
    let mut out = String::new();
    write_value(value, indent, 0, &mut out)?;
    Ok(out)
}

/// `json.dumps(value, indent=indent)`: every character outside printable ASCII is escaped.
pub fn dumps_ascii(value: &Value, indent: Option<usize>) -> Result<String, Crash> {
    let text = dumps(value, indent)?;
    let mut out = String::with_capacity(text.len());
    // Outside strings there are only ASCII characters; inside, control characters are
    // already escaped.
    for c in text.chars() {
        let u = c as u32;
        if (0x20..0x7f).contains(&u) || c == '\n' {
            out.push(c);
        } else if let Some(s) = super::as_surrogate(c) {
            out.push_str(&format!("\\u{s:04x}"));
        } else if u > 0xffff {
            let v = u - 0x10000;
            out.push_str(&format!(
                "\\u{:04x}\\u{:04x}",
                0xd800 + (v >> 10),
                0xdc00 + (v & 0x3ff)
            ));
        } else {
            out.push_str(&format!("\\u{u:04x}"));
        }
    }
    Ok(out)
}

/// `json.dump(value, f, indent=indent, ensure_ascii=False)`: the text written, and the error
/// that stopped it. Python writes as it encodes, so a value that cannot be encoded leaves
/// the text before it in the file.
pub fn dump(value: &Value, indent: Option<usize>) -> (String, Option<Crash>) {
    let mut out = String::new();
    let error = write_value(value, indent, 0, &mut out).err();
    (out, error)
}

/// A dict key as `json.dumps` writes it.
fn encode_key(k: &Value) -> Result<String, Crash> {
    Ok(match k {
        Value::Str(s) => s.clone(),
        Value::Float(f) => encode_float(*f),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Null => "null".into(),
        Value::Int(i) => i.clone(),
        other => {
            return Err(Crash::new(format!(
                "TypeError: keys must be str, int, float, bool or None, not {}",
                other.type_name()
            )));
        }
    })
}

fn write_value(
    v: &Value,
    indent: Option<usize>,
    level: usize,
    out: &mut String,
) -> Result<(), Crash> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(i) => out.push_str(i),
        Value::Float(f) => out.push_str(&encode_float(*f)),
        Value::Str(s) => encode_str(s, out),
        Value::List(items) | Value::Tuple(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                separator(n, indent, level + 1, out);
                write_value(item, indent, level + 1, out)?;
            }
            closing(indent, level, out);
            out.push(']');
        }
        Value::Dict(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            out.push('{');
            for (n, (k, item)) in map.iter().enumerate() {
                // A bad key fails before its separator is written, as in Python.
                let key = encode_key(k)?;
                separator(n, indent, level + 1, out);
                encode_str(&key, out);
                out.push_str(": ");
                write_value(item, indent, level + 1, out)?;
            }
            closing(indent, level, out);
            out.push('}');
        }
        other => {
            return Err(Crash::new(format!(
                "TypeError: Object of type {} is not JSON serializable",
                other.type_name()
            )));
        }
    }
    Ok(())
}

fn separator(n: usize, indent: Option<usize>, level: usize, out: &mut String) {
    match indent {
        Some(width) => {
            if n > 0 {
                out.push(',');
            }
            out.push('\n');
            out.push_str(&" ".repeat(width * level));
        }
        None => {
            if n > 0 {
                out.push_str(", ");
            }
        }
    }
}

fn closing(indent: Option<usize>, level: usize, out: &mut String) {
    if let Some(width) = indent {
        out.push('\n');
        out.push_str(&" ".repeat(width * level));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> String {
        match loads(text) {
            Err(LoadError::Decode(e)) => e.to_string(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn round_trip_keeps_order_and_text() {
        let v = loads("{\"b\": [1, 2.50, -0, 1e400, \"é\\n\"], \"a\": {}, \"b\": null}").unwrap();
        assert_eq!(dumps(&v, None).unwrap(), "{\"b\": null, \"a\": {}}");
        let v = loads("[1, 2.50, -0, 1e400, \"é\\n\", [], {\"x\": true}]").unwrap();
        assert_eq!(
            dumps(&v, Some(2)).unwrap(),
            "[\n  1,\n  2.5,\n  0,\n  Infinity,\n  \"é\\n\",\n  [],\n  {\n    \"x\": true\n  }\n]"
        );
    }

    #[test]
    fn error_messages_match_python() {
        assert_eq!(err(""), "Expecting value: line 1 column 1 (char 0)");
        assert_eq!(
            err("{\n  \"a\" 1}"),
            "Expecting ':' delimiter: line 2 column 7 (char 8)"
        );
        assert_eq!(
            err("{\"a\": 1,}"),
            "Illegal trailing comma before end of object: line 1 column 8 (char 7)"
        );
        assert_eq!(err("[1] x"), "Extra data: line 1 column 5 (char 4)");
        assert_eq!(
            err("\"abc"),
            "Unterminated string starting at: line 1 column 1 (char 0)"
        );
        assert_eq!(err("\"\\x\""), "Invalid \\escape: line 1 column 2 (char 1)");
    }
}
