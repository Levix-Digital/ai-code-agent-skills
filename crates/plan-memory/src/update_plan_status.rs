//! `update_plan_status.py`: updates the lifecycle status of a plan file (e.g. active ->
//! completed, in_progress, superseded) and brings its backlog items along.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::unicode::upper;
use kernel::py::{Crash, Value, now_iso};

use crate::backlog::sync_plan_items;
use crate::index::sync_index;
use crate::store::{PlanStore, STATUSES};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new("update_plan_status.py", "Update a plan's lifecycle status")
        .required_option("--id", "Plan ID to update")
        .required_choice_option("--status", &STATUSES, "New status")
        .option("--notes", "Optional progress notes / rationale")
        .option("--project", "Project name (default: from the repository)")
        .option("--dir", "Project directory (default: current)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    update_plan_status(
        &store,
        args.value("id").unwrap_or_default(),
        args.value("status").unwrap_or_default(),
        args.value("notes"),
    )
}

/// `value.upper()`.
fn upper_of(v: &Value) -> Result<String, Crash> {
    match v {
        Value::Str(s) => Ok(upper(s)),
        other => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'upper'",
            other.type_name()
        ))),
    }
}

/// Sets the status, logs it, saves the plan and syncs the index and the linked items.
/// Returns the exit code: 1 when the plan does not exist.
pub fn update_plan_status(
    store: &PlanStore,
    plan_id: &str,
    new_status: &str,
    notes: Option<&str>,
) -> Result<u8, Crash> {
    let Some(mut plan) = store.get(plan_id)? else {
        err(&format!(
            "❌ Error: Plan ID '{plan_id}' not found in {}.\n",
            store.plans_dir
        ));
        return Ok(1);
    };

    let old_status = plan
        .meta
        .get("status")
        .cloned()
        .unwrap_or_else(|| Value::str("active"));
    plan.meta.insert("status", Value::str(new_status));
    plan.meta.insert("updated_at", Value::str(now_iso()));
    let notes = notes.filter(|n| !n.is_empty());
    let note = match notes {
        Some(n) => n.to_string(),
        None => format!("Status {} -> {new_status}", old_status.py_str()),
    };
    plan.add_log_status(store.author(), &note, Some(&Value::str(new_status)))?;
    store.save(&mut plan)?;
    sync_index(store, None, false, true)?;
    let status = plan.meta.get_or_none("status").clone();
    let items = plan.backlog_items()?;
    let item_messages = if items.is_empty() {
        Vec::new()
    } else {
        // `plan.meta["plan_id"]`: the ID in the frontmatter.
        let pid = plan
            .meta
            .get("plan_id")
            .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?
            .py_str();
        sync_plan_items(&store.root, &pid, &status, &items)?
    };

    let meta = &plan.meta;
    println("✅ Plan status updated successfully!");
    println(&format!("   • Plan ID    : {plan_id}"));
    println(&format!(
        "   • Project    : {}",
        match meta.get("project") {
            Some(p) => p.py_str(),
            None => store.project.clone(),
        }
    ));
    println(&format!(
        "   • Title      : {}",
        meta.get_or_none("title").py_str()
    ));
    println(&format!(
        "   • Status     : {} -> {}",
        upper_of(&old_status)?,
        upper(new_status)
    ));
    if let Some(n) = notes {
        println(&format!("   • Notes      : {n}"));
    }
    println(&format!("   • File       : {}", plan.path));
    for message in &item_messages {
        println(&format!("   {message}"));
    }
    println("   👉 Commit the updated plan file.");
    Ok(0)
}
