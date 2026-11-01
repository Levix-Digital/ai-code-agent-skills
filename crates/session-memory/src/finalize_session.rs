//! `finalize_session.py`: replaces the project's master summary (`<project>::master_summary`)
//! with the consolidated text and removes that project's turn checkpoints. Other projects
//! are never touched.

use std::path::Path;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::{Crash, read_text, today};
use vector_store::{Include, MetaValue, Where};

use crate::common::{get_collection, master_id, resolve_project, store_crash};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "finalize_session.py",
        "Finalize session and update the project's consolidated master summary in ChromaDB",
    )
    .option(
        "--project",
        "Project name (default: git repository root name)",
    )
    .option("--summary", "Full consolidated master summary text")
    .option(
        "--file",
        "Path to text/markdown file containing master summary",
    )
    .option(
        "--version",
        "Optional project version the summary refers to",
    )
    .flag("--keep-legacy", "Do not delete raw turn chunks");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    let file = args.value("file").filter(|f| !f.is_empty());
    let summary = match file {
        Some(f) if Path::new(f).exists() => read_text(Path::new(f))?,
        _ => match args.value("summary").filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => {
                println("Error: Either --summary or --file must be specified.");
                return Ok(1);
            }
        },
    };
    let project = match resolve_project(args.value("project"))? {
        Ok(p) => p,
        Err(code) => return Ok(code),
    };
    finalize_session(
        &project,
        &summary,
        args.value("version"),
        args.flag("keep_legacy"),
    )?;
    Ok(0)
}

fn finalize_session(
    project: &str,
    summary: &str,
    version: Option<&str>,
    keep_legacy: bool,
) -> Result<(), Crash> {
    let (mut client, col) = get_collection()?;
    let date = today();
    let str_value = |s: &str| MetaValue::Str(s.to_string());
    let of_project = Where::eq("project", str_value(project));

    // 1. Update the project's single Master Summary
    let mut metadata = vec![
        ("project".to_string(), str_value(project)),
        ("type".to_string(), str_value("master_summary")),
        ("last_updated".to_string(), str_value(&date)),
        (
            "topic".to_string(),
            str_value("consolidated_master_summary"),
        ),
    ];
    if let Some(v) = version.filter(|v| !v.is_empty()) {
        metadata.push(("version".to_string(), str_value(v)));
    }
    let id = master_id(project);
    client
        .upsert(
            &col,
            std::slice::from_ref(&id),
            &[summary.to_string()],
            &[metadata],
        )
        .map_err(store_crash)?;
    println(&format!(
        "✅ [{project}] Master Summary consolidated and updated in ChromaDB (Doc ID: '{id}')!"
    ));

    // 2. Remove this project's temporary turn chunks if not keeping legacy
    if !keep_legacy {
        let chunks = Where::And(vec![
            of_project.clone(),
            Where::eq("type", str_value("current_session_chunk")),
        ]);
        let ids = client
            .get(
                &col,
                None,
                Some(&chunks),
                None,
                0,
                &[Include::Documents, Include::Metadatas],
            )
            .map_err(store_crash)?
            .ids;
        if !ids.is_empty() {
            client.delete(&col, Some(&ids), None).map_err(store_crash)?;
            println(&format!(
                "🧹 [{project}] Cleaned up {} temporary session turn chunk(s) (now consolidated into master summary).",
                ids.len()
            ));
        }
    }

    let entries = client
        .get(
            &col,
            None,
            Some(&of_project),
            None,
            0,
            &[Include::Documents, Include::Metadatas],
        )
        .map_err(store_crash)?
        .ids
        .len();
    let total = client.count(&col).map_err(store_crash)?;
    println(&format!(
        "Entries for project '{project}': {entries} | Total entries: {total}"
    ));
    Ok(())
}
