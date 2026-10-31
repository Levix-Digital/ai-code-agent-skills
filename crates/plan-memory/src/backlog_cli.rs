//! `backlog.py`: the first level of planning. Captures bugs, features, improvements and
//! adjustments in the project backlog (default: docs/BACKLOG.md) without discovery, groups
//! them under epics, lists and triages them, and closes them. Writes the file; never commits.

use kernel::out::println;
use kernel::py::argparse::{Parsed, Parser};
use kernel::py::json;
use kernel::py::num::strip;
use kernel::py::unicode::upper;
use kernel::py::{Crash, Dict, Value, join as py_join, now_iso, today};

use crate::backlog::{
    Backlog, BacklogFile, CLOSED_STATUSES, Item, OPEN_STATUSES, PRIORITIES, SEVERITIES, TYPES,
    contains, lstrip, sync_plan_items,
};
use crate::index::sync_index;
use crate::roadmap::RoadmapError;
use crate::save_plan::{Options, save_plan};
use crate::store::{Plan, PlanStore};

const BUG_PLAN_TEMPLATE: &str = include_str!("../../../plan-memory/templates/BUG_PLAN_TEMPLATE.md");
const FEATURE_PLAN_TEMPLATE: &str =
    include_str!("../../../plan-memory/templates/FEATURE_PLAN_TEMPLATE.md");
const ITEMS_SECTION: &str = "## Backlog items";

fn error<T>(msg: impl Into<String>) -> Result<T, RoadmapError> {
    Err(RoadmapError::Error(msg.into()))
}

/// The roadmap read for the bug triage fails as `RoadmapError`, which `backlog.py` does not
/// catch.
fn uncaught(e: RoadmapError) -> RoadmapError {
    match e {
        RoadmapError::Error(m) => {
            RoadmapError::Crash(Crash::new(format!("_roadmap.RoadmapError: {m}")))
        }
        crash => crash,
    }
}

fn line_of(item: &Item) -> String {
    let rank = item.rank();
    let rank = if rank.is_empty() {
        "?".to_string()
    } else {
        rank
    };
    let plan = item.plan().map(|p| format!("  → {p}")).unwrap_or_default();
    let epic = item.epic().map(|e| format!("  [{e}]")).unwrap_or_default();
    format!(
        "{:<9} [{}] ({rank}) {}{epic}{plan}",
        item.id,
        item.status(),
        item.title
    )
}

fn epic_progress(backlog: &Backlog, epic: &Item) -> String {
    let items = backlog.epic_items(&epic.id, true);
    let done = items.iter().filter(|i| i.closed()).count();
    format!("{done}/{} items closed", items.len())
}

/// Type, section heading and the open items counted by status.
type Counts = (&'static str, &'static str, Vec<(&'static str, usize)>);

/// The open items of each type counted by status.
fn counts(backlog: &Backlog) -> Vec<Counts> {
    TYPES
        .iter()
        .map(|(kind, _, heading)| {
            let items = backlog.open_items_of(kind);
            let counts = OPEN_STATUSES
                .iter()
                .map(|s| (*s, items.iter().filter(|i| i.status() == *s).count()))
                .collect();
            (*kind, *heading, counts)
        })
        .collect()
}

fn summary_data(
    backlog: &Backlog,
    blocking: &Value,
    untriaged: &[(&Item, String)],
) -> Result<Dict, Crash> {
    let mut types = Dict::new();
    for (kind, _, by_status) in counts(backlog) {
        let mut d = Dict::new();
        for (s, n) in by_status {
            d.insert(s, Value::Int(n.to_string()));
        }
        types.insert(kind, Value::Dict(d));
    }
    let mut data = Dict::new();
    data.insert("types", Value::Dict(types));
    data.insert("blocking_severities", blocking.clone());
    let ids = backlog
        .blocking_bugs(blocking)?
        .iter()
        .map(|i| Value::str(&i.id))
        .collect();
    data.insert("blocking_bugs", Value::List(ids));
    let untriaged = untriaged
        .iter()
        .map(|(i, why)| {
            let mut d = Dict::new();
            d.insert("id", Value::str(&i.id));
            d.insert("reason", Value::str(why));
            Value::Dict(d)
        })
        .collect();
    data.insert("untriaged_bugs", Value::List(untriaged));
    let closed = backlog.items().filter(|i| i.closed()).count();
    data.insert("closed", Value::Int(closed.to_string()));
    Ok(data)
}

fn print_summary(backlog: &Backlog, bf: &BacklogFile) -> Result<(), RoadmapError> {
    println(&format!("📋 Backlog: {}", bf.rel()?));
    for (_, heading, by_status) in counts(backlog) {
        let total: usize = by_status.iter().map(|(_, n)| n).sum();
        let detail: Vec<String> = by_status
            .iter()
            .filter(|(_, n)| *n != 0)
            .map(|(s, n)| format!("{n} {s}"))
            .collect();
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!(" ({})", detail.join(", "))
        };
        println(&format!("   {heading:<13}: {total}{detail}"));
    }
    println(&format!(
        "   Closed       : {}",
        backlog.items().filter(|i| i.closed()).count()
    ));
    let blocking = backlog.blocking_bugs(&bf.blocking_severities)?;
    if !blocking.is_empty() {
        println(&format!(
            "\n🐞 Open bugs that come before new work ({}):",
            py_join(", ", &bf.blocking_severities)?
        ));
        for item in &blocking {
            println(&format!("   {}", line_of(item)));
        }
    }
    let untriaged = bf.untriaged(backlog).map_err(uncaught)?;
    if !untriaged.is_empty() {
        println("\n🏷️ Open bugs not triaged yet (they also come before new work):");
        for (item, why) in &untriaged {
            println(&format!("   {}  - {why}", line_of(item)));
        }
    }
    let listed = |i: &Item| {
        blocking.iter().any(|b| std::ptr::eq(*b, i))
            || untriaged.iter().any(|(b, _)| std::ptr::eq(*b, i))
    };
    for (kind, _, heading) in TYPES {
        let items: Vec<&Item> = backlog
            .open_items_of(kind)
            .into_iter()
            .filter(|i| !listed(i))
            .collect();
        if items.is_empty() {
            continue;
        }
        println(&format!("\n{heading}:"));
        for item in items {
            let extra = if item.kind() == Some("epic") {
                format!("  ({})", epic_progress(backlog, item))
            } else {
                String::new()
            };
            println(&format!("   {}{extra}", line_of(item)));
        }
    }
    Ok(())
}

/// One bug per manual test marked Fail in `run`, skipping tests already captured for that run.
fn add_failed_manual_tests(
    bf: &BacklogFile,
    run: &str,
    severity: Option<&str>,
) -> Result<(), RoadmapError> {
    let suite = match quality_assurance::manual_tests::load_suite(Some(&bf.root), None)? {
        Ok(s) => s,
        Err(message) => return error(message),
    };
    let failed: Vec<_> = suite
        .select(Some(run))
        .into_iter()
        .filter(|t| t.status == Some("Fail"))
        .collect();
    if failed.is_empty() {
        println(&format!("ℹ️ No manual test is marked Fail in run {run}."));
        return Ok(());
    }
    let mut backlog = bf.load_or_template()?;
    let captured = backlog
        .items()
        .map(|i| i.get("Description"))
        .collect::<Vec<_>>()
        .join(" ");
    let severity = severity.filter(|s| !s.is_empty());
    let mut added = Vec::new();
    for t in failed {
        let marker = format!("Manual test {} failed in run {run}", t.id);
        if captured.contains(&marker) {
            println(&format!(
                "ℹ️ {} is already in the backlog for run {run}.",
                t.id
            ));
            continue;
        }
        let notes = suite.notes_text(t);
        let feature = t.feature.as_deref().unwrap_or("None");
        let description = if notes.is_empty() {
            format!("{marker} ({feature}).")
        } else {
            format!("{marker} ({feature}). Notes: {notes}")
        };
        let id = backlog.add(
            "bug",
            &format!("{} fails: {}", t.id, t.title),
            &description,
            &format!("{}, {run}, {}", today(), t.id),
            Some(severity.unwrap_or("high")),
            None,
            None,
        )?;
        added.push(id);
    }
    if !added.is_empty() {
        bf.save(&backlog)?;
        for id in &added {
            let item = backlog.find(id)?;
            println(&format!(
                "✅ {}: {} ({}) added to {}",
                item.id,
                item.title,
                item.rank(),
                bf.rel()?
            ));
        }
        if severity.is_none() {
            println(
                "   Severity defaults to high for failed acceptance tests; adjust with 'backlog.py set <ID> --severity <s>'.",
            );
        }
    }
    Ok(())
}

/// The items as they are copied into a plan body.
fn render_items(items: &[&Item]) -> String {
    let blocks: Vec<String> = items
        .iter()
        .map(|item| {
            let mut lines = vec![format!("### {}: {}", item.id, item.title)];
            for name in ["Severity", "Priority", "Found", "Description"] {
                let value = item.get(name);
                if !value.is_empty() {
                    lines.push(format!("- **{name}**: {value}"));
                }
            }
            lines.join("\n")
        })
        .collect();
    blocks.join("\n\n")
}

fn rstrip(s: &str) -> &str {
    crate::backlog::rstrip(s)
}

/// Adds items at the end of the plan's 'Backlog items' section (or a new one at the end).
fn insert_items(body: &str, items_text: &str) -> String {
    let Some(start) = body.find(ITEMS_SECTION) else {
        return format!("{}\n\n{ITEMS_SECTION}\n\n{items_text}\n", rstrip(body));
    };
    // `re.compile(r"^## ", re.M).search(body, pos)`: a line that starts with "## ".
    let pos = start + ITEMS_SECTION.len();
    let end = body[pos - 1..]
        .match_indices("\n## ")
        .map(|(i, _)| pos - 1 + i + 1)
        .next()
        .unwrap_or(body.len());
    let text = format!(
        "{}\n\n{items_text}\n\n{}",
        rstrip(&body[..end]),
        lstrip(&body[end..])
    );
    format!("{}\n", rstrip(&text))
}

/// Creates a draft plan for open backlog items (or adds them to `into`) and links both ways.
fn promote(
    bf: &BacklogFile,
    ids: &[String],
    title: Option<&str>,
    into: Option<&str>,
    plan_id: Option<&str>,
    version: Option<&str>,
) -> Result<(), RoadmapError> {
    let backlog = bf.load()?;
    let mut items: Vec<&Item> = Vec::new();
    for item_id in ids {
        let item = backlog.find(item_id)?;
        if item.closed() {
            return error(format!(
                "{} is closed ({}); reopen it first",
                item.id,
                item.status()
            ));
        }
        if item.kind() == Some("epic") {
            let members: Vec<&str> = backlog
                .epic_items(&item.id, false)
                .iter()
                .map(|i| i.id.as_str())
                .collect();
            let members = if members.is_empty() {
                "none open".to_string()
            } else {
                members.join(", ")
            };
            return error(format!(
                "{} is an epic; promote its items instead ({members})",
                item.id
            ));
        }
        if let Some(plan) = item.plan() {
            return error(format!(
                "{} is already promoted into plan '{plan}'",
                item.id
            ));
        }
        if items.iter().any(|i| std::ptr::eq(*i, item)) {
            return error(format!("{} is listed twice", item.id));
        }
        items.push(item);
    }
    if items.is_empty() {
        return error("name at least one backlog item");
    }
    let store = PlanStore::new(Some(&bf.root), None)?;
    let all_bugs = items.iter().all(|i| i.kind() == Some("bug"));
    let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();

    let plan: Plan = match into.filter(|i| !i.is_empty()) {
        Some(into) => {
            let Some(mut plan) = store.get(into)? else {
                return error(format!("plan '{into}' not found in {}", store.plans_dir));
            };
            let status = plan.meta.get_or_none("status").clone();
            if status.py_eq(&Value::str("completed")) || status.py_eq(&Value::str("superseded")) {
                return error(format!(
                    "plan '{into}' is {}; promote into an open plan",
                    status.py_str()
                ));
            }
            let mut listed: Vec<Value> =
                plan.backlog_items()?.into_iter().map(Value::Str).collect();
            listed.extend(item_ids.iter().map(Value::str));
            plan.meta.insert("backlog_items", Value::List(listed));
            if plan.kind().py_eq(&Value::str("bug")) && !all_bugs {
                plan.meta.insert("kind", Value::str("feature"));
            }
            plan.meta.insert("updated_at", Value::Str(now_iso()));
            plan.body = insert_items(&plan.body, &render_items(&items));
            plan.add_log(
                store.author(),
                &format!("Backlog items added: {}", item_ids.join(", ")),
            )?;
            store.save(&mut plan)?;
            sync_index(&store, None, false, true)?;
            println(&format!(
                "✅ Added {} to plan '{into}' ({})",
                item_ids.join(", "),
                plan.path
            ));
            plan
        }
        None => {
            let title = match title.filter(|t| !t.is_empty()) {
                Some(t) => t.to_string(),
                None if items.len() > 1 => return error("promoting several items needs --title"),
                None => items[0].title.clone(),
            };
            let (kind, template) = if all_bugs {
                ("bug", BUG_PLAN_TEMPLATE)
            } else {
                ("feature", FEATURE_PLAN_TEMPLATE)
            };
            let body = template
                .replace("{title}", &title)
                .replace("{items}", &render_items(&items));
            let backlog_items = item_ids.join(",");
            save_plan(
                &store,
                &Options {
                    title: &title,
                    content: &body,
                    plan_id,
                    session_id: None,
                    version,
                    tags: None,
                    milestones: None,
                    status: "draft",
                    parent_plan_id: None,
                    supersedes: None,
                    kind: Some(kind),
                    backlog_items: Some(&backlog_items),
                    spec_dir: None,
                },
            )?
        }
    };
    let pid = plan
        .meta
        .get("plan_id")
        .cloned()
        .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?
        .py_str();
    let status = plan.meta.get_or_none("status").clone();
    for message in sync_plan_items(&bf.root, &pid, &status, &plan.backlog_items()?)? {
        println(&message);
    }
    Ok(())
}

fn dumps_list(items: &[&Item]) -> Result<String, Crash> {
    let list = items.iter().map(|i| Value::Dict(i.to_dict())).collect();
    json::dumps(&Value::List(list), Some(2))
}

fn cmd(args: &Parsed) -> Result<u8, RoadmapError> {
    let command = args.value("command").unwrap_or("");
    let bf = BacklogFile::new(args.value("dir"))?;

    if command == "validate" {
        if !bf.exists() {
            println(&format!(
                "ℹ️ No backlog at {} (nothing to validate).",
                bf.rel()?
            ));
            return Ok(0);
        }
        let backlog = bf.load()?;
        let problems = backlog.validate();
        for p in &problems {
            println(&format!("❌ {p}"));
        }
        for w in backlog.warnings() {
            println(&format!("⚠️ {w}"));
        }
        println(&format!(
            "\n{} backlog: {} item(s), {} problem(s).",
            if problems.is_empty() { "✅" } else { "❌" },
            backlog.items().count(),
            problems.len()
        ));
        return Ok(u8::from(!problems.is_empty()));
    }

    if command == "add" && args.flag("from_failed") {
        let Some(run) = args.value("run").filter(|r| !r.is_empty()) else {
            return error("--from-failed needs --run <rc>");
        };
        add_failed_manual_tests(&bf, run, args.value("severity"))?;
        return Ok(0);
    }

    if command == "add" {
        let (Some(kind), Some(title), Some(description)) = (
            args.value("type"),
            args.value("title"),
            args.value("description"),
        ) else {
            return error(
                "add needs --type, --title and --description (or --from-failed --run <rc>)",
            );
        };
        let mut backlog = bf.load_or_template()?;
        let found = match args.value("found_in").map(strip).filter(|f| !f.is_empty()) {
            Some(f) => format!("{}, {f}", today()),
            None => today(),
        };
        let id = backlog.add(
            kind,
            title,
            description,
            &found,
            args.value("severity"),
            args.value("priority"),
            args.value("epic"),
        )?;
        bf.save(&backlog)?;
        let item = backlog.find(&id)?;
        println(&format!(
            "✅ {}: {} ({}) added to {}",
            item.id,
            item.title,
            item.rank(),
            bf.rel()?
        ));
        if item.kind() == Some("bug") && contains(&bf.blocking_severities, &item.rank())? {
            println(&format!(
                "   🐞 {} bug: it comes before new features, improvements and adjustments.",
                item.rank()
            ));
        } else if item.kind() == Some("bug") && bf.bug_deadline.py_eq(&Value::str("next_release")) {
            let next = bf
                .triage(&backlog, &[])
                .map_err(uncaught)?
                .next_release
                .unwrap_or_else(|| "<vX.Y.Z>".to_string());
            println(&format!(
                "   ⏳ {} bug: schedule it in the next release at the latest: roadmap.py assign {} --release {next}",
                item.rank(),
                item.id
            ));
        }
        return Ok(0);
    }

    if !bf.exists() {
        if command == "list" || command == "summary" {
            if args.flag("json") {
                if command == "list" {
                    println("[]");
                } else {
                    let empty = Backlog::parse("");
                    let data = summary_data(&empty, &bf.blocking_severities, &[])?;
                    println(&json::dumps_ascii(&Value::Dict(data), None)?);
                }
            } else {
                println(&format!("ℹ️ No backlog at {} yet.", bf.rel()?));
            }
            return Ok(0);
        }
        return error(format!("no backlog at {}", bf.rel()?));
    }
    let mut backlog = bf.load()?;

    match command {
        "summary" => {
            if args.flag("json") {
                let untriaged = bf.untriaged(&backlog).map_err(uncaught)?;
                let data = summary_data(&backlog, &bf.blocking_severities, &untriaged)?;
                println(&json::dumps_ascii(&Value::Dict(data), Some(2))?);
            } else {
                print_summary(&backlog, &bf)?;
            }
            return Ok(0);
        }
        "list" => {
            let all: Vec<&Item> = if args.flag("all") {
                backlog.items().collect()
            } else {
                backlog.open_items()
            };
            let kind = args.value("type").filter(|t| !t.is_empty());
            let status = args.value("status").filter(|s| !s.is_empty());
            let epic = args.value("epic").filter(|e| !e.is_empty()).map(upper);
            let items: Vec<&Item> = all
                .into_iter()
                .filter(|i| kind.is_none() || i.kind() == kind)
                .filter(|i| status.is_none_or(|s| i.status() == s))
                .filter(|i| epic.is_none() || i.epic() == epic)
                .collect();
            if args.flag("json") {
                println(&dumps_list(&items)?);
            } else {
                for item in &items {
                    println(&line_of(item));
                }
                if items.is_empty() {
                    println("ℹ️ No matching items.");
                }
            }
            return Ok(0);
        }
        "show" => {
            let item = backlog.find(args.value("id").unwrap_or(""))?;
            if args.flag("json") {
                println(&json::dumps(&Value::Dict(item.to_dict()), Some(2))?);
            } else {
                println(&format!("{}: {}", item.id, item.title));
                for (name, _) in &item.fields {
                    println(&format!("  {name}: {}", item.get(name)));
                }
            }
            return Ok(0);
        }
        "promote" => {
            promote(
                &bf,
                &args.list("ids"),
                args.value("title"),
                args.value("into"),
                args.value("id"),
                args.value("version"),
            )?;
            return Ok(0);
        }
        _ => {}
    }

    let item_id = args.value("id").unwrap_or("");
    let message = match command {
        "set" => {
            let id = backlog.update(
                item_id,
                args.value("title"),
                args.value("description"),
                args.value("severity"),
                args.value("priority"),
                args.value("status"),
                args.value("plan"),
                args.value("epic"),
            )?;
            let item = backlog.find(&id)?;
            format!(
                "✅ {} updated: [{}] ({}) {}",
                item.id,
                item.status(),
                item.rank(),
                item.title
            )
        }
        "close" => {
            let closed = match args.value("ref").filter(|r| !r.is_empty()) {
                Some(r) => format!("{}, {}", today(), strip(r)),
                None => today(),
            };
            let id = backlog.find(item_id)?.id.clone();
            backlog.close(&id, "done", &closed)?;
            format!("✅ {id} closed as done")
        }
        "drop" => {
            let reason = strip(args.value("reason").unwrap_or(""));
            if reason.is_empty() {
                return error("dropping an item needs a reason");
            }
            let id = backlog.find(item_id)?.id.clone();
            backlog.close(&id, "dropped", &format!("{}, {reason}", today()))?;
            format!("✅ {id} dropped")
        }
        "duplicate" => {
            let original = backlog.find(args.value("of").unwrap_or(""))?.id.clone();
            if original == upper(item_id) {
                return error("an item cannot duplicate itself");
            }
            let id = backlog.find(item_id)?.id.clone();
            backlog.close(
                &id,
                "duplicate",
                &format!("{}, duplicate of {original}", today()),
            )?;
            format!("✅ {id} closed as duplicate of {original}")
        }
        "supersede" => {
            let by: Vec<String> = args
                .value("by")
                .unwrap_or("")
                .split(',')
                .map(|i| strip(i).to_string())
                .filter(|i| !i.is_empty())
                .collect();
            let id = backlog.supersede(item_id, &by, &today())?;
            let item = backlog.find(&id)?;
            let mut message = format!(
                "✅ {id} closed as superseded by {}",
                item.superseded_by().join(", ")
            );
            if let Some(plan) = item.plan() {
                message.push_str(&format!(
                    "\n⚠️ {id} was in plan '{plan}'; update the plan (backlog.py promote <new ids> --into {plan})."
                ));
            }
            message
        }
        "reopen" => {
            let id = backlog.reopen(item_id)?;
            format!("✅ {id} reopened")
        }
        other => return error(format!("unknown command '{other}'")),
    };
    bf.save(&backlog)?;
    println(&message);
    Ok(0)
}

const DIR_HELP: &str = "Project directory (default: current; the Git root is used)";

fn command_parser(name: &str) -> Parser {
    let types: Vec<&str> = TYPES.iter().map(|t| t.0).collect();
    let statuses: Vec<&str> = OPEN_STATUSES
        .iter()
        .chain(CLOSED_STATUSES.iter())
        .copied()
        .collect();
    let p = Parser::new(&format!("backlog.py {name}"), "").option("--dir", DIR_HELP);
    match name {
        "add" => p
            .choice_option("--type", &types, "")
            .option("--title", "")
            .option("--description", "")
            .choice_option("--severity", &SEVERITIES, "Required for bugs")
            .choice_option("--priority", &PRIORITIES, "Other types (default: medium)")
            .option("--epic", "Epic the item belongs to (EPIC-NNN)")
            .option(
                "--found-in",
                "Where it was found: version, pull request, test",
            )
            .flag(
                "--from-failed",
                "Capture the manual tests marked Fail in --run as bugs",
            )
            .option("--run", "Release candidate run, with --from-failed"),
        "list" => p
            .choice_option("--type", &types, "")
            .choice_option("--status", &statuses, "")
            .option("--epic", "Only the items of this epic")
            .flag("--all", "Include closed items")
            .flag("--json", ""),
        "summary" => p.flag("--json", ""),
        "show" => p.positional("id").flag("--json", ""),
        "set" => p
            .positional("id")
            .option("--title", "")
            .option("--description", "")
            .choice_option("--severity", &SEVERITIES, "")
            .choice_option("--priority", &PRIORITIES, "")
            .choice_option("--status", &OPEN_STATUSES, "")
            .option("--plan", "Plan id ('-' clears it)")
            .option("--epic", "Epic id ('-' clears it)"),
        "close" => p
            .positional("id")
            .option("--ref", "Pull request or commit that delivered it"),
        "drop" => p.positional("id").required_option("--reason", ""),
        "duplicate" => p.positional("id").required_option("--of", ""),
        "supersede" => p
            .positional("id")
            .required_option("--by", "Comma-separated ids of the items that replace it"),
        "reopen" => p.positional("id"),
        "promote" => p
            .positional_plus("ids")
            .option(
                "--title",
                "Plan title (default: the item's title; required for several items)",
            )
            .option("--id", "Plan id (default: generated from the title)")
            .option("--version", "Target version")
            .option("--into", "Add the items to this existing plan instead"),
        _ => p,
    }
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "backlog.py",
        "Capture, list and close items of the project backlog",
    )
    .commands(
        "command",
        &[
            ("add", "Capture a new item"),
            ("list", "List items (open ones by default)"),
            (
                "summary",
                "Counts per type and the bugs that come before new work",
            ),
            ("show", "Show one item"),
            ("set", "Change fields of an open item"),
            ("close", "Close an item as done"),
            ("drop", "Close an item that will not be done"),
            ("duplicate", "Close an item as a duplicate of another"),
            (
                "supersede",
                "Close an item replaced by different, better defined items",
            ),
            ("reopen", "Move a closed item back to its section as open"),
            (
                "promote",
                "Create a draft plan for open items (or add them to an open plan)",
            ),
            ("validate", "Check the backlog format"),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    match cmd(&args) {
        Ok(code) => Ok(code),
        Err(RoadmapError::Error(e)) => {
            println(&format!("❌ {e}"));
            Ok(1)
        }
        Err(RoadmapError::Crash(c)) => Err(c),
    }
}
