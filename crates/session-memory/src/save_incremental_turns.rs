//! `save_incremental_turns.py`: saves a 20-turn conversation block of the active session
//! into the local store, scoped to a project.

use std::path::Path;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::{Crash, num, read_text, today};
use vector_store::{Include, MetaValue, Where};

use crate::common::{chunk_id, get_collection, resolve_project, store_crash};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

/// `int(value)` of an argument `argparse` already checked, as the store keeps it.
fn turn(value: &str) -> Result<i64, Crash> {
    let n = num::int(value, 10)?.to_string();
    n.parse::<i64>().map_err(|_| {
        Crash::new(format!(
            "OverflowError: Python int too large to convert to SQLite INTEGER ({n})"
        ))
    })
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "save_incremental_turns.py",
        "Save 20-turn incremental block to ChromaDB session memory",
    )
    .option(
        "--project",
        "Project name (default: git repository root name)",
    )
    .required_int_option("--start", "Starting turn index")
    .required_int_option("--end", "Ending turn index")
    .option("--content", "Integral text content of the turns")
    .option("--file", "Path to a text/markdown file with the turns")
    .option("--session-id", "Optional session identifier");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    let file = args.value("file").filter(|f| !f.is_empty());
    let content = match file {
        Some(f) if Path::new(f).exists() => read_text(Path::new(f))?,
        _ => match args.value("content").filter(|c| !c.is_empty()) {
            Some(c) => c.to_string(),
            None => {
                println("Error: Either --content or --file must be specified.");
                return Ok(1);
            }
        },
    };
    let project = match resolve_project(args.value("project"))? {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    let start = args.value("start").unwrap_or_default();
    let end = args.value("end").unwrap_or_default();
    save_turn_block(&project, start, end, &content, args.value("session_id"))?;
    Ok(0)
}

fn save_turn_block(
    project: &str,
    start: &str,
    end: &str,
    turns_text: &str,
    session_id: Option<&str>,
) -> Result<(), Crash> {
    let (mut client, col) = get_collection()?;

    let date = today();
    let sid = session_id
        .filter(|s| !s.is_empty())
        .unwrap_or(&date)
        .to_string();
    let (start_n, end_n) = (
        num::int(start, 10)?.to_string(),
        num::int(end, 10)?.to_string(),
    );
    let (start_i, end_i) = (turn(start)?, turn(end)?);
    let doc_id = chunk_id(project, &sid, start_i, end_i);

    let doc = format!(
        "=== [{project}] CURRENT SESSION TURNS {start_n} TO {end_n} (Date: {date}) ===\n\n{turns_text}"
    );
    let metadata = vec![
        ("project".to_string(), MetaValue::Str(project.to_string())),
        (
            "type".to_string(),
            MetaValue::Str("current_session_chunk".into()),
        ),
        ("session_id".to_string(), MetaValue::Str(sid)),
        ("date".to_string(), MetaValue::Str(date)),
        ("start_turn".to_string(), MetaValue::Int(start_i)),
        ("end_turn".to_string(), MetaValue::Int(end_i)),
        (
            "topic".to_string(),
            MetaValue::Str(format!("current_session_turns_{start_n}_{end_n}")),
        ),
    ];
    client
        .upsert(&col, std::slice::from_ref(&doc_id), &[doc], &[metadata])
        .map_err(store_crash)?;

    println(&format!(
        "✅ [{project}] Incremental turns {start_n}-{end_n} saved into ChromaDB! (Doc ID: {doc_id})"
    ));
    let count = client
        .get(
            &col,
            None,
            Some(&Where::eq("project", MetaValue::Str(project.to_string()))),
            None,
            0,
            &[Include::Metadatas],
        )
        .map_err(store_crash)?
        .ids
        .len();
    println(&format!("Entries for project '{project}': {count}"));
    Ok(())
}
