//! `list_plans.py`: lists the plans in the project's docs/plans folder, optionally filtered by
//! status or kind.

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::unicode::upper;
use kernel::py::{Crash, Value, join};

use crate::store::{KINDS, Plan, PlanStore, name_of, progress_bar};

const STATUS_EMOJI: [(&str, &str); 6] = [
    ("active", "🟢"),
    ("in_progress", "⚡"),
    ("completed", "✅"),
    ("superseded", "📦"),
    ("draft", "📝"),
    ("planned", "🗓️"),
];

/// `STATUS_EMOJI.get(status, "📌")`.
fn emoji(status: &Value) -> Result<&'static str, Crash> {
    if !status.is_hashable() {
        return Err(Crash::new(format!(
            "TypeError: unhashable type: '{}'",
            status.type_name()
        )));
    }
    Ok(STATUS_EMOJI
        .iter()
        .find(|(k, _)| status.as_str() == Some(k))
        .map_or("📌", |(_, e)| e))
}

/// `value or "-"`.
fn or_dash(v: &Value) -> String {
    if v.truthy() { v.py_str() } else { "-".into() }
}

fn print_plan(p: &Plan) -> Result<(), Crash> {
    let m = &p.meta;
    let p_status = m.get_or_none("status");
    let title = m.get("title").map_or("Untitled".into(), Value::py_str);
    println(&format!("{} {title}", emoji(p_status)?));
    println(&format!(
        "   • Plan ID    : {}",
        m.get_or_none("plan_id").py_str()
    ));
    let version = or_dash(m.get_or_none("version"));
    let Value::Str(status) = p_status else {
        return Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'upper'",
            p_status.type_name()
        )));
    };
    println(&format!(
        "   • Version    : {version} | Status: {} | Kind: {} | Size: {} chars",
        upper(status),
        p.kind().py_str(),
        p.body.chars().count()
    ));
    println(&format!(
        "   • Author     : {} | Created: {}",
        or_dash(m.get_or_none("author")),
        or_dash(m.get_or_none("created_at"))
    ));
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
    let backlog = p.backlog_items()?;
    if !backlog.is_empty() {
        println(&format!("   • Backlog    : {}", backlog.join(", ")));
    }
    let milestones = p.milestones();
    if !milestones.is_empty() {
        println(&format!(
            "   • Progress   : {}",
            progress_bar(p.done_milestones()?.len(), milestones.len())
        ));
        let names = milestones
            .iter()
            .map(|x| match x {
                Value::Dict(d) => name_of(d),
                _ => unreachable!("read_plan keeps every milestone a dict"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        println(&format!(
            "   • Milestones : {}",
            join(", ", &Value::List(names))?
        ));
    }
    println(&"-".repeat(90));
    Ok(())
}

/// `list_plans(store, status, kind)`.
pub fn list_plans(
    store: &PlanStore,
    status: Option<&str>,
    kind: Option<&str>,
) -> Result<(), Crash> {
    let mut plans = store.load_all()?;
    let total = plans.len();
    if let Some(status) = status.filter(|s| !s.is_empty()) {
        let want = Value::str(status);
        plans.retain(|p| p.meta.get_or_none("status").py_eq(&want));
    }
    if let Some(kind) = kind.filter(|k| !k.is_empty()) {
        let want = Value::str(kind);
        plans.retain(|p| p.kind().py_eq(&want));
    }
    // `sort(key=str(created_at or ""), reverse=True)`: stable, so ties keep the file order.
    let key = |p: &Plan| {
        let c = p.meta.get_or_none("created_at");
        if c.truthy() {
            c.py_str()
        } else {
            String::new()
        }
    };
    plans.sort_by_cached_key(|p| std::cmp::Reverse(key(p)));

    println(&"=".repeat(90));
    println(&format!(
        "📋 PLANS OF '{}' (Showing: {} of {total}) — {}",
        store.project,
        plans.len(),
        store.plans_dir
    ));
    println(&"=".repeat(90));
    if plans.is_empty() {
        println("No plans matching the filter criteria were found.");
        return Ok(());
    }
    for p in &plans {
        print_plan(p)?;
    }
    Ok(())
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new("list_plans.py", "List the project's plans (docs/plans)")
        .option(
            "--status",
            "Filter by status (draft, planned, active, in_progress, completed, superseded)",
        )
        .choice_option("--kind", &KINDS, "Filter by kind (feature, bug)")
        .option("--project", "Project name (default: from the repository)")
        .option("--dir", "Project directory (default: current)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    list_plans(&store, args.value("status"), args.value("kind"))?;
    Ok(0)
}
