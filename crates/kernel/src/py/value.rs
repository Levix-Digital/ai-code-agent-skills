//! A JSON value as Python holds it after `json.load`: `None`, `bool`, `int`, `float`, `str`,
//! `list` and `dict` (insertion-ordered), with Python's truthiness, `str()` and `repr()`.

use super::{float_repr, repr_str};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Python ints have no size limit: kept as their decimal text.
    Int(String),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Dict(Dict),
}

pub static NONE: Value = Value::Null;
pub static EMPTY: Dict = Dict::new();

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

    /// `bool(value)`.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => i != "0",
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Dict(d) => !d.is_empty(),
        }
    }

    /// `value in [<strings>]`.
    pub fn is_one_of(&self, options: &[&str]) -> bool {
        self.as_str().is_some_and(|s| options.contains(&s))
    }

    /// Python's type name, as error messages print it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "NoneType",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::List(_) => "list",
            Value::Dict(_) => "dict",
        }
    }

    /// `str(value)`, as an f-string prints it.
    pub fn py_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
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
            Value::Float(f) => {
                if f.is_nan() {
                    "nan".into()
                } else if f.is_infinite() {
                    if *f > 0.0 { "inf" } else { "-inf" }.into()
                } else {
                    float_repr(*f)
                }
            }
            Value::Str(s) => repr_str(s),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(Value::py_repr).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Dict(d) => {
                let parts: Vec<String> = d
                    .iter()
                    .map(|(k, v)| format!("{}: {}", repr_str(k), v.py_repr()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
}

/// A Python `dict` with string keys: insertion order kept, a key set again keeps its place.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dict {
    entries: Vec<(String, Value)>,
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
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// `d.get(key)`: `None` when missing.
    pub fn get_or_none(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&NONE)
    }

    /// `d[key] = value`.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        let key = key.into();
        match self.get_mut(&key) {
            Some(slot) => *slot = value,
            None => self.entries.push((key, value)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
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
