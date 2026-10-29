//! `get_plan.py`: displays the full content of a plan by its Plan ID, or the project's latest
//! active plan, with its Spec Kit artifacts. Can write the body to a file instead.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::path::{abspath, dirname, makedirs};
use kernel::py::unicode::upper;
use kernel::py::{Crash, Value, write_text_native};

use crate::store::{PlanStore, progress_bar};

/// `text.rstrip()`.
fn rstrip(text: &str) -> &str {
    text.trim_end_matches(kernel::py::num::is_space)
}

/// `value or "-"`.
fn or_dash(v: &Value) -> String {
    if v.truthy() { v.py_str() } else { "-".into() }
}

/// `get_plan(store, plan_id, output_file)`: the exit code.
pub fn get_plan(
    store: &PlanStore,
    plan_id: Option<&str>,
    output_file: Option<&str>,
) -> Result<u8, Crash> {
    let plan = match plan_id.filter(|id| !id.is_empty()) {
        Some(id) => store.get(id)?,
        None => store.latest()?,
    };
    let Some(plan) = plan else {
        let shown = plan_id.map_or("None".to_string(), str::to_string);
        err(&format!(
            "❌ Error: Plan not found (plan_id='{shown}', project='{}') in {}\n",
            store.project, store.plans_dir
        ));
        return Ok(1);
    };

    let m = &plan.meta;
    let plan_id = m.get_or_none("plan_id").py_str();
    if let Some(output) = output_file.filter(|o| !o.is_empty()) {
        let out_dir = dirname(&abspath(output).to_string_lossy());
        makedirs(&out_dir)?;
        write_text_native(std::path::Path::new(output), &store.full_text(&plan)?)?;
        println(&format!("✅ Plan '{plan_id}' body written to: {output}"));
        return Ok(0);
    }

    println(&"=".repeat(90));
    println(&format!("📜 FULL ARCHITECTURAL PLAN: {plan_id}"));
    let project = m
        .get("project")
        .map_or(store.project.clone(), Value::py_str);
    let version = or_dash(m.get_or_none("version"));
    let status = match m.get("status") {
        Some(Value::Str(s)) => upper(s),
        Some(other) => {
            return Err(Crash::new(format!(
                "AttributeError: '{}' object has no attribute 'upper'",
                other.type_name()
            )));
        }
        None => "ACTIVE".into(),
    };
    println(&format!(
        "   • Project    : {project} | Version: {version} | Status: {status} | Kind: {}",
        plan.kind().py_str()
    ));
    println(&format!(
        "   • Author     : {} | Created: {} | Updated: {}",
        or_dash(m.get_or_none("author")),
        or_dash(m.get_or_none("created_at")),
        or_dash(m.get_or_none("updated_at"))
    ));
    println(&format!("   • File       : {}", plan.path));
    if m.get_or_none("supersedes").truthy() {
        println(&format!(
            "   • Supersedes : {}",
            m.get_or_none("supersedes").py_str()
        ));
    }
    if m.get_or_none("superseded_by").truthy() {
        println(&format!(
            "   • Superseded by: {}",
            m.get_or_none("superseded_by").py_str()
        ));
    }
    let backlog = plan.backlog_items()?;
    if !backlog.is_empty() {
        println(&format!("   • Backlog    : {}", backlog.join(", ")));
    }
    let spec_dir = plan.spec_dir().filter(|d| !d.is_empty());
    if let Some(dir) = &spec_dir {
        println(&format!("   • Spec Kit   : {dir}"));
    }
    let milestones = plan.milestones();
    if !milestones.is_empty() {
        println(&format!(
            "   • Milestones : {}",
            progress_bar(plan.done_milestones()?.len(), milestones.len())
        ));
    }
    println(&format!("{}\n", "=".repeat(90)));
    println(&plan.body);
    for (rel, text) in store.spec_files(&plan)? {
        println(&format!("\n{}", "-".repeat(90)));
        println(&format!("📎 Spec Kit artifact: {rel}"));
        println(&format!("{}\n", "-".repeat(90)));
        println(rstrip(&text));
    }
    if let Some(dir) = &spec_dir
        && store.spec_files(&plan)?.is_empty()
    {
        println(&format!(
            "\n⚠️ Spec folder '{dir}' has no Spec Kit artifacts."
        ));
    }
    println(&format!("\n{}", "=".repeat(90)));
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
        "get_plan.py",
        "Get a full, uncompressed plan from docs/plans",
    )
    .option(
        "--id",
        "Specific Plan ID (default: the project's latest active plan)",
    )
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)")
    .option("--output", "Write the plan body to a local markdown file");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    get_plan(&store, args.value("id"), args.value("output"))
}
