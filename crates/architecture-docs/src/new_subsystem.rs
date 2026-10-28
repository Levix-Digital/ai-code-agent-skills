//! `new_subsystem.py`: creates a living documentation page for a subsystem
//! (docs/architecture/<name>.md) from the standard template and refreshes the index.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::num::strip;
use kernel::py::path::{exists, join};
use kernel::py::{Crash, Dict, Value};

use crate::build_index::build_index;
use crate::docs::{Doc, Project, ascii_slug, write_doc};

/// `BODY_TEMPLATE.format(title=title, summary=summary)`.
fn body(title: &str, summary: &str) -> String {
    format!(
        "# {title}

{summary}

## Responsibilities

What this subsystem owns, and what it explicitly does not.

## Structure

Main modules and types, with paths. A diagram when it helps:

```mermaid
flowchart LR
    A[Caller] --> B[{title}]
```

## Behavior

How it works today: main flows, data formats, configuration, defaults, error handling.

## Interfaces

What other subsystems and surfaces (CLI, TUI, API, MCP) call, and what this subsystem calls.

## Decisions

Link the ADRs that shaped this subsystem, one line each on what they decided.
"
    )
}

/// `python new_subsystem.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "new_subsystem.py",
        "Create a subsystem page in docs/architecture",
    )
    .required_option("--name", "Short page name (e.g. rag, ingestion, tui)")
    .required_option(
        "--title",
        "Page title (e.g. Retrieval-Augmented Generation)",
    )
    .required_option("--summary", "One sentence on what the subsystem does")
    .option("--code", "Comma-separated source paths this page documents")
    .option(
        "--dir",
        "Project directory (default: current; the Git root is used)",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let name = args.value("name").unwrap_or_default();
    let title = args.value("title").unwrap_or_default();
    let summary = args.value("summary").unwrap_or_default();
    let code = args.value("code").unwrap_or_default();

    let project = Project::new(args.value("dir"))?;
    let path = join(
        &project.architecture_dir,
        &format!("{}.md", ascii_slug(name)),
    );
    if exists(&path) {
        err(&format!("❌ {} already exists.\n", project.rel(&path)?));
        return Ok(1);
    }
    let code: Vec<Value> = code
        .split(',')
        .map(strip)
        .filter(|c| !c.is_empty())
        .map(Value::str)
        .collect();
    let mut meta = Dict::new();
    meta.insert("title", Value::str(title));
    meta.insert("summary", Value::str(summary));
    meta.insert("code", Value::List(code));
    write_doc(&Doc {
        path: path.clone(),
        meta: Value::Dict(meta),
        body: body(title, summary),
        has_frontmatter: true,
    })?;
    build_index(&project, false)?;
    println(&format!(
        "✅ Subsystem page created: {}",
        project.rel(&path)?
    ));
    Ok(0)
}
