//! `migrate_legacy.py`: copies records from an old single-project database (no `project`
//! metadata, unscoped IDs) into the shared multi-project database, assigning them to
//! `--project`. The source database is left untouched.

use std::path::Path;

use kernel::out::println;
use kernel::py::Crash;
use kernel::py::argparse::Parser;
use vector_store::{Client, Include, MetaValue, Metadata};

use crate::common::{
    COLLECTION_NAME, get_collection, get_db_path, master_id, resolve_project, store_crash,
};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "migrate_legacy.py",
        "Migrate legacy single-project session memory into the multi-project database",
    )
    .required_option("--source", "Path to the legacy ChromaDB directory")
    .required_option("--project", "Project the legacy records belong to")
    .flag("--dry-run", "Only show what would be migrated");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let project = match resolve_project(args.value("project"))? {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    migrate(
        args.value("source").unwrap_or_default(),
        &project,
        args.flag("dry_run"),
    )?;
    Ok(0)
}

fn get<'a>(meta: &'a Metadata, key: &str) -> Option<&'a MetaValue> {
    meta.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// `str(value)` of a metadata value.
fn text(v: &MetaValue) -> String {
    plan_memory::migrate_legacy::value_of(v).py_str()
}

/// `f"{value:03d}"`.
fn turn(v: &MetaValue) -> Result<String, Crash> {
    let unknown = |t: &str| {
        Crash::new(format!(
            "ValueError: Unknown format code 'd' for object of type '{t}'"
        ))
    };
    let unsupported = |t: &str| {
        Crash::new(format!(
            "TypeError: unsupported format string passed to {t}.__format__"
        ))
    };
    match v {
        MetaValue::Int(i) => Ok(format!("{i:03}")),
        MetaValue::Bool(b) => Ok(format!("{:03}", i64::from(*b))),
        MetaValue::Str(_) => Err(unknown("str")),
        MetaValue::Float(_) => Err(unknown("float")),
        MetaValue::Null => Err(unsupported("NoneType")),
        _ => Err(unsupported("list")),
    }
}

fn migrate(source: &str, project: &str, dry_run: bool) -> Result<(), Crash> {
    let mut src = Client::persistent(Path::new(source)).map_err(store_crash)?;
    let src_col = src
        .get_or_create_collection(COLLECTION_NAME, None)
        .map_err(store_crash)?;
    let records = src
        .get(
            &src_col,
            None,
            None,
            None,
            0,
            &[Include::Documents, Include::Metadatas],
        )
        .map_err(store_crash)?;
    let (mut dst, dst_col) = get_collection()?;

    for (i, old_id) in records.ids.iter().enumerate() {
        let mut meta: Metadata = records
            .metadatas
            .get(i)
            .cloned()
            .flatten()
            .unwrap_or_default();
        let doc = records.documents.get(i).cloned().flatten();
        if let Some(p) = get(&meta, "project")
            && plan_memory::migrate_legacy::value_of(p).truthy()
        {
            println(&format!(
                "↷ {old_id}: already has project '{}', skipped",
                text(p)
            ));
            continue;
        }
        let project_value = MetaValue::Str(project.to_string());
        match meta.iter_mut().find(|(k, _)| k == "project") {
            Some((_, v)) => *v = project_value,
            None => meta.push(("project".to_string(), project_value)),
        }
        let kind = get(&meta, "type");
        let is = |v: Option<&MetaValue>, s: &str| matches!(v, Some(MetaValue::Str(t)) if t == s);
        let new_id = if old_id == "master_summary" || is(kind, "master_summary") {
            master_id(project)
        } else if is(kind, "current_session_chunk") {
            let sid = get(&meta, "session_id").map_or_else(|| "legacy".to_string(), text);
            let zero = MetaValue::Int(0);
            let start = turn(get(&meta, "start_turn").unwrap_or(&zero))?;
            let end = turn(get(&meta, "end_turn").unwrap_or(&zero))?;
            format!("{project}::turns::{sid}::{start}_to_{end}")
        } else {
            format!("{project}::{old_id}")
        };
        println(&format!(
            "{}{old_id} -> {new_id}",
            if dry_run { "(dry-run) " } else { "" }
        ));
        if !dry_run {
            let Some(doc) = doc else {
                return Err(Crash::new(
                    "ValueError: Expected document to be a str, got None in upsert.",
                ));
            };
            dst.upsert(&dst_col, &[new_id], &[doc], &[meta])
                .map_err(store_crash)?;
        }
    }

    let count = dst.count(&dst_col).map_err(store_crash)?;
    println(&format!(
        "Destination DB: {} | Total entries: {count}",
        get_db_path().to_string_lossy()
    ));
    Ok(())
}
