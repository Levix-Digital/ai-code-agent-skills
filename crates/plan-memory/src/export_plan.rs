//! `export_plan.py`: exports a plan file (`docs/plans/<plan_id>.md`) to JSON, or to a
//! Markdown copy elsewhere, with or without its frontmatter.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::path::{abspath, dirname, join, makedirs};
use kernel::py::{Crash, Value, io_crash, json, write_text_native};

use crate::store::PlanStore;

/// `export_plan(...)`: the exit code.
pub fn export_plan(
    store: &PlanStore,
    plan_id: Option<&str>,
    output_path: Option<&str>,
    output_dir: Option<&str>,
    format_type: &str,
    include_frontmatter: bool,
) -> Result<u8, Crash> {
    let plan = match plan_id.filter(|id| !id.is_empty()) {
        Some(id) => store.get(id)?,
        None => store.latest()?,
    };
    let Some(plan) = plan else {
        let shown = plan_id.map_or("None".to_string(), str::to_string);
        err(&format!(
            "❌ Error: Plan not found (plan_id='{shown}', project='{}')\n",
            store.project
        ));
        return Ok(1);
    };

    let pid = plan.meta.get_or_none("plan_id").py_str();
    let ext = if format_type == "json" {
        ".json"
    } else {
        ".md"
    };
    let target = match output_path.filter(|o| !o.is_empty()) {
        Some(o) => o.to_string(),
        None => {
            let dir = match output_dir.filter(|d| !d.is_empty()) {
                Some(d) => d.to_string(),
                None => abspath("").to_string_lossy().into_owned(),
            };
            join(&dir, &format!("{pid}{ext}"))
        }
    };
    let target = abspath(&target).to_string_lossy().into_owned();
    if target == abspath(&plan.path).to_string_lossy() {
        err("❌ Error: target is the plan file itself.\n");
        return Ok(1);
    }
    makedirs(&dirname(&target))?;
    let path = std::path::Path::new(&target);

    if format_type == "json" {
        let mut data = plan.meta.clone();
        data.insert("content", Value::str(&plan.body));
        let (text, error) = json::dump(&Value::Dict(data), Some(2));
        write_text_native(path, &text)?;
        if let Some(e) = error {
            return Err(e);
        }
    } else if include_frontmatter {
        std::fs::copy(&plan.path, path).map_err(|e| io_crash(&e, path))?;
    } else {
        write_text_native(path, &plan.body)?;
    }

    let size = std::fs::metadata(path)
        .map_err(|e| io_crash(&e, path))?
        .len();
    println(&"=".repeat(80));
    println("📦 PLAN EXPORTED");
    println(&format!("   • Plan ID : {pid}"));
    println(&format!("   • Format  : {}", format_type.to_uppercase()));
    println(&format!("   • File    : {target}"));
    println(&format!("   • Size    : {size} bytes"));
    println(&format!("{}\n", "=".repeat(80)));
    Ok(0)
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "export_plan.py",
        "Export a plan to JSON or to a Markdown copy",
    )
    .option(
        "--id",
        "Target Plan ID (default: the project's latest active plan)",
    )
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)")
    .option("--output", "Exact destination file path")
    .option("--output-dir", "Directory to save the export")
    .choice_option("--format", &["markdown", "json"], "Output format")
    .flag(
        "--no-frontmatter",
        "Exclude YAML frontmatter from a markdown export",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    export_plan(
        &store,
        args.value("id"),
        args.value("output"),
        args.value("output_dir"),
        args.value("format").unwrap_or("markdown"),
        !args.flag("no_frontmatter"),
    )
}
