//! `migrate_legacy.py`: one-time migration from the old ChromaDB-only storage (collection
//! `project_plans`) to plan files in the repository (docs/plans/<plan_id>.md), reconciling
//! plan files that already exist there:
//!
//! - plan only in the database          -> new file with the database metadata
//! - file with the same ID as a plan    -> database metadata; body from whichever is newer
//! - file matching a plan's content     -> renamed (git mv) to the database ID, which the
//!   under another ID                      lineage fields reference; then as above
//! - file with no matching plan         -> kept, with minimal metadata (--orphan-status)
//!
//! The source database is never modified. The local search index is rebuilt at the end.

use std::path::{Path, PathBuf};

use architecture_docs::docs::{listdir, no_attribute, split_frontmatter};
use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::datetime::fromisoformat;
use kernel::py::num::strip;
use kernel::py::path::{isdir, isfile, join};
use kernel::py::re::Regex;
use kernel::py::subprocess::spawn_error;
use kernel::py::{Crash, Dict, Value, difflib, now_iso, read_text};
use vector_store::{Client, Include, MetaValue, Metadata, Where};

use crate::index::{get_db_path, store_crash, sync_index};
use crate::store::{Plan, PlanStore};

const LEGACY_COLLECTION: &str = "project_plans";
const NOTE_RE: &str = r"\[(\d{4}-\d{2}-\d{2}T[\d:.]+)\]\s*(.*?)(?=\s*\[\d{4}-\d{2}-\d{2}T|\Z)";

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "migrate_legacy.py",
        "Migrate plans from the old plan-memory database to docs/plans files",
    )
    .option(
        "--source",
        "Legacy ChromaDB folder (default: the plan-memory database path)",
    )
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)")
    .option(
        "--orphan-status",
        "Status for plan files with no database entry",
    )
    .float_option(
        "--threshold",
        "Content similarity needed to pair a file with a plan under another ID",
    )
    .flag("--dry-run", "Only show what would be done");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    let source = match args.value("source") {
        Some(s) if !s.is_empty() => PathBuf::from(s),
        _ => get_db_path(),
    };
    let orphan_status = args.value("orphan_status").unwrap_or("completed");
    let threshold = match args.value("threshold") {
        Some(t) => kernel::py::num::float(t)?,
        None => 0.9,
    };
    migrate(
        &store,
        &source,
        orphan_status,
        threshold,
        args.flag("dry_run"),
    )?;
    Ok(0)
}

/// A metadata value as chromadb returns it to Python.
fn value_of(v: &MetaValue) -> Value {
    let list = |items: Vec<Value>| Value::List(items);
    match v {
        MetaValue::Str(s) => Value::str(s.clone()),
        MetaValue::Int(i) => Value::Int(i.to_string()),
        MetaValue::Float(f) => Value::Float(*f),
        MetaValue::Bool(b) => Value::Bool(*b),
        MetaValue::Null => Value::Null,
        MetaValue::StrList(l) => list(l.iter().map(|s| Value::str(s.clone())).collect()),
        MetaValue::IntList(l) => list(l.iter().map(|i| Value::Int(i.to_string())).collect()),
        MetaValue::FloatList(l) => list(l.iter().map(|f| Value::Float(*f)).collect()),
        MetaValue::BoolList(l) => list(l.iter().map(|b| Value::Bool(*b)).collect()),
    }
}

/// `dict(m or {}, plan_id=i)`.
fn meta_dict(m: Option<&Metadata>, id: &str) -> Dict {
    let mut d = Dict::new();
    for (k, v) in m.into_iter().flatten() {
        d.insert(k.clone(), value_of(v));
    }
    d.insert("plan_id", Value::str(id));
    d
}

/// `split_csv(value)`: `(value or "").split(",")`, stripped, empty parts left out.
fn split_csv(value: &Value) -> Result<Vec<String>, Crash> {
    if !value.truthy() {
        return Ok(Vec::new());
    }
    let Value::Str(s) = value else {
        return Err(no_attribute(value, "split"));
    };
    Ok(s.split(',')
        .map(|p| strip(p).to_string())
        .filter(|p| !p.is_empty())
        .collect())
}

/// `iso_seconds(value)`.
fn iso_seconds(value: &Value) -> Result<Value, Crash> {
    if !value.truthy() {
        return Ok(Value::Null);
    }
    let Value::Str(s) = value else {
        return Err(Crash::new("TypeError: fromisoformat: argument must be str"));
    };
    Ok(match fromisoformat(s) {
        Some(mut d) => {
            d.microsecond = 0;
            Value::str(d.isoformat())
        }
        None => value.clone(),
    })
}

/// `git_date(root, path, first)`: the commit date (ISO, no offset) of the newest, or with
/// `first` the oldest, commit that touched the file.
fn git_date(root: &str, path: &str, first: bool) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["log", "--format=%cI", "--", path])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let words: Vec<&str> = text.split_whitespace().collect();
    let value = if first { words.last()? } else { words.first()? };
    let mut d = fromisoformat(value)?;
    d.offset = None;
    Some(d.isoformat())
}

fn opt(m: &Dict, key: &str) -> Value {
    match m.get_or_none(key) {
        Value::Str(s) if s.is_empty() || s == "unknown" || s == "unversioned" => Value::Null,
        v => v.clone(),
    }
}

/// `file_meta_from_db(m, project, author)`.
fn file_meta_from_db(m: &Dict, project: &str, author: &Value) -> Result<Dict, Crash> {
    let done = split_csv(m.get_or_none("completed_milestones"))?;
    let notes = m.get_or_none("status_notes");
    let notes = if notes.truthy() {
        match notes {
            Value::Str(s) => s.clone(),
            other => {
                return Err(Crash::new(format!(
                    "TypeError: expected string or bytes-like object, got '{}'",
                    other.type_name()
                )));
            }
        }
    } else {
        String::new()
    };
    let chars: Vec<char> = notes.chars().collect();
    let mut log = Vec::new();
    for found in Regex::fixed(NOTE_RE, "s").find_iter(&chars) {
        let at = found.group(&chars, 1).unwrap_or_default();
        let note = found.group(&chars, 2).unwrap_or_default();
        let mut entry = Dict::new();
        entry.insert("at", iso_seconds(&Value::str(at))?);
        entry.insert("note", Value::str(strip(&note)));
        log.push(Value::Dict(entry));
    }
    let id = m.get_or_none("plan_id").clone();
    let title = m.get_or_none("title");
    let status = m.get_or_none("status");
    let mut milestones = Vec::new();
    for n in split_csv(m.get_or_none("milestones"))? {
        let is_done = done.contains(&n);
        let mut ms = Dict::new();
        ms.insert("name", Value::str(n));
        ms.insert("done", Value::Bool(is_done));
        milestones.push(Value::Dict(ms));
    }
    let mut meta = Dict::new();
    meta.insert("plan_id", id.clone());
    meta.insert("project", Value::str(project));
    meta.insert("title", if title.truthy() { title.clone() } else { id });
    meta.insert("version", opt(m, "version"));
    meta.insert(
        "status",
        if status.truthy() {
            status.clone()
        } else {
            Value::str("active")
        },
    );
    meta.insert("author", author.clone());
    meta.insert("created_at", iso_seconds(m.get_or_none("created_at"))?);
    meta.insert("updated_at", iso_seconds(m.get_or_none("updated_at"))?);
    meta.insert("session_id", opt(m, "session_id"));
    meta.insert(
        "tags",
        Value::List(
            split_csv(m.get_or_none("tags"))?
                .into_iter()
                .map(Value::str)
                .collect(),
        ),
    );
    meta.insert("supersedes", opt(m, "supersedes"));
    meta.insert("superseded_by", opt(m, "superseded_by"));
    meta.insert("parent_plan_id", opt(m, "parent_plan_id"));
    meta.insert("milestones", Value::List(milestones));
    meta.insert("status_log", Value::List(log));
    Ok(meta)
}

/// `first_heading(body, default)`.
fn first_heading(body: &str, default: &str) -> String {
    for line in kernel::py::splitlines(body) {
        if let Some(rest) = line.strip_prefix("# ") {
            return strip(rest).to_string();
        }
    }
    default.to_string()
}

/// `similarity(a, b)`: the ratio of the first 3000 characters of each.
fn similarity(a: &str, b: &str) -> f64 {
    let head = |s: &str| s.chars().take(3000).collect::<String>();
    difflib::ratio(&head(a), &head(b))
}

/// `text.strip()` of a document, which is `None` when the record has none.
fn strip_doc(doc: &Option<String>) -> Result<String, Crash> {
    match doc {
        Some(d) => Ok(strip(d).to_string()),
        None => Err(no_attribute(&Value::Null, "strip")),
    }
}

fn git_mv(root: &str, from: &str, to: &str) -> Result<(), Crash> {
    let cmd = ["git", "mv", from, to];
    let status = std::process::Command::new("git")
        .args(&cmd[1..])
        .current_dir(root)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => {
            let list = Value::List(cmd.iter().map(|c| Value::str(*c)).collect());
            Err(Crash::new(format!(
                "subprocess.CalledProcessError: Command '{}' returned non-zero exit status {}.",
                list.py_repr(),
                s.code().unwrap_or(-1)
            )))
        }
        Err(e) => {
            let (not_found, text) = spawn_error(&e, "git");
            Err(Crash::new(if not_found {
                format!("FileNotFoundError: {text}")
            } else {
                text
            }))
        }
    }
}

struct DbPlan {
    id: String,
    doc: Option<String>,
    meta: Dict,
}

struct Pair {
    fid: String,
    path: String,
    body: String,
}

fn migrate(
    store: &PlanStore,
    source: &Path,
    orphan_status: &str,
    threshold: f64,
    dry_run: bool,
) -> Result<(), Crash> {
    let tag = if dry_run { "(dry-run) " } else { "" };
    let author = store.author();

    let mut client = Client::persistent(source).map_err(store_crash)?;
    let col = client
        .get_collection(LEGACY_COLLECTION)
        .map_err(store_crash)?;
    let res = client
        .get(
            &col,
            None,
            Some(&Where::eq("project", MetaValue::Str(store.project.clone()))),
            None,
            0,
            &[Include::Documents, Include::Metadatas],
        )
        .map_err(store_crash)?;
    let mut db: Vec<DbPlan> = Vec::new();
    for (i, id) in res.ids.iter().enumerate() {
        let meta = meta_dict(res.metadatas.get(i).and_then(Option::as_ref), id);
        let doc = res.documents.get(i).cloned().flatten();
        match db.iter_mut().find(|p| &p.id == id) {
            Some(p) => {
                p.doc = doc;
                p.meta = meta;
            }
            None => db.push(DbPlan {
                id: id.clone(),
                doc,
                meta,
            }),
        }
    }
    let in_db = |id: &str| db.iter().any(|p| p.id == id);
    println(&format!(
        "Source DB: {} plan(s) for project '{}' | Target: {}",
        db.len(),
        store.project,
        store.plans_dir
    ));

    let mut files: Vec<(String, String, String)> = Vec::new();
    if isdir(&store.plans_dir) {
        for name in listdir(&store.plans_dir)? {
            if let Some(fid) = name.strip_suffix(".md") {
                let path = join(&store.plans_dir, &name);
                let (_, body, _) = split_frontmatter(&read_text(Path::new(&path))?)?;
                files.push((fid.to_string(), path, strip(&body).to_string()));
            }
        }
    }
    println(&format!("Existing plan files: {}\n", files.len()));

    // 1. Pair files with database plans (same ID, else by content)
    let mut pairs: Vec<(String, Pair)> = Vec::new();
    let mut orphans: Vec<Pair> = Vec::new();
    let mut claimed: Vec<String> = files
        .iter()
        .filter(|(fid, _, _)| in_db(fid))
        .map(|(fid, _, _)| fid.clone())
        .collect();
    for (fid, path, body) in &files {
        let pair = Pair {
            fid: fid.clone(),
            path: path.clone(),
            body: body.clone(),
        };
        if in_db(fid) {
            pairs.push((fid.clone(), pair));
            continue;
        }
        let mut best: Option<(f64, String)> = None;
        for p in &db {
            if claimed.contains(&p.id) {
                continue;
            }
            let score = similarity(&strip_doc(&p.doc)?, body);
            let better = match &best {
                None => true,
                Some((s, id)) => score > *s || (score == *s && p.id > *id),
            };
            if better {
                best = Some((score, p.id.clone()));
            }
        }
        match best {
            Some((score, id)) if !id.is_empty() && score >= threshold => {
                claimed.push(id.clone());
                println(&format!(
                    "{tag}rename  {fid}.md -> {id}.md  (content match {score:.2})"
                ));
                pairs.push((id, pair));
            }
            _ => orphans.push(pair),
        }
    }

    // 2. Write every database plan as a file
    let mut written = 0;
    for p in &db {
        let mut meta = file_meta_from_db(&p.meta, &store.project, &author)?;
        let mut body = strip_doc(&p.doc)?;
        if let Some((_, pair)) = pairs.iter().find(|(id, _)| id == &p.id) {
            if pair.body != body {
                let file_date = git_date(&store.root, &pair.path, false).unwrap_or_default();
                let updated = match meta.get_or_none("updated_at") {
                    Value::Str(s) => s.clone(),
                    _ => String::new(),
                };
                let choice = if file_date >= updated {
                    body = pair.body.clone();
                    format!(
                        "file (last commit {})",
                        file_date.chars().take(10).collect::<String>()
                    )
                } else {
                    format!(
                        "database (updated {})",
                        updated.chars().take(10).collect::<String>()
                    )
                };
                println(&format!("{tag}body    {}: differs, keeping {choice}", p.id));
            }
            if pair.fid != p.id && !dry_run {
                git_mv(&store.root, &pair.path, &store.path_for(&p.id))?;
            }
        }
        let Some(Value::List(mut log)) = meta.get("status_log").cloned() else {
            unreachable!("status_log is a list");
        };
        let mut entry = Dict::new();
        entry.insert("at", Value::str(now_iso()));
        entry.insert("by", author.clone());
        entry.insert(
            "note",
            Value::str("Migrated from the local plan-memory database to docs/plans"),
        );
        log.push(Value::Dict(entry));
        meta.insert("status_log", Value::List(log));
        if !dry_run {
            store.save(&mut new_plan(meta, body))?;
        }
        written += 1;
    }

    // 3. Files with no matching database plan
    for o in &orphans {
        let date = |first| git_date(&store.root, &o.path, first).map_or(Value::Null, Value::str);
        let mut log_entry = Dict::new();
        log_entry.insert("at", Value::str(now_iso()));
        log_entry.insert("by", author.clone());
        log_entry.insert("status", Value::str(orphan_status));
        log_entry.insert(
            "note",
            Value::str(
                "Plan file had no entry in the plan-memory database; metadata added during migration",
            ),
        );
        let mut meta = Dict::new();
        meta.insert("plan_id", Value::str(o.fid.clone()));
        meta.insert("project", Value::str(store.project.clone()));
        meta.insert("title", Value::str(first_heading(&o.body, &o.fid)));
        meta.insert("version", Value::Null);
        meta.insert("status", Value::str(orphan_status));
        meta.insert("author", author.clone());
        meta.insert("created_at", date(true));
        meta.insert("updated_at", date(false));
        meta.insert("session_id", Value::Null);
        meta.insert("tags", Value::List(Vec::new()));
        meta.insert("supersedes", Value::Null);
        meta.insert("superseded_by", Value::Null);
        meta.insert("parent_plan_id", Value::Null);
        meta.insert("milestones", Value::List(Vec::new()));
        meta.insert("status_log", Value::List(vec![Value::Dict(log_entry)]));
        println(&format!(
            "{tag}orphan  {}.md -> status '{orphan_status}'",
            o.fid
        ));
        if !dry_run {
            store.save(&mut new_plan(meta, o.body.clone()))?;
        }
    }

    println(&format!(
        "\n{tag}{written} plan(s) from the database, {} orphan file(s): {} plan files.",
        orphans.len(),
        written + orphans.len()
    ));
    if !dry_run {
        sync_index(store, None, true, false)?;
        let mut bad: Vec<Value> = Vec::new();
        for p in store.load_all()? {
            for k in ["supersedes", "superseded_by", "parent_plan_id"] {
                let v = p.meta.get_or_none(k);
                if v.truthy() && !isfile(&store.path_for(&v.py_str())) {
                    let id = p.meta.get_or_none("plan_id").clone();
                    if !bad.iter().any(|b| b.py_repr() == id.py_repr()) {
                        bad.push(id);
                    }
                }
            }
        }
        if !bad.is_empty() {
            println(&format!(
                "⚠️ Lineage references to missing plans in: {}",
                Value::List(sorted(bad)?).py_repr()
            ));
        }
        println("👉 Review the changes in docs/plans and commit them.");
    }
    Ok(())
}

/// `sorted(values)` for plan IDs: text, or whole numbers read from YAML.
fn sorted(mut values: Vec<Value>) -> Result<Vec<Value>, Crash> {
    if values.iter().all(|v| matches!(v, Value::Str(_))) {
        values.sort_by_key(Value::py_str);
        return Ok(values);
    }
    if values.iter().all(|v| matches!(v, Value::Int(_))) {
        let num = |v: &Value| v.py_str().parse::<i128>().unwrap_or(0);
        values.sort_by_key(num);
        return Ok(values);
    }
    Err(Crash::new(
        "TypeError: '<' not supported between instances of 'int' and 'str'",
    ))
}

/// `Plan(meta, body)`.
fn new_plan(meta: Dict, body: String) -> Plan {
    Plan {
        meta,
        body,
        path: String::new(),
        sha256: None,
    }
}
