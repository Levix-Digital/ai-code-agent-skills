//! `read_memory.py`: prints a project's consolidated master summary and its active turn
//! checkpoints; `--list-projects` lists every project in the shared store.

use std::cmp::Ordering;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::{Crash, Value};
use plan_memory::migrate_legacy::value_of;
use vector_store::{Client, Collection, Include, MetaValue, Metadata, Where};

use crate::common::{get_collection, get_db_path, master_id, resolve_project, store_crash};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "read_memory.py",
        "Read a project's session memory from ChromaDB",
    )
    .option(
        "--project",
        "Project name (default: git repository root name)",
    )
    .flag("--list-projects", "List projects stored in the database");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let (mut client, col) = get_collection()?;
    if args.flag("list_projects") {
        list_projects(&mut client, &col)?;
    } else {
        let project = match resolve_project(args.value("project"))? {
            Ok(p) => p,
            Err(code) => return Ok(code),
        };
        read_project(&mut client, &col, &project)?;
    }
    Ok(0)
}

fn get(key: &Metadata, name: &str) -> Option<MetaValue> {
    key.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

/// A sort key compared as Python compares it: text with text, numbers with numbers.
#[derive(Clone)]
enum Key {
    Text(String),
    Int(i64),
    Float(f64),
    Other(Value),
}

impl Key {
    fn of(v: &MetaValue) -> Key {
        match v {
            MetaValue::Str(s) => Key::Text(s.clone()),
            MetaValue::Int(i) => Key::Int(*i),
            MetaValue::Bool(b) => Key::Int(i64::from(*b)),
            MetaValue::Float(f) => Key::Float(*f),
            other => Key::Other(value_of(other)),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Key::Text(_) => "str",
            Key::Int(_) => "int",
            Key::Float(_) => "float",
            Key::Other(v) => v.type_name(),
        }
    }

    fn cmp(&self, other: &Key) -> Result<Ordering, Crash> {
        let num = |k: &Key| match k {
            Key::Int(i) => Some(*i as f64),
            Key::Float(f) => Some(*f),
            _ => None,
        };
        match (self, other) {
            (Key::Text(a), Key::Text(b)) => Ok(a.cmp(b)),
            (Key::Int(a), Key::Int(b)) => Ok(a.cmp(b)),
            _ => match (num(self), num(other)) {
                (Some(a), Some(b)) => Ok(a.partial_cmp(&b).unwrap_or(Ordering::Equal)),
                _ => Err(Crash::new(format!(
                    "TypeError: '<' not supported between instances of '{}' and '{}'",
                    self.type_name(),
                    other.type_name()
                ))),
            },
        }
    }
}

/// `sorted(items, key=...)`: a stable insertion of each item, comparing as Python does.
fn sorted_by<T>(items: Vec<(Vec<Key>, T)>) -> Result<Vec<T>, Crash> {
    let mut out: Vec<(Vec<Key>, T)> = Vec::new();
    for (key, item) in items {
        let mut at = out.len();
        while at > 0 {
            if less(&key, &out[at - 1].0)? {
                at -= 1;
            } else {
                break;
            }
        }
        out.insert(at, (key, item));
    }
    Ok(out.into_iter().map(|(_, t)| t).collect())
}

/// Tuple `<`: the first unequal element decides.
fn less(a: &[Key], b: &[Key]) -> Result<bool, Crash> {
    for (x, y) in a.iter().zip(b) {
        match x.cmp(y)? {
            Ordering::Equal => continue,
            o => return Ok(o == Ordering::Less),
        }
    }
    Ok(a.len() < b.len())
}

/// `list_projects(col)`.
fn list_projects(client: &mut Client, col: &Collection) -> Result<(), Crash> {
    let all = client
        .get(col, None, None, None, 0, &[Include::Metadatas])
        .map_err(store_crash)?;
    let mut counts: Vec<(Key, String, usize)> = Vec::new();
    for meta in &all.metadatas {
        let key = meta
            .as_ref()
            .and_then(|m| get(m, "project"))
            .map_or_else(|| Key::Text("<no project>".into()), |v| Key::of(&v));
        let shown = match &key {
            Key::Text(s) => s.clone(),
            Key::Int(i) => i.to_string(),
            Key::Float(f) => kernel::py::float_repr(*f),
            Key::Other(v) => v.py_str(),
        };
        match counts.iter_mut().find(|(_, s, _)| *s == shown) {
            Some(entry) => entry.2 += 1,
            None => counts.push((key, shown, 1)),
        }
    }
    if counts.is_empty() {
        println("No memory entries found yet.");
    }
    let rows = sorted_by(
        counts
            .into_iter()
            .map(|(k, s, n)| (vec![k, Key::Int(n as i64)], (s, n)))
            .collect(),
    )?;
    for (p, n) in rows {
        println(&format!(
            "- {p} ({n} entr{})",
            if n == 1 { "y" } else { "ies" }
        ));
    }
    Ok(())
}

/// `read_project(col, project)`.
fn read_project(client: &mut Client, col: &Collection, project: &str) -> Result<(), Crash> {
    println(&format!(
        "Project: {project} | DB: {}\n",
        get_db_path().to_string_lossy()
    ));
    let both = [Include::Documents, Include::Metadatas];
    let master = client
        .get(col, Some(&[master_id(project)]), None, None, 0, &both)
        .map_err(store_crash)?;
    let rule = "=".repeat(80);
    if let Some(doc) = master.documents.first() {
        println(&rule);
        println(&format!(
            "📜 CONSOLIDATED MASTER SUMMARY [{project}] (All Prior Sessions Unified)"
        ));
        println(&rule);
        println(doc.as_deref().unwrap_or("None"));
        println(&format!("\n{rule}\n"));
    }

    let chunks = client
        .get(
            col,
            None,
            Some(&Where::And(vec![
                Where::eq("project", MetaValue::Str(project.to_string())),
                Where::eq("type", MetaValue::Str("current_session_chunk".into())),
            ])),
            None,
            0,
            &both,
        )
        .map_err(store_crash)?;
    let mut keyed = Vec::new();
    for (i, id) in chunks.ids.iter().enumerate() {
        let meta = chunks
            .metadatas
            .get(i)
            .cloned()
            .flatten()
            .unwrap_or_default();
        let doc = chunks.documents.get(i).cloned().flatten();
        let session = get(&meta, "session_id").map_or(Key::Text(String::new()), |v| Key::of(&v));
        let start = get(&meta, "start_turn").map_or(Key::Int(0), |v| Key::of(&v));
        keyed.push((vec![session, start], (id.clone(), meta, doc)));
    }
    let rows = sorted_by(keyed)?;
    for (id, meta, doc) in &rows {
        let topic =
            get(meta, "topic").map_or_else(|| "None".to_string(), |v| value_of(&v).py_str());
        println(&format!("--- Entry: {id} [{topic}] ---"));
        println(doc.as_deref().unwrap_or("None"));
        println("");
    }

    if master.documents.is_empty() && rows.is_empty() {
        println(&format!(
            "No memory entries found for project '{project}'. Existing projects:"
        ));
        list_projects(client, col)?;
    }
    Ok(())
}
