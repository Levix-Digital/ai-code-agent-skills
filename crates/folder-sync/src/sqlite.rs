//! The values and errors of Python's `sqlite3`, for the scripts that read a sync state.

use kernel::py::value::repr_bytes;
use kernel::py::{Crash, float_repr};
use rusqlite::types::ValueRef;

/// A value as `sqlite3` returns it to Python.
#[derive(Debug, Clone)]
pub enum Cell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    pub fn from_ref(v: ValueRef<'_>) -> Cell {
        match v {
            ValueRef::Null => Cell::Null,
            ValueRef::Integer(i) => Cell::Int(i),
            ValueRef::Real(f) => Cell::Real(f),
            ValueRef::Text(t) => Cell::Text(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(b) => Cell::Blob(b.to_vec()),
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Cell::Null => false,
            Cell::Int(i) => *i != 0,
            Cell::Real(f) => *f != 0.0,
            Cell::Text(t) => !t.is_empty(),
            Cell::Blob(b) => !b.is_empty(),
        }
    }

    /// `str(value)`.
    pub fn py_str(&self) -> String {
        match self {
            Cell::Null => "None".into(),
            Cell::Int(i) => i.to_string(),
            Cell::Real(f) => float_repr(*f),
            Cell::Text(t) => t.clone(),
            Cell::Blob(b) => repr_bytes(b),
        }
    }

    /// `str(value or 0)`.
    pub fn or_zero(&self) -> String {
        if self.truthy() {
            self.py_str()
        } else {
            "0".into()
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Cell::Null => "NoneType",
            Cell::Int(_) => "int",
            Cell::Real(_) => "float",
            Cell::Text(_) => "str",
            Cell::Blob(_) => "bytes",
        }
    }
}

/// The `sqlite3` exception for an error of the SQLite library.
pub fn sqlite_crash(e: rusqlite::Error) -> Crash {
    use rusqlite::ffi::ErrorCode;
    let (name, text) = match &e {
        rusqlite::Error::SqliteFailure(f, msg) => {
            let name = match f.code {
                ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt => "DatabaseError",
                ErrorCode::ReadOnly | ErrorCode::ConstraintViolation => "IntegrityError",
                _ => "OperationalError",
            };
            let text = msg.clone().unwrap_or_else(|| f.to_string());
            (name, text)
        }
        other => ("OperationalError", other.to_string()),
    };
    Crash::new(format!("sqlite3.{name}: {text}"))
}
