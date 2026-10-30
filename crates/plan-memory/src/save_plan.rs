//! `save_plan.py`: saves an uncompressed, comprehensive plan as
//! `<repo>/docs/plans/<plan_id>.md` (YAML frontmatter + Markdown body) and indexes it.
//! `--supersedes` marks the older plan superseded. Saving an existing plan ID updates it,
//! keeping its author, creation date and milestone progress.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::path::{isdir, join};
use kernel::py::unicode::upper;
use kernel::py::{Crash, Dict, Value, join as py_join, now_iso, read_text};

use crate::backlog::sync_plan_items;
use crate::index::sync_index;
use crate::store::{KINDS, Plan, PlanStore, STATUSES, slugify_id, split_csv};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "save_plan.py",
        "Save an uncompressed architecture plan or roadmap to docs/plans",
    )
    .required_option("--title", "Plan title / headline")
    .option("--content", "Inline markdown content")
    .option("--file", "Path to markdown/text file containing the plan")
    .option(
        "--project",
        "Project name (default: .dev-cycle.json project_name or repository folder name)",
    )
    .option(
        "--dir",
        "Project directory (default: current; the Git root is used)",
    )
    .option(
        "--id",
        "Explicit unique Plan ID (auto-generated if omitted)",
    )
    .option(
        "--session-id",
        "Conversation ID where the plan was conceived",
    )
    .option("--version", "Target software version (e.g. v0.30.37)")
    .option("--tags", "Comma-separated keywords/tags")
    .option("--milestones", "Comma-separated milestone targets")
    .choice_option("--status", &STATUSES, "Plan status")
    .option("--parent-id", "Parent plan ID for hierarchical breakdown")
    .option(
        "--supersedes",
        "Older Plan ID that this new plan supersedes",
    )
    .choice_option("--kind", &KINDS, "Plan kind (default: feature)")
    .option(
        "--backlog-items",
        "Comma-separated backlog item ids the plan delivers",
    )
    .option(
        "--spec-dir",
        "Spec Kit folder with the plan's detail (e.g. specs/001-photo-upload)",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    let content = match (args.value("file"), args.value("content")) {
        (Some(f), _) if !f.is_empty() => read_text(std::path::Path::new(f))?,
        (_, Some(c)) if !c.is_empty() => c.to_string(),
        _ => {
            err("❌ Error: You must supply either --file <path> or --content <text>\n");
            return Ok(1);
        }
    };

    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    save_plan(
        &store,
        &Options {
            title: args.value("title").unwrap_or_default(),
            content: &content,
            plan_id: args.value("id"),
            session_id: args.value("session_id"),
            version: args.value("version"),
            tags: args.value("tags"),
            milestones: args.value("milestones"),
            status: args.value("status").unwrap_or("active"),
            parent_plan_id: args.value("parent_id"),
            supersedes: args.value("supersedes"),
            kind: args.value("kind"),
            backlog_items: args.value("backlog_items"),
            spec_dir: args.value("spec_dir"),
        },
    )?;
    Ok(0)
}

pub struct Options<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub plan_id: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub version: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub milestones: Option<&'a str>,
    pub status: &'a str,
    pub parent_plan_id: Option<&'a str>,
    pub supersedes: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub backlog_items: Option<&'a str>,
    pub spec_dir: Option<&'a str>,
}

/// A non-empty option, as `if value:` reads it.
fn given(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.is_empty())
}

/// `m["name"]` of a milestone dict.
fn milestone_name(m: &Value) -> Result<Value, Crash> {
    match m {
        Value::Dict(d) => d
            .get("name")
            .cloned()
            .ok_or_else(|| Crash::new("KeyError: 'name'")),
        other => Err(Crash::new(format!(
            "TypeError: '{}' object is not subscriptable",
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

/// Writes the plan file, then the lineage, the index and the linked backlog items.
pub fn save_plan(store: &PlanStore, o: &Options) -> Result<Plan, Crash> {
    let pid = match given(o.plan_id) {
        Some(p) => p.to_string(),
        None => format!("{}_{}", slugify_id(&store.project), slugify_id(o.title)),
    };
    let author = store.author();
    let ts = now_iso();

    let existing = store.get(&pid)?;
    let (mut meta, old_done, action) = match existing {
        Some(plan) => {
            let mut done: Vec<(String, Value)> = Vec::new();
            for m in plan.milestones() {
                // `{m["name"]: m}`: a later milestone of the same name wins.
                if let Value::Str(name) = milestone_name(&m)? {
                    done.retain(|(n, _)| *n != name);
                    done.push((name, m));
                }
            }
            (plan.meta, done, "updated")
        }
        None => {
            let mut meta = Dict::new();
            meta.insert("plan_id", Value::str(&pid));
            meta.insert("author", author.clone());
            meta.insert("created_at", Value::str(&ts));
            meta.insert("status_log", Value::List(Vec::new()));
            (meta, Vec::new(), "created")
        }
    };

    meta.insert("project", Value::str(&store.project));
    meta.insert("title", Value::str(o.title));
    meta.insert("status", Value::str(o.status));
    meta.insert("updated_at", Value::str(&ts));
    if let Some(v) = given(o.version) {
        meta.insert("version", Value::str(v));
    }
    if let Some(v) = given(o.session_id) {
        meta.insert("session_id", Value::str(v));
    }
    if let Some(tags) = o.tags {
        let list = split_csv(tags).into_iter().map(Value::Str).collect();
        meta.insert("tags", Value::List(list));
    }
    if let Some(v) = given(o.parent_plan_id) {
        meta.insert("parent_plan_id", Value::str(v));
    }
    if let Some(v) = given(o.supersedes) {
        meta.insert("supersedes", Value::str(v));
    }
    if let Some(kind) = given(o.kind) {
        meta.insert("kind", Value::str(kind));
    }
    if let Some(items) = o.backlog_items {
        let list = split_csv(items)
            .iter()
            .map(|i| Value::Str(upper(i)))
            .collect();
        meta.insert("backlog_items", Value::List(list));
    }
    if let Some(dir) = o.spec_dir {
        let dir = kernel::py::num::strip(dir)
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        meta.insert(
            "spec_dir",
            if dir.is_empty() {
                Value::Null
            } else {
                Value::str(&dir)
            },
        );
        if !dir.is_empty() && !isdir(&join(&store.root, &dir)) {
            err(&format!(
                "⚠️ Warning: spec folder '{dir}' does not exist (yet) in {}.\n",
                store.root
            ));
        }
    }
    if let Some(names) = o.milestones {
        let list = split_csv(names)
            .into_iter()
            .map(|name| match old_done.iter().find(|(n, _)| *n == name) {
                Some((_, m)) => m.clone(),
                None => {
                    let mut d = Dict::new();
                    d.insert("name", Value::Str(name));
                    d.insert("done", Value::Bool(false));
                    Value::Dict(d)
                }
            })
            .collect();
        meta.insert("milestones", Value::List(list));
    }
    for key in [
        "version",
        "session_id",
        "supersedes",
        "superseded_by",
        "parent_plan_id",
    ] {
        if !meta.contains_key(key) {
            meta.insert(key, Value::Null);
        }
    }
    if !meta.contains_key("tags") {
        meta.insert("tags", Value::List(Vec::new()));
    }
    if !meta.contains_key("milestones") {
        meta.insert("milestones", Value::List(Vec::new()));
    }

    let mut plan = Plan {
        meta,
        body: o.content.to_string(),
        path: String::new(),
        sha256: None,
    };
    let status = plan.meta.get_or_none("status").clone();
    plan.add_log_status(author.clone(), &format!("Plan {action}"), Some(&status))?;
    store.save(&mut plan)?;

    if let Some(supersedes) = given(o.supersedes) {
        match store.get(supersedes)? {
            Some(mut old) => {
                let old_status = old
                    .meta
                    .get("status")
                    .cloned()
                    .unwrap_or_else(|| Value::str("active"));
                old.meta.insert("status", Value::str("superseded"));
                old.meta.insert("superseded_by", Value::str(&pid));
                old.meta.insert("updated_at", Value::str(&ts));
                old.add_log_status(
                    author.clone(),
                    &format!("Superseded by {pid}"),
                    Some(&Value::str("superseded")),
                )?;
                store.save(&mut old)?;
                println(&format!(
                    "📦 Plan Lineage: marked '{supersedes}' as SUPERSEDED (was {})",
                    upper_of(&old_status)?
                ));
            }
            None => err(&format!(
                "⚠️ Warning: plan to supersede '{supersedes}' not found in {}.\n",
                store.plans_dir
            )),
        }
    }

    sync_index(store, None, false, true)?;
    let items = plan.backlog_items()?;
    let item_messages = sync_plan_items(&store.root, &pid, &status, &items)?;

    let meta = &plan.meta;
    println(&format!("✅ Plan {action}: {}", plan.path));
    println(&format!("   • Plan ID    : {pid}"));
    println(&format!("   • Project    : {}", store.project));
    println(&format!("   • Title      : {}", o.title));
    println(&format!("   • Kind       : {}", plan.kind().py_str()));
    if !items.is_empty() {
        println(&format!("   • Backlog    : {}", items.join(", ")));
    }
    if let Some(dir) = plan.spec_dir() {
        println(&format!(
            "   • Spec Kit   : {dir} ({} artifact(s))",
            store.spec_files(&plan)?.len()
        ));
    }
    println(&format!(
        "   • Author     : {}",
        meta.get_or_none("author").py_str()
    ));
    let version = meta.get_or_none("version");
    println(&format!(
        "   • Version    : {}",
        if version.truthy() {
            version.py_str()
        } else {
            "-".to_string()
        }
    ));
    println(&format!(
        "   • Status     : {}",
        upper_of(meta.get_or_none("status"))?
    ));
    println(&format!(
        "   • Size       : {} characters (100% uncompressed)",
        o.content.chars().count()
    ));
    if let Some(supersedes) = given(o.supersedes) {
        println(&format!("   • Supersedes : {supersedes}"));
    }
    let milestones = plan.milestones();
    if !milestones.is_empty() {
        let names = milestones
            .iter()
            .map(milestone_name)
            .collect::<Result<Vec<_>, _>>()?;
        println(&format!(
            "   • Milestones : {}",
            py_join(", ", &Value::List(names))?
        ));
    }
    for message in &item_messages {
        println(&format!("   {message}"));
    }
    println(
        "   👉 Commit the plan file with the change it describes (dev-cycle-protocol Phase 3).\n",
    );
    Ok(plan)
}
