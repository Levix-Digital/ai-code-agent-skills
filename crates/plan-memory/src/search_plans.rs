//! `search_plans.py`: semantic vector search across the project's plans. The local index is
//! synced from docs/plans before every search.

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::num::{self, strip};
use kernel::py::unicode::upper;
use kernel::py::{Crash, float_repr};
use vector_store::{MetaValue, Metadata, Where};

use crate::index::{store_crash, sync_index};
use crate::store::PlanStore;

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "search_plans.py",
        "Semantic search over the project's plans",
    )
    .required_option("--query", "Search query or concept")
    .int_option("--limit", "Maximum number of results to return")
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)")
    .flag(
        "--all-projects",
        "Search every project indexed on this machine",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let limit = match args.value("limit") {
        Some(v) => num::int(v, 10)?,
        None => num::BigInt::from_i64(5),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    search_plans(
        &store,
        args.value("query").unwrap_or_default(),
        &limit,
        args.flag("all_projects"),
    )?;
    Ok(0)
}

/// `str(meta.get(key))`.
fn text(meta: &Metadata, key: &str) -> String {
    match meta.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
        None | Some(MetaValue::Null) => "None".into(),
        Some(MetaValue::Str(s)) => s.clone(),
        Some(MetaValue::Int(i)) => i.to_string(),
        Some(MetaValue::Float(f)) => float_repr(*f),
        Some(MetaValue::Bool(b)) => if *b { "True" } else { "False" }.into(),
        Some(other) => other.to_json().to_string(),
    }
}

/// `meta.get(key) or default`, as text.
fn text_or(meta: &Metadata, key: &str, default: &str) -> String {
    let falsy = match meta.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
        None | Some(MetaValue::Null) => true,
        Some(MetaValue::Str(s)) => s.is_empty(),
        Some(MetaValue::Int(i)) => *i == 0,
        Some(MetaValue::Float(f)) => *f == 0.0,
        Some(MetaValue::Bool(b)) => !b,
        Some(_) => false,
    };
    if falsy {
        default.to_string()
    } else {
        text(meta, key)
    }
}

/// `(meta.get('status') or 'active').upper()`.
fn status(meta: &Metadata) -> Result<String, Crash> {
    match meta.iter().find(|(k, _)| k == "status").map(|(_, v)| v) {
        Some(MetaValue::Str(s)) if !s.is_empty() => Ok(upper(s)),
        None | Some(MetaValue::Null) | Some(MetaValue::Str(_)) => Ok("ACTIVE".into()),
        Some(MetaValue::Int(0)) | Some(MetaValue::Bool(false)) => Ok("ACTIVE".into()),
        Some(MetaValue::Float(f)) if *f == 0.0 => Ok("ACTIVE".into()),
        Some(v) => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'upper'",
            match v {
                MetaValue::Int(_) => "int",
                MetaValue::Float(_) => "float",
                MetaValue::Bool(_) => "bool",
                _ => "list",
            }
        ))),
    }
}

pub fn search_plans(
    store: &PlanStore,
    query: &str,
    limit: &num::BigInt,
    all_projects: bool,
) -> Result<(), Crash> {
    let (mut client, col) = sync_index(store, None, false, true)?;
    let filter = Where::eq("project", MetaValue::Str(store.project.clone()));
    let filter = (!all_projects).then_some(filter);
    let count = match &filter {
        Some(w) => client
            .get(&col, None, Some(w), None, 0, &[])
            .map_err(store_crash)?
            .ids
            .len() as u64,
        None => client.count(&col).map_err(store_crash)?,
    };
    if count == 0 {
        println(&format!(
            "No plans indexed for '{}' ({}).",
            store.project, store.plans_dir
        ));
        return Ok(());
    }

    // `min(limit, count)`: a limit beyond i64 is past any count.
    let limit = limit.to_string();
    let n = match limit.parse::<i64>() {
        Ok(l) => l.min(count as i64),
        Err(_) if limit.starts_with('-') => {
            return Err(Crash::new(format!(
                "TypeError: Number of requested results {limit}, cannot be negative, or zero. in query."
            )));
        }
        Err(_) => count as i64,
    };
    if n <= 0 {
        return Err(Crash::new(format!(
            "TypeError: Number of requested results {n}, cannot be negative, or zero. in query."
        )));
    }
    let results = client
        .query(&col, query, n as usize, filter.as_ref())
        .map_err(store_crash)?;

    let rule = "=".repeat(90);
    println(&rule);
    println(&format!(
        "🔍 SEMANTIC SEARCH IN PLANS: '{query}' (Found: {})",
        results.ids.len()
    ));
    println(&rule);
    if results.ids.is_empty() {
        println("No matching plans found.");
        return Ok(());
    }

    for (i, pid) in results.ids.iter().enumerate() {
        let meta = results.metadatas[i].clone().unwrap_or_default();
        let doc = results.documents[i].clone().unwrap_or_default();
        let dist = results.distances[i] as f64;
        let sim = 1.0 / (1.0 + dist.max(0.0));
        println(&format!(
            "🎯 Score: {sim:.3} | [{}] {} ({})",
            text(&meta, "project"),
            text_or(&meta, "title", "Untitled"),
            text_or(&meta, "version", "-")
        ));
        println(&format!(
            "   • Plan ID    : {pid} | Status: {}",
            status(&meta)?
        ));
        println(&format!("   • File       : {}", text(&meta, "path")));
        let mut preview = strip(&doc).replace('\n', " ");
        if preview.chars().count() > 300 {
            preview = preview.chars().take(300).collect::<String>() + "...";
        }
        println(&format!("   • Preview    : {preview}"));
        println(&"-".repeat(90));
    }
    Ok(())
}
