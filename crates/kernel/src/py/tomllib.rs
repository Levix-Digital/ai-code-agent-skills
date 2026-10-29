//! `tomllib.loads` of Python 3.13, translated function by function, so that documents load to
//! the same values and fail with the same `TOMLDecodeError` messages.
//!
//! A local time (`07:32:00`) has no `Value` of its own: it loads as a tuple of its hour, minute,
//! second and microsecond, which no caller reads.

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;

use super::num;
use super::value::{Date, DateTime};
use super::{Crash, Dict, Value, repr_str};

type Key = Vec<String>;

const FROZEN: u8 = 0;
const EXPLICIT_NEST: u8 = 1;

fn is_ascii_ctrl(c: char) -> bool {
    (c as u32) < 32 || c as u32 == 127
}

fn illegal_basic(c: char) -> bool {
    is_ascii_ctrl(c) && c != '\t'
}

fn illegal_multiline(c: char) -> bool {
    is_ascii_ctrl(c) && c != '\t' && c != '\n'
}

fn is_ws(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_ws_or_nl(c: char) -> bool {
    is_ws(c) || c == '\n'
}

fn is_bare_key(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

const TIME_RE: &str = r"([01][0-9]|2[0-3]):([0-5][0-9]):([0-5][0-9])(?:\.([0-9]{1,6})[0-9]*)?";

static RE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:0(?:x[0-9A-Fa-f](?:_?[0-9A-Fa-f])*|b[01](?:_?[01])*|o[0-7](?:_?[0-7])*)",
        r"|[+-]?(?:0|[1-9](?:_?[0-9])*)(?P<floatpart>(?:\.[0-9](?:_?[0-9])*)?",
        r"(?:[eE][+-]?[0-9](?:_?[0-9])*)?))"
    ))
    .expect("valid pattern")
});
static RE_LOCALTIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{TIME_RE}")).expect("valid pattern"));
static RE_DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^([0-9]{{4}})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])\
         (?:[Tt ]{TIME_RE}(?:([Zz])|([+-])([01][0-9]|2[0-3]):([0-5][0-9]))?)?"
    ))
    .expect("valid pattern")
});

enum Fail {
    Decode(String),
    Crash(Crash),
}

type Res<T> = Result<T, Fail>;

/// `repr(key)` of a key tuple.
fn key_repr(key: &[String]) -> String {
    let parts: Vec<String> = key.iter().map(|k| repr_str(k)).collect();
    if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    }
}

#[derive(Default)]
struct FlagNode {
    flags: BTreeSet<u8>,
    recursive_flags: BTreeSet<u8>,
    nested: HashMap<String, FlagNode>,
}

#[derive(Default)]
struct Flags {
    root: HashMap<String, FlagNode>,
    pending: Vec<(Key, u8)>,
}

impl Flags {
    fn add_pending(&mut self, key: Key, flag: u8) {
        if !self.pending.iter().any(|(k, f)| *k == key && *f == flag) {
            self.pending.push((key, flag));
        }
    }

    fn finalize_pending(&mut self) {
        for (key, flag) in std::mem::take(&mut self.pending) {
            self.set(&key, flag, false);
        }
    }

    fn unset_all(&mut self, key: &[String]) {
        let mut cont = &mut self.root;
        for k in &key[..key.len() - 1] {
            match cont.get_mut(k) {
                Some(node) => cont = &mut node.nested,
                None => return,
            }
        }
        cont.remove(&key[key.len() - 1]);
    }

    fn set(&mut self, key: &[String], flag: u8, recursive: bool) {
        let mut cont = &mut self.root;
        for k in &key[..key.len() - 1] {
            cont = &mut cont.entry(k.clone()).or_default().nested;
        }
        let node = cont.entry(key[key.len() - 1].clone()).or_default();
        if recursive {
            node.recursive_flags.insert(flag);
        } else {
            node.flags.insert(flag);
        }
    }

    fn is(&self, key: &[String], flag: u8) -> bool {
        let Some((stem, parent)) = key.split_last() else {
            return false;
        };
        let mut cont = &self.root;
        for k in parent {
            let Some(inner) = cont.get(k) else {
                return false;
            };
            if inner.recursive_flags.contains(&flag) {
                return true;
            }
            cont = &inner.nested;
        }
        cont.get(stem)
            .is_some_and(|n| n.flags.contains(&flag) || n.recursive_flags.contains(&flag))
    }
}

/// `KeyError` from `NestedDict`, or the `IndexError` of an empty list.
enum NestError {
    Key,
    Index,
}

fn get_or_create_nest<'a>(
    root: &'a mut Dict,
    key: &[String],
    access_lists: bool,
) -> Result<&'a mut Dict, NestError> {
    let mut cont = root;
    for k in key {
        if cont.get(k).is_none() {
            cont.insert(k.clone(), Value::Dict(Dict::new()));
        }
        let mut value = cont.get_mut(k).expect("just inserted");
        if access_lists && let Value::List(items) = value {
            value = items.last_mut().ok_or(NestError::Index)?;
        }
        match value {
            Value::Dict(d) => cont = d,
            _ => return Err(NestError::Key),
        }
    }
    Ok(cont)
}

fn append_nest_to_list(root: &mut Dict, key: &[String]) -> Result<(), NestError> {
    let (last, parent) = key.split_last().expect("a key has parts");
    let cont = get_or_create_nest(root, parent, true)?;
    match cont.get_mut(last) {
        Some(Value::List(items)) => items.push(Value::Dict(Dict::new())),
        Some(_) => return Err(NestError::Key),
        None => cont.insert(last.clone(), Value::List(vec![Value::Dict(Dict::new())])),
    }
    Ok(())
}

struct Parser {
    src: Vec<char>,
}

impl Parser {
    fn at(&self, pos: usize) -> Option<char> {
        self.src.get(pos).copied()
    }

    fn starts_with(&self, pos: usize, s: &str) -> bool {
        (pos..).zip(s.chars()).all(|(p, c)| self.at(p) == Some(c))
    }

    fn slice(&self, a: usize, b: usize) -> String {
        let b = b.min(self.src.len());
        if a >= b {
            return String::new();
        }
        self.src[a..b].iter().collect()
    }

    fn err(&self, pos: usize, msg: &str) -> Fail {
        let coord = if pos >= self.src.len() {
            "end of document".to_string()
        } else {
            let line = self.src[..pos].iter().filter(|&&c| c == '\n').count() + 1;
            let column = if line == 1 {
                pos + 1
            } else {
                pos - self.src[..pos]
                    .iter()
                    .rposition(|&c| c == '\n')
                    .expect("a newline")
            };
            format!("line {line}, column {column}")
        };
        Fail::Decode(format!("{msg} (at {coord})"))
    }

    fn skip_chars(&self, mut pos: usize, pred: fn(char) -> bool) -> usize {
        while self.at(pos).is_some_and(pred) {
            pos += 1;
        }
        pos
    }

    fn skip_until(
        &self,
        mut pos: usize,
        expect: &str,
        error_on: fn(char) -> bool,
        error_on_eof: bool,
    ) -> Res<usize> {
        let first = expect.chars().next().expect("non-empty");
        let found =
            (pos..self.src.len()).find(|&p| self.src[p] == first && self.starts_with(p, expect));
        let new_pos = match found {
            Some(p) => p,
            None => {
                if error_on_eof {
                    return Err(self.err(self.src.len(), &format!("Expected {}", repr_str(expect))));
                }
                self.src.len()
            }
        };
        if self.src[pos..new_pos].iter().any(|&c| error_on(c)) {
            while !error_on(self.src[pos]) {
                pos += 1;
            }
            return Err(self.err(
                pos,
                &format!(
                    "Found invalid character {}",
                    repr_str(&self.src[pos].to_string())
                ),
            ));
        }
        Ok(new_pos)
    }

    fn skip_comment(&self, pos: usize) -> Res<usize> {
        if self.at(pos) == Some('#') {
            return self.skip_until(pos + 1, "\n", illegal_basic, false);
        }
        Ok(pos)
    }

    fn skip_comments_and_array_ws(&self, mut pos: usize) -> Res<usize> {
        loop {
            let before = pos;
            pos = self.skip_chars(pos, is_ws_or_nl);
            pos = self.skip_comment(pos)?;
            if pos == before {
                return Ok(pos);
            }
        }
    }

    fn create_dict_rule(
        &self,
        mut pos: usize,
        data: &mut Dict,
        flags: &mut Flags,
    ) -> Res<(usize, Key)> {
        pos += 1;
        pos = self.skip_chars(pos, is_ws);
        let (p, key) = self.parse_key(pos)?;
        pos = p;
        if flags.is(&key, EXPLICIT_NEST) || flags.is(&key, FROZEN) {
            return Err(self.err(pos, &format!("Cannot declare {} twice", key_repr(&key))));
        }
        flags.set(&key, EXPLICIT_NEST, false);
        match get_or_create_nest(data, &key, true) {
            Ok(_) => {}
            Err(NestError::Key) => return Err(self.err(pos, "Cannot overwrite a value")),
            Err(NestError::Index) => return Err(index_error()),
        }
        if !self.starts_with(pos, "]") {
            return Err(self.err(pos, "Expected ']' at the end of a table declaration"));
        }
        Ok((pos + 1, key))
    }

    fn create_list_rule(
        &self,
        mut pos: usize,
        data: &mut Dict,
        flags: &mut Flags,
    ) -> Res<(usize, Key)> {
        pos += 2;
        pos = self.skip_chars(pos, is_ws);
        let (p, key) = self.parse_key(pos)?;
        pos = p;
        if flags.is(&key, FROZEN) {
            return Err(self.err(
                pos,
                &format!("Cannot mutate immutable namespace {}", key_repr(&key)),
            ));
        }
        flags.unset_all(&key);
        flags.set(&key, EXPLICIT_NEST, false);
        match append_nest_to_list(data, &key) {
            Ok(()) => {}
            Err(NestError::Key) => return Err(self.err(pos, "Cannot overwrite a value")),
            Err(NestError::Index) => return Err(index_error()),
        }
        if !self.starts_with(pos, "]]") {
            return Err(self.err(pos, "Expected ']]' at the end of an array declaration"));
        }
        Ok((pos + 2, key))
    }

    fn key_value_rule(
        &self,
        pos: usize,
        data: &mut Dict,
        flags: &mut Flags,
        header: &[String],
    ) -> Res<usize> {
        let (pos, key, value) = self.parse_key_value_pair(pos)?;
        let (stem, parent) = key.split_last().expect("a key has parts");
        let mut abs_parent: Key = header.to_vec();
        abs_parent.extend_from_slice(parent);
        for i in 1..key.len() {
            let mut cont_key: Key = header.to_vec();
            cont_key.extend_from_slice(&key[..i]);
            if flags.is(&cont_key, EXPLICIT_NEST) {
                return Err(self.err(
                    pos,
                    &format!("Cannot redefine namespace {}", key_repr(&cont_key)),
                ));
            }
            flags.add_pending(cont_key, EXPLICIT_NEST);
        }
        if flags.is(&abs_parent, FROZEN) {
            return Err(self.err(
                pos,
                &format!(
                    "Cannot mutate immutable namespace {}",
                    key_repr(&abs_parent)
                ),
            ));
        }
        let nest = match get_or_create_nest(data, &abs_parent, true) {
            Ok(n) => n,
            Err(NestError::Key) => return Err(self.err(pos, "Cannot overwrite a value")),
            Err(NestError::Index) => return Err(index_error()),
        };
        if nest.get(stem).is_some() {
            return Err(self.err(pos, "Cannot overwrite a value"));
        }
        if matches!(value, Value::Dict(_) | Value::List(_)) {
            let mut full: Key = header.to_vec();
            full.extend_from_slice(&key);
            flags.set(&full, FROZEN, true);
        }
        nest.insert(stem.clone(), value);
        Ok(pos)
    }

    fn parse_key_value_pair(&self, pos: usize) -> Res<(usize, Key, Value)> {
        let (mut pos, key) = self.parse_key(pos)?;
        if self.at(pos) != Some('=') {
            return Err(self.err(pos, "Expected '=' after a key in a key/value pair"));
        }
        pos += 1;
        pos = self.skip_chars(pos, is_ws);
        let (pos, value) = self.parse_value(pos)?;
        Ok((pos, key, value))
    }

    fn parse_key(&self, pos: usize) -> Res<(usize, Key)> {
        let (mut pos, part) = self.parse_key_part(pos)?;
        let mut key = vec![part];
        pos = self.skip_chars(pos, is_ws);
        loop {
            if self.at(pos) != Some('.') {
                return Ok((pos, key));
            }
            pos += 1;
            pos = self.skip_chars(pos, is_ws);
            let (p, part) = self.parse_key_part(pos)?;
            pos = p;
            key.push(part);
            if key.len() > 1000 {
                return Err(Fail::Crash(Crash::new(
                    "RecursionError: TOML key has more than the allowed 1000 parts",
                )));
            }
            pos = self.skip_chars(pos, is_ws);
        }
    }

    fn parse_key_part(&self, pos: usize) -> Res<(usize, String)> {
        match self.at(pos) {
            Some(c) if is_bare_key(c) => {
                let end = self.skip_chars(pos, is_bare_key);
                Ok((end, self.slice(pos, end)))
            }
            Some('\'') => self.parse_literal_str(pos),
            Some('"') => self.parse_basic_str(pos + 1, false),
            _ => Err(self.err(pos, "Invalid initial character for a key part")),
        }
    }

    fn parse_array(&self, mut pos: usize) -> Res<(usize, Value)> {
        pos += 1;
        let mut array = Vec::new();
        pos = self.skip_comments_and_array_ws(pos)?;
        if self.starts_with(pos, "]") {
            return Ok((pos + 1, Value::List(array)));
        }
        loop {
            let (p, val) = self.parse_value(pos)?;
            array.push(val);
            pos = self.skip_comments_and_array_ws(p)?;
            match self.at(pos) {
                Some(']') => return Ok((pos + 1, Value::List(array))),
                Some(',') => {}
                _ => return Err(self.err(pos, "Unclosed array")),
            }
            pos += 1;
            pos = self.skip_comments_and_array_ws(pos)?;
            if self.starts_with(pos, "]") {
                return Ok((pos + 1, Value::List(array)));
            }
        }
    }

    fn parse_inline_table(&self, mut pos: usize) -> Res<(usize, Value)> {
        pos += 1;
        let mut nested = Dict::new();
        let mut flags = Flags::default();
        pos = self.skip_chars(pos, is_ws);
        if self.starts_with(pos, "}") {
            return Ok((pos + 1, Value::Dict(nested)));
        }
        loop {
            let (p, key, value) = self.parse_key_value_pair(pos)?;
            pos = p;
            let (stem, parent) = key.split_last().expect("a key has parts");
            if flags.is(&key, FROZEN) {
                return Err(self.err(
                    pos,
                    &format!("Cannot mutate immutable namespace {}", key_repr(&key)),
                ));
            }
            let nest = match get_or_create_nest(&mut nested, parent, false) {
                Ok(n) => n,
                Err(_) => return Err(self.err(pos, "Cannot overwrite a value")),
            };
            if nest.get(stem).is_some() {
                return Err(self.err(
                    pos,
                    &format!("Duplicate inline table key {}", repr_str(stem)),
                ));
            }
            let is_container = matches!(value, Value::Dict(_) | Value::List(_));
            nest.insert(stem.clone(), value);
            pos = self.skip_chars(pos, is_ws);
            match self.at(pos) {
                Some('}') => return Ok((pos + 1, Value::Dict(nested))),
                Some(',') => {}
                _ => return Err(self.err(pos, "Unclosed inline table")),
            }
            if is_container {
                flags.set(&key, FROZEN, true);
            }
            pos += 1;
            pos = self.skip_chars(pos, is_ws);
        }
    }

    fn parse_basic_str_escape(&self, mut pos: usize, multiline: bool) -> Res<(usize, String)> {
        let escape_id = self.slice(pos, pos + 2);
        pos += 2;
        if multiline && matches!(escape_id.as_str(), "\\ " | "\\\t" | "\\\n") {
            if escape_id != "\\\n" {
                pos = self.skip_chars(pos, is_ws);
                match self.at(pos) {
                    None => return Ok((pos, String::new())),
                    Some('\n') => {}
                    Some(_) => return Err(self.err(pos, "Unescaped '\\' in a string")),
                }
                pos += 1;
            }
            pos = self.skip_chars(pos, is_ws_or_nl);
            return Ok((pos, String::new()));
        }
        match escape_id.as_str() {
            "\\u" => return self.parse_hex_char(pos, 4),
            "\\U" => return self.parse_hex_char(pos, 8),
            _ => {}
        }
        let replacement = match escape_id.as_str() {
            "\\b" => "\u{8}",
            "\\t" => "\t",
            "\\n" => "\n",
            "\\f" => "\u{c}",
            "\\r" => "\r",
            "\\\"" => "\"",
            "\\\\" => "\\",
            _ => return Err(self.err(pos, "Unescaped '\\' in a string")),
        };
        Ok((pos, replacement.to_string()))
    }

    fn parse_hex_char(&self, mut pos: usize, hex_len: usize) -> Res<(usize, String)> {
        let hex = self.slice(pos, pos + hex_len);
        if hex.chars().count() != hex_len || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.err(pos, "Invalid hex value"));
        }
        pos += hex_len;
        let code = u32::from_str_radix(&hex, 16).expect("hex digits");
        match char::from_u32(code) {
            Some(c) => Ok((pos, c.to_string())),
            None => Err(self.err(pos, "Escaped character is not a Unicode scalar value")),
        }
    }

    fn parse_literal_str(&self, pos: usize) -> Res<(usize, String)> {
        let start = pos + 1;
        let end = self.skip_until(start, "'", illegal_basic, true)?;
        Ok((end + 1, self.slice(start, end)))
    }

    fn parse_multiline_str(&self, mut pos: usize, literal: bool) -> Res<(usize, String)> {
        pos += 3;
        if self.starts_with(pos, "\n") {
            pos += 1;
        }
        let (delim, mut result) = if literal {
            let end = self.skip_until(pos, "'''", illegal_multiline, true)?;
            let result = self.slice(pos, end);
            pos = end + 3;
            ("'", result)
        } else {
            let (p, result) = self.parse_basic_str(pos, true)?;
            pos = p;
            ("\"", result)
        };
        if !self.starts_with(pos, delim) {
            return Ok((pos, result));
        }
        pos += 1;
        if !self.starts_with(pos, delim) {
            result.push_str(delim);
            return Ok((pos, result));
        }
        pos += 1;
        result.push_str(delim);
        result.push_str(delim);
        Ok((pos, result))
    }

    fn parse_basic_str(&self, mut pos: usize, multiline: bool) -> Res<(usize, String)> {
        let error_on: fn(char) -> bool = if multiline {
            illegal_multiline
        } else {
            illegal_basic
        };
        let mut result = String::new();
        let mut start = pos;
        loop {
            let Some(c) = self.at(pos) else {
                return Err(self.err(pos, "Unterminated string"));
            };
            if c == '"' {
                if !multiline {
                    result.push_str(&self.slice(start, pos));
                    return Ok((pos + 1, result));
                }
                if self.starts_with(pos, "\"\"\"") {
                    result.push_str(&self.slice(start, pos));
                    return Ok((pos + 3, result));
                }
                pos += 1;
                continue;
            }
            if c == '\\' {
                result.push_str(&self.slice(start, pos));
                let (p, escaped) = self.parse_basic_str_escape(pos, multiline)?;
                result.push_str(&escaped);
                pos = p;
                start = pos;
                continue;
            }
            if error_on(c) {
                return Err(self.err(
                    pos,
                    &format!("Illegal character {}", repr_str(&c.to_string())),
                ));
            }
            pos += 1;
        }
    }

    /// The text from `pos` to the end.
    fn rest(&self, pos: usize) -> String {
        self.slice(pos, self.src.len())
    }

    fn parse_value(&self, pos: usize) -> Res<(usize, Value)> {
        let c = self.at(pos);
        match c {
            Some('"') => {
                if self.starts_with(pos, "\"\"\"") {
                    return self
                        .parse_multiline_str(pos, false)
                        .map(|(p, s)| (p, Value::Str(s)));
                }
                return self
                    .parse_basic_str(pos + 1, false)
                    .map(|(p, s)| (p, Value::Str(s)));
            }
            Some('\'') => {
                if self.starts_with(pos, "'''") {
                    return self
                        .parse_multiline_str(pos, true)
                        .map(|(p, s)| (p, Value::Str(s)));
                }
                return self.parse_literal_str(pos).map(|(p, s)| (p, Value::Str(s)));
            }
            Some('t') if self.starts_with(pos, "true") => return Ok((pos + 4, Value::Bool(true))),
            Some('f') if self.starts_with(pos, "false") => {
                return Ok((pos + 5, Value::Bool(false)));
            }
            Some('[') => return self.parse_array(pos),
            Some('{') => return self.parse_inline_table(pos),
            _ => {}
        }
        let rest = self.rest(pos);
        let chars_in = |bytes: usize| rest[..bytes].chars().count();
        if let Some(m) = RE_DATETIME.captures(&rest) {
            let end = pos + chars_in(m.get(0).expect("match").end());
            return match datetime_value(&m) {
                Some(v) => Ok((end, v)),
                None => Err(self.err(pos, "Invalid date or datetime")),
            };
        }
        if let Some(m) = RE_LOCALTIME.captures(&rest) {
            let end = pos + chars_in(m.get(0).expect("match").end());
            let part = |i: usize| {
                m.get(i)
                    .map_or(0, |g| g.as_str().parse::<i64>().unwrap_or(0))
            };
            let micros = m.get(4).map_or(0, |g| micros(g.as_str()));
            return Ok((
                end,
                Value::Tuple(vec![
                    Value::int(part(1)),
                    Value::int(part(2)),
                    Value::int(part(3)),
                    Value::int(micros),
                ]),
            ));
        }
        if let Some(m) = RE_NUMBER.captures(&rest) {
            let text = m.get(0).expect("match").as_str();
            let end = pos + chars_in(text.len());
            let floatpart = m.name("floatpart").is_some_and(|g| !g.as_str().is_empty());
            return number_value(text, floatpart)
                .map(|v| (end, v))
                .map_err(Fail::Crash);
        }
        let first_three = self.slice(pos, pos + 3);
        if first_three == "inf" || first_three == "nan" {
            let f = num::float(&first_three).map_err(Fail::Crash)?;
            return Ok((pos + 3, Value::Float(f)));
        }
        let first_four = self.slice(pos, pos + 4);
        if matches!(first_four.as_str(), "-inf" | "+inf" | "-nan" | "+nan") {
            let f = num::float(&first_four).map_err(Fail::Crash)?;
            return Ok((pos + 4, Value::Float(f)));
        }
        Err(self.err(pos, "Invalid value"))
    }
}

fn index_error() -> Fail {
    Fail::Crash(Crash::new("IndexError: list index out of range"))
}

/// `int(micros_str.ljust(6, "0"))`.
fn micros(s: &str) -> i64 {
    format!("{s:0<6}").parse().unwrap_or(0)
}

/// `match_to_datetime`: `None` for a date that does not exist.
fn datetime_value(m: &regex::Captures) -> Option<Value> {
    let g = |i: usize| m.get(i).map(|x| x.as_str());
    let year: i32 = g(1)?.parse().ok()?;
    let month: u32 = g(2)?.parse().ok()?;
    let day: u32 = g(3)?.parse().ok()?;
    if year < 1 || day > Date::days_in_month(year, month) {
        return None;
    }
    let date = Date { year, month, day };
    let Some(hour) = g(4) else {
        return Some(Value::Date(date));
    };
    let offset = match (g(9), g(8)) {
        (Some(sign), _) => {
            let minutes = g(10)?.parse::<i32>().ok()? * 60 + g(11)?.parse::<i32>().ok()?;
            Some(if sign == "+" { minutes } else { -minutes } * 60)
        }
        (None, Some(_)) => Some(0),
        (None, None) => None,
    };
    Some(Value::DateTime(DateTime {
        date,
        hour: hour.parse().ok()?,
        minute: g(5)?.parse().ok()?,
        second: g(6)?.parse().ok()?,
        microsecond: g(7).map_or(0, |s| micros(s) as u32),
        offset,
    }))
}

/// `match_to_number`.
fn number_value(text: &str, floatpart: bool) -> Result<Value, Crash> {
    if floatpart {
        return num::float(text).map(Value::Float);
    }
    let (base, digits) = match text.get(..2) {
        Some("0x") => (16, &text[2..]),
        Some("0b") => (2, &text[2..]),
        Some("0o") => (8, &text[2..]),
        _ => (10, text),
    };
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    Ok(Value::Int(num::int(&digits, base)?.to_string()))
}

/// `tomllib.loads(s)`.
pub fn loads(s: &str) -> Result<Value, Crash> {
    let src: Vec<char> = s.replace("\r\n", "\n").chars().collect();
    let parser = Parser { src };
    let mut data = Dict::new();
    let mut flags = Flags::default();
    match parse(&parser, &mut data, &mut flags) {
        Ok(()) => Ok(Value::Dict(data)),
        Err(Fail::Decode(msg)) => Err(Crash::new(format!("tomllib.TOMLDecodeError: {msg}"))),
        Err(Fail::Crash(c)) => Err(c),
    }
}

fn parse(p: &Parser, data: &mut Dict, flags: &mut Flags) -> Res<()> {
    let mut pos = 0;
    let mut header: Key = Vec::new();
    loop {
        pos = p.skip_chars(pos, is_ws);
        let Some(c) = p.at(pos) else { break };
        if c == '\n' {
            pos += 1;
            continue;
        }
        if is_bare_key(c) || c == '"' || c == '\'' {
            pos = p.key_value_rule(pos, data, flags, &header)?;
            pos = p.skip_chars(pos, is_ws);
        } else if c == '[' {
            let second = p.at(pos + 1);
            flags.finalize_pending();
            let (np, key) = if second == Some('[') {
                p.create_list_rule(pos, data, flags)?
            } else {
                p.create_dict_rule(pos, data, flags)?
            };
            pos = p.skip_chars(np, is_ws);
            header = key;
        } else if c != '#' {
            return Err(p.err(pos, "Invalid statement"));
        }
        pos = p.skip_comment(pos)?;
        let Some(c) = p.at(pos) else { break };
        if c != '\n' {
            return Err(p.err(pos, "Expected newline or end of document after a statement"));
        }
        pos += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(s: &str) -> String {
        loads(s).unwrap_err().0
    }

    #[test]
    fn errors_like_python() {
        let e = |m: &str| format!("tomllib.TOMLDecodeError: {m}");
        assert_eq!(
            err("a=[]\n[a.b]"),
            e("Cannot declare ('a', 'b') twice (at line 2, column 5)")
        );
        assert_eq!(
            err("a=1\na=2"),
            e("Cannot overwrite a value (at end of document)")
        );
        assert_eq!(
            err("[t]\n[t]"),
            e("Cannot declare ('t',) twice (at line 2, column 3)")
        );
        assert_eq!(
            err("x = 2021-02-30"),
            e("Invalid date or datetime (at line 1, column 5)")
        );
        assert_eq!(
            err("x=\"\\q\""),
            e("Unescaped '\\' in a string (at line 1, column 6)")
        );
        assert_eq!(
            err("a.b=1\n[a]"),
            e("Cannot declare ('a',) twice (at line 2, column 3)")
        );
        assert_eq!(
            err("x=1\n[[x]]"),
            e("Cannot overwrite a value (at line 2, column 4)")
        );
        assert_eq!(
            err("x = {a=1, a=2}"),
            e("Duplicate inline table key 'a' (at line 1, column 14)")
        );
        assert_eq!(
            err("k=\"\x01\""),
            e("Illegal character '\\x01' (at line 1, column 4)")
        );
        assert_eq!(
            err("# \x01"),
            e("Found invalid character '\\x01' (at line 1, column 3)")
        );
        assert_eq!(
            err("k=\"abc"),
            e("Unterminated string (at end of document)")
        );
        assert_eq!(
            err("[a"),
            e("Expected ']' at the end of a table declaration (at end of document)")
        );
        assert_eq!(
            err("x=1 y"),
            e("Expected newline or end of document after a statement (at line 1, column 5)")
        );
    }

    #[test]
    fn values_like_python() {
        let v = loads(
            "[package]\nname = \"a\"\nversion = \"1.2.3\"\n[workspace.package]\nversion = '2'\n",
        )
        .unwrap();
        let Value::Dict(d) = v else { panic!() };
        let Some(Value::Dict(p)) = d.get("package") else {
            panic!()
        };
        assert_eq!(p.get("version").and_then(Value::as_str), Some("1.2.3"));
        assert!(loads("a = 0x1_F\nb = 1e3\nc = 1979-05-27T07:32:00Z\nd = 07:32:00\ne = [1, [2]]\nf = {g.h = 1}\n").is_ok());
    }
}
