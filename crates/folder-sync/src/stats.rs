//! `stats.py`: statistics of a folder's `.folder_brain` index, read from its sync state.

use std::path::Path;

use chrono::{Datelike, TimeZone, Timelike};
use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::path::{basename, exists, join, realpath};
use kernel::py::value::repr_bytes;
use kernel::py::{Crash, float_repr};
use rusqlite::types::ValueRef;

pub fn main(argv: &[String]) -> u8 {
    let parser = Parser::new("stats.py", "Folder Brain statistics")
        .option("--dir", "Target directory (default: current)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return exit.report(),
    };
    match show_stats(args.value("dir").unwrap_or(".")) {
        Ok(()) => 0,
        Err(c) => c.report(),
    }
}

/// A value as `sqlite3` returns it to Python.
#[derive(Debug, Clone)]
enum Cell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    fn from_ref(v: ValueRef<'_>) -> Cell {
        match v {
            ValueRef::Null => Cell::Null,
            ValueRef::Integer(i) => Cell::Int(i),
            ValueRef::Real(f) => Cell::Real(f),
            ValueRef::Text(t) => Cell::Text(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(b) => Cell::Blob(b.to_vec()),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Cell::Null => false,
            Cell::Int(i) => *i != 0,
            Cell::Real(f) => *f != 0.0,
            Cell::Text(t) => !t.is_empty(),
            Cell::Blob(b) => !b.is_empty(),
        }
    }

    /// `str(value)`.
    fn py_str(&self) -> String {
        match self {
            Cell::Null => "None".into(),
            Cell::Int(i) => i.to_string(),
            Cell::Real(f) => float_repr(*f),
            Cell::Text(t) => t.clone(),
            Cell::Blob(b) => repr_bytes(b),
        }
    }

    /// `str(value or 0)`.
    fn or_zero(&self) -> String {
        if self.truthy() {
            self.py_str()
        } else {
            "0".into()
        }
    }

    fn type_name(&self) -> &'static str {
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
fn sqlite_crash(e: rusqlite::Error) -> Crash {
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

fn show_stats(target_dir: &str) -> Result<(), Crash> {
    let root = realpath(target_dir);
    let name = basename(Path::new(&root));
    let brain = join(&root, ".folder_brain");
    let db_path = join(&brain, "sync_state.sqlite");

    if !exists(&brain) || !exists(&db_path) {
        println(&format!(
            "ℹ️ Folder '{name}' has no synced `.folder_brain` index yet."
        ));
        return Ok(());
    }

    let conn = rusqlite::Connection::open(&db_path).map_err(sqlite_crash)?;
    let totals: Vec<Cell> = conn
        .query_row(
            "SELECT COUNT(*), SUM(chunk_count), SUM(file_size), MAX(last_synced_at) FROM file_hashes",
            [],
            |r| (0..4).map(|i| r.get_ref(i).map(Cell::from_ref)).collect(),
        )
        .map_err(sqlite_crash)?;
    let mut stmt = conn
        .prepare(
            "SELECT relative_path, chunk_count, last_synced_at FROM file_hashes ORDER BY chunk_count DESC LIMIT 10",
        )
        .map_err(sqlite_crash)?;
    let top: Vec<(Cell, Cell)> = stmt
        .query_map([], |r| {
            Ok((Cell::from_ref(r.get_ref(0)?), Cell::from_ref(r.get_ref(1)?)))
        })
        .map_err(sqlite_crash)?
        .collect::<Result<_, _>>()
        .map_err(sqlite_crash)?;

    let size = match &totals[2] {
        Cell::Int(i) if *i != 0 => *i as f64,
        Cell::Real(f) if *f != 0.0 => *f,
        c if !c.truthy() => 0.0,
        c => {
            return Err(Crash::new(format!(
                "TypeError: unsupported operand type(s) for /: '{}' and 'int'",
                c.type_name()
            )));
        }
    };
    let size_mb = round2(size / (1024.0 * 1024.0));
    let last_sync = if totals[3].truthy() {
        from_timestamp(&totals[3])?
    } else {
        "Nunca".to_string()
    };

    println(&format!("\n📊 **Folder Brain Statistics:** `{name}`"));
    println(&format!("📁 Path: `{root}`"));
    println(&format!("📄 Total indexed files: {}", totals[0].or_zero()));
    println(&format!("🧩 Total vector chunks: {}", totals[1].or_zero()));
    println(&format!(
        "💾 Volume total de dados: {} MB",
        float_repr(size_mb)
    ));
    println(&format!("🕒 Last sync: {last_sync}\n"));

    if !top.is_empty() {
        println("🔝 **Maiores documentos indexados:**");
        for (rel_path, chunks) in &top {
            println(&format!(
                "- `{}` ({} chunks)",
                rel_path.py_str(),
                chunks.py_str()
            ));
        }
    }
    Ok(())
}

/// `round(x, 2)`: the decimal nearest to the exact value, ties to even.
fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// `datetime.fromtimestamp(t).strftime("%d/%m/%Y %H:%M:%S")`, in local time.
fn from_timestamp(t: &Cell) -> Result<String, Crash> {
    let (secs, _micros) = match t {
        Cell::Int(i) => (*i, 0),
        Cell::Real(f) => {
            if f.is_nan() {
                return Err(Crash::new("ValueError: Invalid value NaN (not a number)"));
            }
            let mut secs = f.trunc();
            let mut us = ((f - secs) * 1e6).round_ties_even();
            if us >= 1e6 {
                secs += 1.0;
                us -= 1e6;
            } else if us < 0.0 {
                secs -= 1.0;
                us += 1e6;
            }
            if !(-9.2e18..9.2e18).contains(&secs) {
                return Err(Crash::new(
                    "OverflowError: timestamp out of range for platform time_t",
                ));
            }
            (secs as i64, us as i64)
        }
        other => {
            return Err(Crash::new(format!(
                "TypeError: '{}' object cannot be interpreted as an integer",
                other.type_name()
            )));
        }
    };
    let local = match chrono::Local.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(d) | chrono::LocalResult::Ambiguous(d, _) => d,
        chrono::LocalResult::None => {
            return Err(Crash::new(format!(
                "ValueError: year is out of range for timestamp {secs}"
            )));
        }
    };
    let year = local.year();
    if !(1..=9999).contains(&year) {
        return Err(Crash::new(format!(
            "ValueError: year {year} is out of range"
        )));
    }
    Ok(format!(
        "{:02}/{:02}/{year} {:02}:{:02}:{:02}",
        local.day(),
        local.month(),
        local.hour(),
        local.minute(),
        local.second()
    ))
}
