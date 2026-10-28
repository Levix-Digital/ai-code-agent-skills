//! `new_adr.py`: creates the next Architecture Decision Record (docs/adr/NNNN-<slug>.md)
//! from the standard template, records relations in both directions (`--supersedes` /
//! `--amends`) and refreshes the index. Accepted ADRs are never rewritten afterwards: only
//! their status and relation fields change.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::num::strip;
use kernel::py::{Crash, Dict, Value, today};

use crate::build_index::build_index;
use crate::docs::{
    ADR_FIELD_ORDER, ADR_STATUSES, Doc, Project, adr_label, as_id_list, cmp_int, find_adr, get,
    next_adr_id, write_doc_ordered,
};

/// `BODY_TEMPLATE.format(label=label, title=title)`.
fn body(label: &str, title: &str) -> String {
    format!(
        "# {label}: {title}

## Context

What problem or force made this decision necessary? Facts only: current behavior, constraints, measurements.

## Decision

What was decided, stated precisely (components, files, interfaces, defaults).

## Alternatives Considered

- **Alternative A**: why it was not chosen.

## Consequences

What becomes easier or harder, risks, follow-up work, and how to verify the decision holds.
"
    )
}

fn int_list(ids: &[String]) -> Value {
    Value::List(ids.iter().map(|i| Value::Int(i.clone())).collect())
}

/// `sorted(set(ids))`.
fn sorted_unique(mut ids: Vec<String>) -> Vec<String> {
    ids.sort_by(|a, b| cmp_int(a, b));
    ids.dedup();
    ids
}

/// `new_adr(project, title, status, version, subsystems, supersedes, amends)`.
fn new_adr(
    project: &Project,
    title: &str,
    status: &str,
    version: Option<&str>,
    subsystems: Vec<Value>,
    supersedes: &[String],
    amends: &[String],
) -> Result<u8, Crash> {
    let (mut docs, entries) = project.adr_by_id()?;
    for target in supersedes.iter().chain(amends) {
        if find_adr(&entries, target).is_none() {
            err(&format!(
                "❌ {} not found in {}.\n",
                adr_label(target),
                project.rel(&project.adr_dir)?
            ));
            return Ok(1);
        }
    }

    let adr_id = next_adr_id(&entries)?;
    let label = adr_label(&adr_id);
    let mut meta = Dict::new();
    meta.insert("id", Value::Int(adr_id.clone()));
    meta.insert("title", Value::str(title));
    meta.insert("status", Value::str(status));
    meta.insert("date", Value::str(today()));
    meta.insert("version", version.map_or(Value::Null, Value::str));
    meta.insert("subsystems", Value::List(subsystems));
    meta.insert("supersedes", int_list(supersedes));
    meta.insert("superseded_by", Value::List(Vec::new()));
    meta.insert("amends", int_list(amends));
    meta.insert("amended_by", Value::List(Vec::new()));
    let doc = Doc {
        path: project.adr_path(&adr_id, title),
        meta: Value::Dict(meta),
        body: body(&label, title),
        has_frontmatter: true,
    };
    write_doc_ordered(&doc, &ADR_FIELD_ORDER)?;

    for (field, back, targets) in [
        ("supersedes", "superseded_by", supersedes),
        ("amends", "amended_by", amends),
    ] {
        for target in targets {
            let Some(i) = find_adr(&entries, target) else {
                continue;
            };
            let old = &mut docs[i];
            let mut ids = as_id_list(get(&old.meta, back)?)?;
            ids.push(adr_id.clone());
            if let Value::Dict(m) = &mut old.meta {
                m.insert(back, int_list(&sorted_unique(ids)));
                if field == "supersedes" {
                    m.insert("status", Value::str("superseded"));
                }
            }
            write_doc_ordered(old, &ADR_FIELD_ORDER)?;
            println(&format!(
                "🔗 {}: {back} += {label}{}",
                adr_label(target),
                if field == "supersedes" {
                    " (status: superseded)"
                } else {
                    ""
                }
            ));
        }
    }

    build_index(project, false)?;
    println(&format!("✅ {label} created: {}", project.rel(&doc.path)?));
    println(
        "   👉 Fill in Context, Decision, Alternatives Considered and Consequences, link the ADR from the",
    );
    println("      affected subsystem pages, run check_docs.py and commit.");
    Ok(0)
}

/// `python new_adr.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

/// `as_id_list(text.split(","))`.
fn id_args(text: &str) -> Result<Vec<String>, Crash> {
    as_id_list(&Value::List(text.split(',').map(Value::str).collect()))
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new("new_adr.py", "Create the next Architecture Decision Record")
        .required_option("--title", "Decision title, without version numbers")
        .choice_option("--status", &ADR_STATUSES[..2], "Initial status")
        .option(
            "--version",
            "Version that ships the decision (e.g. v0.46.0)",
        )
        .option(
            "--subsystem",
            "Comma-separated subsystem page names (e.g. rag,onnx)",
        )
        .option(
            "--supersedes",
            "Comma-separated ADR numbers this one replaces entirely",
        )
        .option(
            "--amends",
            "Comma-separated ADR numbers this one corrects in part",
        )
        .option(
            "--dir",
            "Project directory (default: current; the Git root is used)",
        );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let title = args.value("title").unwrap_or_default();
    let status = args.value("status").unwrap_or("accepted");
    let subsystems: Vec<Value> = args
        .value("subsystem")
        .unwrap_or_default()
        .split(',')
        .map(strip)
        .filter(|s| !s.is_empty())
        .map(Value::str)
        .collect();

    let project = Project::new(args.value("dir"))?;
    let supersedes = id_args(args.value("supersedes").unwrap_or_default())?;
    let amends = id_args(args.value("amends").unwrap_or_default())?;
    new_adr(
        &project,
        title,
        status,
        args.value("version"),
        subsystems,
        &supersedes,
        &amends,
    )
}
