//! Promoting backlog items into plans, and the items following their plan's status (formerly
//! `plan-memory/tests/test_promotion.py`).

mod common;

use common::{Out, Project, has};
use plan_memory::backlog::Backlog;
use plan_memory::store::{Plan, PlanStore, slugify_id};

fn cmd(p: &Project, command: &str, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = args.to_vec();
    all.extend(["--dir", root.as_str()]);
    p.run("plan-memory", command, &all)
}

fn backlog(p: &Project, args: &[&str]) -> Out {
    cmd(p, "backlog", args)
}

#[track_caller]
fn ok(res: Out) -> Out {
    assert!(res.ok(), "{}", res.all());
    res
}

#[track_caller]
fn fails(res: Out, piece: &str) {
    assert_eq!(res.code, 1, "{}", res.all());
    has(&res.all(), &[piece]);
}

fn project() -> Project {
    let p = Project::new();
    p.git(&["init", "-q"]);
    for (kind, title, desc, severity) in [
        ("bug", "Crash on start", "It crashes.", Some("high")),
        ("bug", "Wrong label", "Typo in a label.", Some("low")),
        ("feature", "Export answers", "Export to Markdown.", None),
    ] {
        let mut args = vec![
            "add",
            "--type",
            kind,
            "--title",
            title,
            "--description",
            desc,
        ];
        if let Some(s) = severity {
            args.extend(["--severity", s]);
        }
        ok(backlog(&p, &args));
    }
    p
}

fn items(p: &Project) -> Backlog {
    Backlog::parse(&p.read("docs/BACKLOG.md"))
}

fn store(p: &Project) -> PlanStore {
    PlanStore::new(Some(&p.root()), None).unwrap_or_else(|_| panic!("store"))
}

fn plan(p: &Project, id: &str) -> Plan {
    store(p)
        .get(id)
        .unwrap_or_else(|_| panic!("plan {id}"))
        .unwrap_or_else(|| panic!("no plan {id}"))
}

fn backlog_items(plan: &Plan) -> Vec<String> {
    plan.backlog_items()
        .unwrap_or_else(|_| panic!("backlog items"))
}

fn meta(plan: &Plan, key: &str) -> String {
    plan.meta
        .get_or_none(key)
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn promote_one_bug_creates_a_draft_bug_plan() {
    let p = project();
    ok(backlog(&p, &["promote", "BUG-001"]));
    let id = format!("{}_crash_on_start", slugify_id(&store(&p).project));
    let plan = plan(&p, &id);
    assert_eq!(plan.kind().as_str(), Some("bug"));
    assert_eq!(meta(&plan, "status"), "draft");
    assert_eq!(backlog_items(&plan), ["BUG-001"]);
    has(
        &plan.body,
        &[
            "## 1. Triage",
            "## 2. Reproduction",
            "### BUG-001: Crash on start",
            "- **Severity**: high",
        ],
    );
    let b = items(&p);
    let item = b.find("BUG-001").unwrap();
    assert_eq!(item.status(), "planned");
    assert_eq!(item.plan(), Some(meta(&plan, "plan_id")));
    assert!(b.validate().is_empty());
}

#[test]
fn promote_mixed_items_needs_a_title_and_makes_a_feature_plan() {
    let p = project();
    fails(
        backlog(&p, &["promote", "BUG-002", "FEAT-001"]),
        "needs --title",
    );
    ok(backlog(
        &p,
        &[
            "promote",
            "BUG-002",
            "FEAT-001",
            "--title",
            "Export polish",
            "--id",
            "export_polish",
        ],
    ));
    let plan = plan(&p, "export_polish");
    assert_eq!(plan.kind().as_str(), Some("feature"));
    assert_eq!(backlog_items(&plan), ["BUG-002", "FEAT-001"]);
    has(&plan.body, &["## 1. Goal and decisions"]);
}

#[test]
fn promote_refuses_planned_closed_and_unknown_items() {
    let p = project();
    ok(backlog(&p, &["promote", "BUG-001", "--id", "crash"]));
    fails(
        backlog(&p, &["promote", "BUG-001"]),
        "already promoted into plan 'crash'",
    );
    ok(backlog(
        &p,
        &["drop", "BUG-002", "--reason", "not reproducible"],
    ));
    fails(backlog(&p, &["promote", "BUG-002"]), "is closed");
    fails(
        backlog(&p, &["promote", "FEAT-009"]),
        "no such backlog item",
    );
}

#[test]
fn promote_into_an_existing_plan() {
    let p = project();
    ok(backlog(&p, &["promote", "BUG-001", "--id", "crash"]));
    ok(backlog(&p, &["promote", "FEAT-001", "--into", "crash"]));
    let plan = plan(&p, "crash");
    assert_eq!(backlog_items(&plan), ["BUG-001", "FEAT-001"]);
    assert_eq!(plan.kind().as_str(), Some("feature"));
    let section = plan.body.split_once("## Backlog items").unwrap().1;
    let section = section.split("\n## ").next().unwrap();
    has(section, &["### BUG-001", "### FEAT-001"]);
    assert_eq!(
        items(&p).find("FEAT-001").unwrap().plan().as_deref(),
        Some("crash")
    );
    fails(
        backlog(&p, &["promote", "BUG-002", "--into", "nope"]),
        "not found",
    );
}

#[test]
fn items_follow_the_plan_status_and_close_when_it_completes() {
    let p = project();
    ok(backlog(
        &p,
        &[
            "promote",
            "BUG-001",
            "BUG-002",
            "--title",
            "Crash fixes",
            "--id",
            "crash",
        ],
    ));
    let body = plan(&p, "crash").body;
    ok(cmd(
        &p,
        "save_plan",
        &[
            "--title",
            "Crash fixes",
            "--content",
            &body,
            "--id",
            "crash",
            "--status",
            "active",
            "--milestones",
            "M1 Fix,M2 Release",
        ],
    ));
    ok(cmd(
        &p,
        "check_milestone",
        &["--id", "crash", "--milestone", "M1 Fix"],
    ));
    assert_eq!(items(&p).find("BUG-001").unwrap().status(), "in_progress");
    ok(cmd(
        &p,
        "check_milestone",
        &["--id", "crash", "--milestone", "M2 Release"],
    ));
    let b = items(&p);
    let closed: Vec<&str> = b.sections[b.sections.len() - 1]
        .iter()
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(closed, ["BUG-001", "BUG-002"]);
    let item = b.find("BUG-001").unwrap();
    assert_eq!(item.status(), "done");
    assert!(item.get("Closed").contains("plan crash"));
    assert!(b.validate().is_empty());
}

#[test]
fn update_plan_status_completed_closes_items() {
    let p = project();
    ok(backlog(&p, &["promote", "FEAT-001", "--id", "export"]));
    ok(cmd(
        &p,
        "update_plan_status",
        &["--id", "export", "--status", "completed"],
    ));
    assert_eq!(items(&p).find("FEAT-001").unwrap().status(), "done");
}

#[test]
fn save_plan_with_backlog_items_links_them() {
    let p = project();
    let res = ok(cmd(
        &p,
        "save_plan",
        &[
            "--title",
            "Manual link",
            "--content",
            "# Manual link\n",
            "--id",
            "manual",
            "--kind",
            "bug",
            "--backlog-items",
            "bug-002,BUG-404",
        ],
    ));
    has(
        &res.all(),
        &["BUG-404 (plan 'manual') is not in docs/BACKLOG.md"],
    );
    let b = items(&p);
    let item = b.find("BUG-002").unwrap();
    assert_eq!(item.status(), "planned");
    assert_eq!(item.plan().as_deref(), Some("manual"));
    let res = cmd(
        &p,
        "save_plan",
        &[
            "--title",
            "Bad",
            "--content",
            "x",
            "--id",
            "bad",
            "--kind",
            "epic",
        ],
    );
    assert_ne!(res.code, 0);
    has(&res.all(), &["kind"]);
}

#[test]
fn list_plans_filters_by_kind() {
    let p = project();
    ok(backlog(&p, &["promote", "BUG-001", "--id", "crash"]));
    ok(cmd(
        &p,
        "save_plan",
        &[
            "--title",
            "Roadmap",
            "--content",
            "# Roadmap\n",
            "--id",
            "roadmap",
        ],
    ));
    let res = ok(cmd(&p, "list_plans", &["--kind", "bug"]));
    let out = res.all();
    has(&out, &["crash", "Kind: bug", "Backlog    : BUG-001"]);
    assert!(!out.contains("roadmap"), "{out}");
}
