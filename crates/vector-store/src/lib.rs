//! The local vector store of the memory skills. It reads and writes chromadb 1.5's
//! persistent format (the `chroma.sqlite3` file and one HNSW folder per vector segment),
//! so stores written by the Python scripts keep working and the other way round.
//!
//! The layers follow chromadb's own: the database file and its migrations (`db`), the
//! collections and segments (`sysdb`), the write-ahead log (`log`), the metadata segment
//! (`metadata`), the vector segment (`hnsw`), the step that applies the log to both
//! (`compaction`), the default embedding model (`embed`) and the client calls the scripts
//! make (`client`).

pub mod client;
pub mod compaction;
pub mod db;
pub mod embed;
pub mod hnsw;
pub mod log;
pub mod metadata;
pub mod sysdb;

use std::fmt;

pub use client::{Client, Collection, GetResult, Include, QueryResult};
pub use metadata::Where;

#[derive(Debug, Clone)]
pub struct StoreError(pub String);

impl StoreError {
    pub fn new(msg: impl Into<String>) -> Self {
        StoreError(msg.into())
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError(format!("sqlite: {e}"))
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError(format!("json: {e}"))
    }
}

/// A metadata value, as chromadb stores it.
#[derive(Debug, Clone, PartialEq)]
pub enum MetaValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    StrList(Vec<String>),
    IntList(Vec<i64>),
    FloatList(Vec<f64>),
    BoolList(Vec<bool>),
    /// `None` in an update: removes the key.
    Null,
}

impl MetaValue {
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            MetaValue::StrList(_)
                | MetaValue::IntList(_)
                | MetaValue::FloatList(_)
                | MetaValue::BoolList(_)
        )
    }

    /// The schema's value-type key: arrays index like their elements.
    pub fn type_key(&self) -> Option<&'static str> {
        match self {
            MetaValue::Str(_) | MetaValue::StrList(_) => Some("string"),
            MetaValue::Int(_) | MetaValue::IntList(_) => Some("int"),
            MetaValue::Float(_) | MetaValue::FloatList(_) => Some("float"),
            MetaValue::Bool(_) | MetaValue::BoolList(_) => Some("bool"),
            MetaValue::Null => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        match self {
            MetaValue::Str(s) => J::String(s.clone()),
            MetaValue::Int(i) => J::from(*i),
            MetaValue::Float(f) => J::from(*f),
            MetaValue::Bool(b) => J::Bool(*b),
            MetaValue::StrList(v) => J::Array(v.iter().map(|s| J::String(s.clone())).collect()),
            MetaValue::IntList(v) => J::Array(v.iter().map(|i| J::from(*i)).collect()),
            MetaValue::FloatList(v) => J::Array(v.iter().map(|f| J::from(*f)).collect()),
            MetaValue::BoolList(v) => J::Array(v.iter().map(|b| J::Bool(*b)).collect()),
            MetaValue::Null => J::Null,
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Result<MetaValue, StoreError> {
        use serde_json::Value as J;
        Ok(match v {
            J::Null => MetaValue::Null,
            J::Bool(b) => MetaValue::Bool(*b),
            J::String(s) => MetaValue::Str(s.clone()),
            J::Number(n) => match n.as_i64() {
                Some(i) => MetaValue::Int(i),
                None => MetaValue::Float(n.as_f64().unwrap_or(f64::NAN)),
            },
            J::Array(items) => {
                let first = items.first();
                match first {
                    Some(J::Bool(_)) => MetaValue::BoolList(
                        items
                            .iter()
                            .map(|i| i.as_bool().unwrap_or_default())
                            .collect(),
                    ),
                    Some(J::Number(n)) if n.is_i64() => MetaValue::IntList(
                        items
                            .iter()
                            .map(|i| i.as_i64().unwrap_or_default())
                            .collect(),
                    ),
                    Some(J::Number(_)) => MetaValue::FloatList(
                        items
                            .iter()
                            .map(|i| i.as_f64().unwrap_or_default())
                            .collect(),
                    ),
                    _ => MetaValue::StrList(
                        items
                            .iter()
                            .map(|i| i.as_str().unwrap_or_default().to_string())
                            .collect(),
                    ),
                }
            }
            J::Object(_) => {
                return Err(StoreError::new(
                    "Sparse vector is not supported for local chroma",
                ));
            }
        })
    }
}

/// Metadata in insertion order.
pub type Metadata = Vec<(String, MetaValue)>;

/// The key under which chromadb keeps a record's document among its metadata.
pub const DOCUMENT_KEY: &str = "chroma:document";
