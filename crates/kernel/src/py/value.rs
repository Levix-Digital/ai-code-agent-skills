//! A value as Python holds it after `json.load` or `yaml.safe_load`: `None`, `bool`, `int`,
//! `float`, `str`, `bytes`, `list`, `tuple`, `set`, `dict` (insertion-ordered, any hashable
//! key), `datetime.date` and `datetime.datetime`, with Python's equality, truthiness, `str()`
//! and `repr()`.

use super::{float_repr, repr_str};

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    /// Python ints have no size limit: kept as their decimal text.
    Int(String),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    /// Insertion order kept, as the scripts never sort a set they print.
    Set(Vec<Value>),
    Dict(Dict),
    Date(Date),
    DateTime(DateTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub date: Date,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub microsecond: u32,
    /// UTC offset in seconds; `None` for a naive datetime. A zero offset is `timezone.utc`.
    pub offset: Option<i32>,
}

impl Date {
    /// `date.isoformat()`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
            _ => 28,
        }
    }
}

impl DateTime {
    fn offset_text(&self) -> String {
        match self.offset {
            None => String::new(),
            Some(off) => {
                let sign = if off < 0 { '-' } else { '+' };
                let a = off.abs();
                let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
                if s != 0 {
                    format!("{sign}{h:02}:{m:02}:{s:02}")
                } else {
                    format!("{sign}{h:02}:{m:02}")
                }
            }
        }
    }

    /// `datetime.isoformat(sep)`.
    pub fn iso(&self, sep: char) -> String {
        let mut s = format!(
            "{}{sep}{:02}:{:02}:{:02}",
            self.date.iso(),
            self.hour,
            self.minute,
            self.second
        );
        if self.microsecond != 0 {
            s.push_str(&format!(".{:06}", self.microsecond));
        }
        s.push_str(&self.offset_text());
        s
    }
}

pub static NONE: Value = Value::Null;
pub static EMPTY: Dict = Dict::new();

/// A number as Python compares it: `True == 1 == 1.0`.
enum Num<'a> {
    Int(&'a str),
    Float(f64),
}

impl Value {
    /// `int(text)` for a JSON integer literal (`-0` is `0`).
    pub fn int_from_text(text: &str) -> Value {
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.bytes().all(|b| b == b'0') {
            Value::Int("0".into())
        } else {
            Value::Int(text.to_string())
        }
    }

    pub fn int(i: i64) -> Value {
        Value::Int(i.to_string())
    }

    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn list_of_str(items: &[&str]) -> Value {
        Value::List(items.iter().map(|s| Value::str(*s)).collect())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&Dict> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// The value of an `int` that fits in an `i64`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => i.parse().ok(),
            Value::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    pub fn is_dict(&self) -> bool {
        matches!(self, Value::Dict(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Value::List(_))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// `isinstance(value, int)`: `bool` is an `int` too.
    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Bool(_))
    }

    /// `bool(value)`.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => i != "0",
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Bytes(b) => !b.is_empty(),
            Value::List(l) | Value::Tuple(l) | Value::Set(l) => !l.is_empty(),
            Value::Dict(d) => !d.is_empty(),
            Value::Date(_) | Value::DateTime(_) => true,
        }
    }

    /// `value in [<strings>]`.
    pub fn is_one_of(&self, options: &[&str]) -> bool {
        self.as_str().is_some_and(|s| options.contains(&s))
    }

    /// `hash(value)` would succeed.
    pub fn is_hashable(&self) -> bool {
        match self {
            Value::List(_) | Value::Dict(_) | Value::Set(_) => false,
            Value::Tuple(items) => items.iter().all(Value::is_hashable),
            _ => true,
        }
    }

    fn num(&self) -> Option<Num<'_>> {
        match self {
            Value::Bool(true) => Some(Num::Int("1")),
            Value::Bool(false) => Some(Num::Int("0")),
            Value::Int(i) => Some(Num::Int(i)),
            Value::Float(f) => Some(Num::Float(*f)),
            _ => None,
        }
    }

    /// `a == b`.
    pub fn py_eq(&self, other: &Value) -> bool {
        if let (Some(a), Some(b)) = (self.num(), other.num()) {
            return match (a, b) {
                (Num::Int(x), Num::Int(y)) => x == y,
                (Num::Float(x), Num::Float(y)) => x == y,
                (Num::Int(i), Num::Float(f)) | (Num::Float(f), Num::Int(i)) => {
                    f.is_finite() && f.fract() == 0.0 && int_eq_float(i, f)
                }
            };
        }
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.py_eq(y))
            }
            (Value::Set(a), Value::Set(b)) => {
                a.len() == b.len() && a.iter().all(|x| b.iter().any(|y| x.py_eq(y)))
            }
            (Value::Dict(a), Value::Dict(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get_value(k).is_some_and(|w| v.py_eq(w)))
            }
            (Value::Date(a), Value::Date(b)) => a == b,
            (Value::DateTime(a), Value::DateTime(b)) => match (a.offset, b.offset) {
                (None, None) => a == b,
                (Some(_), Some(_)) => {
                    a.utc_seconds() == b.utc_seconds() && a.microsecond == b.microsecond
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Python's type name, as error messages print it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "NoneType",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Bytes(_) => "bytes",
            Value::List(_) => "list",
            Value::Tuple(_) => "tuple",
            Value::Set(_) => "set",
            Value::Dict(_) => "dict",
            Value::Date(_) => "date",
            Value::DateTime(_) => "datetime",
        }
    }

    /// `str(value)`, as an f-string prints it.
    pub fn py_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Date(d) => d.iso(),
            Value::DateTime(d) => d.iso(' '),
            other => other.py_repr(),
        }
    }

    /// `repr(value)`.
    pub fn py_repr(&self) -> String {
        match self {
            Value::Null => "None".into(),
            Value::Bool(true) => "True".into(),
            Value::Bool(false) => "False".into(),
            Value::Int(i) => i.clone(),
            Value::Float(f) => float_repr(*f),
            Value::Str(s) => repr_str(s),
            Value::Bytes(b) => repr_bytes(b),
            Value::List(items) => format!("[{}]", reprs(items)),
            Value::Tuple(items) if items.len() == 1 => format!("({},)", items[0].py_repr()),
            Value::Tuple(items) => format!("({})", reprs(items)),
            Value::Set(items) if items.is_empty() => "set()".into(),
            Value::Set(items) => format!("{{{}}}", reprs(items)),
            Value::Dict(d) => {
                let parts: Vec<String> = d
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.py_repr(), v.py_repr()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::Date(d) => format!("datetime.date({}, {}, {})", d.year, d.month, d.day),
            Value::DateTime(d) => {
                let mut parts = vec![
                    d.date.year.to_string(),
                    d.date.month.to_string(),
                    d.date.day.to_string(),
                    d.hour.to_string(),
                    d.minute.to_string(),
                ];
                if d.second != 0 || d.microsecond != 0 {
                    parts.push(d.second.to_string());
                }
                if d.microsecond != 0 {
                    parts.push(d.microsecond.to_string());
                }
                let mut s = format!("datetime.datetime({}", parts.join(", "));
                if let Some(off) = d.offset {
                    if off == 0 {
                        s.push_str(", tzinfo=datetime.timezone.utc");
                    } else {
                        s.push_str(&format!(
                            ", tzinfo=datetime.timezone({})",
                            timedelta_repr(off)
                        ));
                    }
                }
                s.push(')');
                s
            }
        }
    }
}

impl DateTime {
    fn utc_seconds(&self) -> i64 {
        let days = days_from_civil(self.date.year, self.date.month, self.date.day);
        days * 86400 + (self.hour * 3600 + self.minute * 60 + self.second) as i64
            - self.offset.unwrap_or(0) as i64
    }
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `repr(datetime.timedelta(seconds=...))`.
pub(crate) fn timedelta_repr(seconds: i32) -> String {
    if seconds == 0 {
        return "datetime.timedelta(0)".into();
    }
    let days = seconds.div_euclid(86400);
    let secs = seconds.rem_euclid(86400);
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("days={days}"));
    }
    if secs != 0 {
        parts.push(format!("seconds={secs}"));
    }
    format!("datetime.timedelta({})", parts.join(", "))
}

fn reprs(items: &[Value]) -> String {
    items
        .iter()
        .map(Value::py_repr)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `repr(bytes)`.
pub fn repr_bytes(b: &[u8]) -> String {
    let quote = if b.contains(&b'\'') && !b.contains(&b'"') {
        '"'
    } else {
        '\''
    };
    let mut out = format!("b{quote}");
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            c if c as char == quote => {
                out.push('\\');
                out.push(c as char);
            }
            0x20..=0x7e => out.push(c as char),
            c => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out.push(quote);
    out
}

/// `int == float` for an integral float.
fn int_eq_float(i: &str, f: f64) -> bool {
    if f.abs() < 9.0e15 {
        return i.parse::<i64>().is_ok_and(|n| n as f64 == f);
    }
    // Large values: compare the float's exact integer text.
    format!("{f:.0}") == i
}

/// A Python `dict`: insertion order kept, a key set again keeps its place. Keys compare as
/// Python compares them (`1`, `1.0` and `True` are the same key).
#[derive(Debug, Clone, Default)]
pub struct Dict {
    entries: Vec<(Value, Value)>,
}

impl Dict {
    pub const fn new() -> Self {
        Dict {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// `d.get("key")`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    /// `d.get(key)` for any key.
    pub fn get_value(&self, key: &Value) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(k, _)| k.py_eq(key))
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    /// `d.get(key)`: `None` when missing.
    pub fn get_or_none(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&NONE)
    }

    /// `d["key"] = value`.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.insert_value(Value::Str(key.into()), value);
    }

    /// `d[key] = value` for any hashable key: an equal key keeps its place and its own object.
    pub fn insert_value(&mut self, key: Value, value: Value) {
        match self.entries.iter_mut().find(|(k, _)| k.py_eq(&key)) {
            Some(slot) => slot.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// `del d[key]`.
    /// Appends an entry whose key the caller knows is not in the dict yet.
    pub fn push_new(&mut self, key: Value, value: Value) {
        self.entries.push((key, value));
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self
            .entries
            .iter()
            .position(|(k, _)| k.as_str() == Some(key))?;
        Some(self.entries.remove(i).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Value, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// The entries whose keys are strings, as every JSON object has.
    pub fn str_iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.iter().filter_map(|(k, v)| k.as_str().map(|k| (k, v)))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(k, _)| k)
    }
}

impl FromIterator<(String, Value)> for Dict {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        let mut d = Dict::new();
        for (k, v) in iter {
            d.insert(k, v);
        }
        d
    }
}

impl FromIterator<(Value, Value)> for Dict {
    fn from_iter<I: IntoIterator<Item = (Value, Value)>>(iter: I) -> Self {
        let mut d = Dict::new();
        for (k, v) in iter {
            d.insert_value(k, v);
        }
        d
    }
}
