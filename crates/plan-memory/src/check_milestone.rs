//! `check_milestone.py`: tracks milestone completion in the plan file's frontmatter
//! (docs/plans/<plan_id>.md) and shows the progress bar and the checklist.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::unicode::{lower, upper};
use kernel::py::{Crash, Value, now_iso, today};

use crate::backlog::sync_plan_items;
use crate::index::sync_index;
use crate::store::{PlanStore, name_of, progress_bar_width};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_milestone.py",
        "Track granular milestone progress in plan files",
    )
    .option(
        "--id",
        "Target Plan ID (default: the project's latest active plan)",
    )
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)")
    .option("--milestone", "Milestone name to mark as completed")
    .flag("--uncheck", "Revert milestone to pending")
    .option("--notes", "Notes or rationale for milestone update")
    .flag("--list", "List milestones and progress without modifying")
    .flag(
        "--auto-complete",
        "Automatically set plan status to completed if all milestones are done",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    check_milestone(
        &store,
        &Options {
            plan_id: args.value("id"),
            milestone_name: args.value("milestone"),
            uncheck: args.flag("uncheck"),
            notes: args.value("notes"),
            list_only: args.flag("list"),
            auto_complete: args.flag("auto_complete"),
        },
    )
}

pub struct Options<'a> {
    pub plan_id: Option<&'a str>,
    pub milestone_name: Option<&'a str>,
    pub uncheck: bool,
    pub notes: Option<&'a str>,
    pub list_only: bool,
    pub auto_complete: bool,
}

/// `name.lower()` of a milestone name.
fn lower_of(v: &Value) -> Result<String, Crash> {
    match v {
        Value::Str(s) => Ok(lower(s)),
        other => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'lower'",
            other.type_name()
        ))),
    }
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

/// `m.get(key)` of a milestone.
fn field(m: &Value, key: &str) -> Value {
    match m {
        Value::Dict(d) => d.get_or_none(key).clone(),
        _ => Value::Null,
    }
}

/// Marks or reverts a milestone, then prints the tracker. Returns the exit code.
pub fn check_milestone(store: &PlanStore, o: &Options) -> Result<u8, Crash> {
    let plan_id = o.plan_id.filter(|p| !p.is_empty());
    let found = match plan_id {
        Some(p) => store.get(p)?,
        None => store.latest()?,
    };
    let Some(mut plan) = found else {
        let target = match plan_id {
            Some(p) => format!("plan_id='{p}'"),
            None => format!("project='{}'", store.project),
        };
        err(&format!(
            "❌ Error: Plan not found for {target} in {}\n",
            store.plans_dir
        ));
        return Ok(1);
    };

    let pid = plan
        .meta
        .get("plan_id")
        .cloned()
        .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?;
    if plan.milestones().is_empty() {
        println(&format!(
            "⚠️ Plan '{}' does not have any milestones defined in its frontmatter.",
            pid.py_str()
        ));
        return Ok(0);
    }

    let mut modified = false;
    let milestone_name = o.milestone_name.filter(|m| !m.is_empty());
    if let (Some(wanted), false) = (milestone_name, o.list_only) {
        let names = plan
            .milestones()
            .iter()
            .map(|m| match m {
                Value::Dict(d) => name_of(d),
                _ => Err(Crash::new("KeyError: 'name'")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let wanted_lower = lower(wanted);
        let mut matched = None;
        for n in &names {
            if lower_of(n)? == wanted_lower {
                matched = Some(n.clone());
                break;
            }
        }
        if matched.is_none() {
            let mut matches = Vec::new();
            for n in &names {
                if lower_of(n)?.contains(&wanted_lower) {
                    matches.push(n.clone());
                }
            }
            if matches.len() == 1 {
                matched = matches.pop();
            } else if matches.len() > 1 {
                err(&format!(
                    "❌ Ambiguous milestone name '{wanted}'. Matches: {}\n",
                    Value::List(matches).py_repr()
                ));
                return Ok(1);
            }
        }
        let Some(matched) = matched else {
            err(&format!(
                "❌ Milestone '{wanted}' not found in plan '{}'.\n",
                pid.py_str()
            ));
            println(&format!(
                "   Available milestones: {}",
                names
                    .iter()
                    .map(Value::py_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            return Ok(1);
        };
        let matched_text = matched.py_str();

        let author = store.author();
        let Some(Value::List(milestones)) = plan.meta.get_mut("milestones") else {
            return Err(Crash::new("KeyError: 'milestones'"));
        };
        let ms = milestones
            .iter_mut()
            .find(|m| field(m, "name").py_eq(&matched))
            .ok_or_else(|| Crash::new("StopIteration"))?;
        let Value::Dict(ms) = ms else {
            return Err(Crash::new("StopIteration"));
        };
        if o.uncheck {
            if ms.get_or_none("done").truthy() {
                ms.insert("done", Value::Bool(false));
                ms.remove("done_at");
                ms.remove("by");
                modified = true;
                println(&format!(
                    "⏳ Reverted milestone to PENDING: '{matched_text}'"
                ));
            } else {
                println(&format!("ℹ️ Milestone '{matched_text}' was not completed."));
            }
        } else if !ms.get_or_none("done").truthy() {
            ms.insert("done", Value::Bool(true));
            ms.insert("done_at", Value::Str(today()));
            ms.insert("by", author.clone());
            modified = true;
            println(&format!(
                "✅ Marked milestone as COMPLETED: '{matched_text}'"
            ));
        } else {
            println(&format!(
                "ℹ️ Milestone '{matched_text}' was already completed."
            ));
        }

        let notes = o.notes.filter(|n| !n.is_empty());
        if modified || notes.is_some() {
            let action = if o.uncheck { "reopened" } else { "completed" };
            let suffix = notes.map(|n| format!(": {n}")).unwrap_or_default();
            plan.add_log(
                author,
                &format!("Milestone '{matched_text}' {action}{suffix}"),
            )?;
            modified = true;
        }
    }

    let total = plan.milestones().len();
    if modified {
        let done = plan.done_milestones()?.len();
        let status = plan.meta.get_or_none("status").clone();
        if done == total {
            let open =
                status.py_eq(&Value::str("active")) || status.py_eq(&Value::str("in_progress"));
            if o.auto_complete || open {
                plan.meta.insert("status", Value::str("completed"));
                plan.add_log_status(
                    store.author(),
                    "All milestones completed",
                    Some(&Value::str("completed")),
                )?;
                println(
                    "🎉 All milestones completed! Plan status automatically updated to COMPLETED.",
                );
            }
        } else if status.py_eq(&Value::str("active")) && done > 0 {
            plan.meta.insert("status", Value::str("in_progress"));
            println("⚡ Plan status updated to IN_PROGRESS.");
        }
        plan.meta.insert("updated_at", Value::Str(now_iso()));
        store.save(&mut plan)?;
        sync_index(store, None, false, true)?;
        let items = plan.backlog_items()?;
        if !items.is_empty() {
            let status = plan.meta.get_or_none("status").clone();
            for message in sync_plan_items(&store.root, &pid.py_str(), &status, &items)? {
                println(&message);
            }
        }
    }

    let meta = &plan.meta;
    let rule = "=".repeat(80);
    println(&rule);
    println(&format!(
        "📊 MILESTONE TRACKER: [{}] {}",
        match meta.get("project") {
            Some(p) => p.py_str(),
            None => store.project.clone(),
        },
        meta.get("title").unwrap_or(&pid).py_str()
    ));
    let version = meta.get_or_none("version");
    println(&format!(
        "   • Plan ID : {} | Status: {} | Version: {}",
        pid.py_str(),
        upper_of(meta.get("status").unwrap_or(&Value::str("active")))?,
        if version.truthy() {
            version.py_str()
        } else {
            "-".to_string()
        }
    ));
    println(&format!(
        "   • Progress: {}",
        progress_bar_width(plan.done_milestones()?.len(), total, 24)
    ));
    println(&format!("   • File    : {}", plan.path));
    println(&"-".repeat(80));
    println("📋 MILESTONE CHECKLIST:");
    for (idx, m) in plan.milestones().iter().enumerate() {
        let done = field(m, "done").truthy();
        let icon = if done { "✅ [X]" } else { "⏳ [ ]" };
        let done_at = field(m, "done_at");
        let extra = if done && done_at.truthy() {
            format!("  ({}, {})", done_at.py_str(), field(m, "by").py_str())
        } else {
            String::new()
        };
        let Value::Dict(d) = m else {
            return Err(Crash::new("KeyError: 'name'"));
        };
        println(&format!(
            "   {icon} {}. {}{extra}",
            idx + 1,
            name_of(d)?.py_str()
        ));
    }
    println(&format!("{rule}\n"));
    if modified {
        println("👉 Commit the updated plan file.\n");
    }
    Ok(0)
}
