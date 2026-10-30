//! `inspect_project.py`: analyzes the workspace to detect the project's tech stack, build
//! tools, test runners, Git health and documentation files, and shows the configuration, the
//! backlog, the roadmap, the product baseline and Spec Kit. Project-agnostic: works across
//! Python, Rust, Node/TypeScript, Go, C++, etc.

use std::path::Path;

use kernel::out::{err, println};
use kernel::project_config::{ProjectConfig, branch_summary, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::fnmatch::glob_in;
use kernel::py::json::dumps_ascii;
use kernel::py::num::strip;
use kernel::py::path::{abspath, basename, exists, isfile, join, relpath};
use kernel::py::subprocess::run_replace;
use kernel::py::{Crash, Dict, Value, iterate, splitlines};
use plan_memory::backlog::BacklogFile;
use plan_memory::roadmap::{RoadmapError, RoadmapFile, current_data};

use crate::speckit_setup::{self, Skills};

/// `str(e)` for an exception the script caught: its message without the type.
fn exc_str(c: &Crash) -> String {
    c.0.split_once(": ")
        .map_or(String::new(), |(_, m)| m.to_string())
}

fn roadmap_exc(e: RoadmapError) -> String {
    match e {
        RoadmapError::Error(m) => m,
        RoadmapError::Crash(c) => exc_str(&c),
    }
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn error(msg: String) -> Value {
    let mut d = Dict::new();
    d.insert("error", Value::Str(msg));
    Value::Dict(d)
}

/// `run_cmd(cmd, cwd)`: the exit code and stdout stripped; -1 and the message when the
/// program cannot be started.
fn run_cmd(cmd: &[&str], cwd: &str) -> (i32, String) {
    let args: Vec<String> = cmd.iter().map(|a| a.to_string()).collect();
    match run_replace(&args, Some(cwd)) {
        Ok((code, stdout, _)) => (code, strip(&stdout).to_string()),
        Err(Ok(msg)) => (-1, msg),
        Err(Err(c)) => (-1, exc_str(&c)),
    }
}

/// Open backlog items per type and the bugs that come before new work.
fn backlog_profile(cwd: &str) -> Value {
    let profile = || -> Result<Dict, RoadmapError> {
        let bf = BacklogFile::new(Some(cwd))?;
        let mut d = Dict::new();
        d.insert("file", Value::Str(bf.rel()?));
        if !bf.exists() {
            d.insert("exists", Value::Bool(false));
            return Ok(d);
        }
        let backlog = bf.load()?;
        d.insert("exists", Value::Bool(true));
        let mut open = Dict::new();
        for kind in ["bug", "feature", "improvement", "adjustment", "epic"] {
            open.insert(kind, Value::int(backlog.open_items_of(kind).len() as i64));
        }
        d.insert("open", Value::Dict(open));
        let blocking = backlog
            .blocking_bugs(&bf.blocking_severities)?
            .into_iter()
            .map(|b| Value::Str(format!("{} ({}) {}", b.id, b.rank(), b.title)))
            .collect();
        d.insert("blocking_bugs", Value::List(blocking));
        let untriaged = bf
            .untriaged(&backlog)?
            .into_iter()
            .map(|(b, why)| {
                let rank = b.rank();
                let rank = if rank.is_empty() { "?" } else { &rank };
                Value::Str(format!("{} ({rank}) {}: {why}", b.id, b.title))
            })
            .collect();
        d.insert("untriaged_bugs", Value::List(untriaged));
        Ok(d)
    };
    profile().map_or_else(|e| error(roadmap_exc(e)), Value::Dict)
}

/// The current release of the roadmap and its open items.
fn roadmap_profile(cwd: &str) -> Value {
    let profile = || -> Result<Dict, RoadmapError> {
        let rf = RoadmapFile::new(Some(cwd))?;
        let mut d = Dict::new();
        d.insert("file", Value::Str(rf.rel()?));
        if !rf.exists() {
            d.insert("exists", Value::Bool(false));
            return Ok(d);
        }
        let roadmap = rf.load(false)?;
        let bf = BacklogFile::new(Some(cwd))?;
        let backlog = if bf.exists() { Some(bf.load()?) } else { None };
        let data = current_data(&roadmap, backlog.as_ref());
        d.insert("exists", Value::Bool(true));
        let field = |v: &Value, k: &str| {
            v.as_dict()
                .map_or(Value::Null, |r| r.get_or_none(k).clone())
                .py_str()
        };
        let release = data.get_or_none("release");
        d.insert(
            "current",
            if release.truthy() {
                Value::Str(format!(
                    "{}: {}",
                    field(release, "version"),
                    field(release, "name")
                ))
            } else {
                Value::Null
            },
        );
        let items = data
            .get_or_none("open_items")
            .as_list()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|i| {
                Value::Str(format!(
                    "{} ({}) {}",
                    field(i, "id"),
                    field(i, "rank"),
                    field(i, "title")
                ))
            })
            .collect();
        d.insert("open_items", Value::List(items));
        let unscheduled = data
            .get_or_none("unscheduled")
            .as_list()
            .map_or(0, Vec::len);
        d.insert("unscheduled", Value::int(unscheduled as i64));
        Ok(d)
    };
    profile().map_or_else(|e| error(roadmap_exc(e)), Value::Dict)
}

/// Phase 0 baseline: which product documents exist.
fn product_profile(cfg: &ProjectConfig) -> Result<Value, Crash> {
    let product = cfg.section("product");
    let root = cfg.root.to_string_lossy();
    let mut d = Dict::new();
    for key in ["vision", "tech_spec"] {
        let file = product.get_or_none(key);
        let exists = file.truthy() && isfile(&architecture_docs::docs::join_value(&root, file)?);
        let mut entry = Dict::new();
        entry.insert("file", file.clone());
        entry.insert("exists", Value::Bool(exists));
        d.insert(key, Value::Dict(entry));
    }
    Ok(Value::Dict(d))
}

fn speckit_profile(skills: &Skills, cwd: &str) -> Value {
    match speckit_setup::status_data(skills, cwd) {
        Ok(d) => Value::Dict(d),
        Err(speckit_setup::Error::Setup(m)) => error(m),
        Err(speckit_setup::Error::Crash(c)) => error(exc_str(&c)),
    }
}

fn push(profile: &mut Dict, key: &str, item: &str) {
    if let Some(Value::List(items)) = profile.get_mut(key) {
        items.push(s(item));
    }
}

fn config_profile(skills: &Skills, cwd: &str, profile: &mut Dict) -> Result<ProjectConfig, Crash> {
    let cfg = load_project_config(Some(cwd)).map_err(|e| e.into_crash())?;
    profile.insert(
        "config_manifest",
        cfg.manifest_path.as_ref().map_or(Value::Null, |p| {
            Value::Str(p.to_string_lossy().into_owned())
        }),
    );
    let name = cfg.get("project_name");
    let name = if name.truthy() {
        name.clone()
    } else {
        Value::Str(basename(&cfg.root))
    };
    profile.insert("project_name", name);
    profile.insert(
        "architecture",
        Value::Dict(cfg.section("architecture").clone()),
    );
    let commands = cfg.get("test_commands");
    let commands = if commands.truthy() {
        iterate(commands)?
    } else {
        Vec::new()
    };
    profile.insert("test_commands", Value::List(commands));
    profile.insert("devops", Value::Dict(cfg.section("devops").clone()));
    let problems = cfg.validate().into_iter().map(Value::Str).collect();
    profile.insert("config_problems", Value::List(problems));
    profile.insert("backlog", backlog_profile(cwd));
    profile.insert("roadmap", roadmap_profile(cwd));
    profile.insert("product", product_profile(&cfg)?);
    profile.insert("speckit", speckit_profile(skills, cwd));
    Ok(cfg)
}

fn first(found: Vec<String>) -> Option<String> {
    found.into_iter().next()
}

pub fn detect_project_profile(skills: &Skills, workspace_dir: Option<&str>) -> Result<Dict, Crash> {
    let cwd = abspath(workspace_dir.unwrap_or(""))
        .to_string_lossy()
        .into_owned();
    let mut profile = Dict::new();
    profile.insert("project_name", Value::Str(basename(Path::new(&cwd))));
    profile.insert("workspace_root", s(&cwd));
    for key in [
        "languages",
        "frameworks",
        "package_manifests",
        "test_runners",
        "build_tools",
    ] {
        profile.insert(key, Value::List(Vec::new()));
    }
    profile.insert("doc_files", Value::Dict(Dict::new()));
    profile.insert("git_status", Value::Dict(Dict::new()));
    profile.insert("config_manifest", Value::Null);
    profile.insert("architecture", Value::Dict(Dict::new()));
    profile.insert("test_commands", Value::List(Vec::new()));
    profile.insert("devops", Value::Dict(Dict::new()));
    profile.insert("config_problems", Value::List(Vec::new()));
    for key in ["backlog", "roadmap", "product", "speckit"] {
        profile.insert(key, Value::Dict(Dict::new()));
    }

    // Project configuration (.dev-cycle.json, shared by every skill)
    let cfg = match config_profile(skills, &cwd, &mut profile) {
        Ok(cfg) => Some(cfg),
        Err(c) => {
            let msg = format!("could not read .dev-cycle.json: {}", exc_str(&c));
            profile.insert("config_problems", Value::List(vec![Value::Str(msg)]));
            None
        }
    };

    let has = |name: &str| exists(&join(&cwd, name));
    // Detect Languages & Manifests
    if has("Cargo.toml") {
        push(&mut profile, "languages", "Rust");
        push(&mut profile, "package_manifests", "Cargo.toml");
        push(&mut profile, "build_tools", "cargo");
        push(&mut profile, "test_runners", "cargo test");
    }
    if has("pyproject.toml") || has("setup.py") || has("requirements.txt") {
        push(&mut profile, "languages", "Python");
        if has("pyproject.toml") {
            push(&mut profile, "package_manifests", "pyproject.toml");
        }
        if has("setup.py") {
            push(&mut profile, "package_manifests", "setup.py");
        }
        push(&mut profile, "build_tools", "pip/build");
        if has("pytest.ini") || has("tests") {
            push(&mut profile, "test_runners", "pytest");
        }
    }
    if has("package.json") {
        push(&mut profile, "languages", "TypeScript/JavaScript");
        push(&mut profile, "package_manifests", "package.json");
        push(&mut profile, "build_tools", "npm/pnpm/yarn");
        push(&mut profile, "test_runners", "npm test");
    }
    if has("go.mod") {
        push(&mut profile, "languages", "Go");
        push(&mut profile, "package_manifests", "go.mod");
        push(&mut profile, "build_tools", "go build");
        push(&mut profile, "test_runners", "go test ./...");
    }

    // Detect Documentation files
    let mut docs = Dict::new();
    if let Some(readme) = first(glob_in(&cwd, "*README*")) {
        docs.insert("user_doc", Value::Str(basename(Path::new(&readme))));
    }
    let tecdoc = first(glob_in(&cwd, "*TECDOC*"))
        .or_else(|| first(glob_in(&join(&cwd, "docs"), "*ARCHITECTURE*")));
    if let Some(t) = tecdoc {
        docs.insert("tec_doc", Value::Str(relpath(&t, &cwd)?));
    }
    let manual = first(glob_in(&join(&cwd, "tests"), "*MANUAL*"))
        .or_else(|| first(glob_in(&cwd, "*MANUAL_TESTS*")));
    if let Some(m) = manual {
        docs.insert("manual_tests", Value::Str(relpath(&m, &cwd)?));
    }

    // Configured documentation paths win over detection
    if let Some(cfg) = cfg.as_ref().filter(|c| c.manifest_path.is_some()) {
        for (section, key) in [
            ("docs", "user_doc"),
            ("docs", "tec_doc"),
            ("tests", "manual_tests"),
        ] {
            let value = cfg.section(section).get_or_none(key);
            if value.truthy() && exists(&architecture_docs::docs::join_value(&cwd, value)?) {
                docs.insert(key, value.clone());
            }
        }
    }
    profile.insert("doc_files", Value::Dict(docs));

    // Configured test commands win over detected runners
    let commands = profile.get_or_none("test_commands").clone();
    if commands.truthy() {
        profile.insert("test_runners", commands);
    }

    // Git Inspection
    let mut git = Dict::new();
    if exists(&join(&cwd, ".git")) {
        let (code, branch) = run_cmd(&["git", "rev-parse", "--abbrev-ref", "HEAD"], &cwd);
        git.insert("branch", s(if code == 0 { &branch } else { "unknown" }));
        let (_, status_out) = run_cmd(&["git", "status", "--porcelain"], &cwd);
        let dirty = splitlines(&status_out)
            .into_iter()
            .filter(|l| !strip(l).is_empty())
            .count();
        git.insert("dirty_files_count", Value::int(dirty as i64));
        git.insert("is_clean", Value::Bool(dirty == 0));
        let (code, last_tag) = run_cmd(&["git", "describe", "--tags", "--abbrev=0"], &cwd);
        git.insert("last_tag", s(if code == 0 { &last_tag } else { "none" }));
        let range = format!("origin/{branch}...{branch}");
        let (code, ahead_behind) = run_cmd(
            &["git", "rev-list", "--left-right", "--count", &range],
            &cwd,
        );
        if code == 0 && ahead_behind.contains('\t') {
            let parts: Vec<&str> = ahead_behind.split('\t').collect();
            git.insert("behind_origin", Value::int_from_text(strip(parts[0])));
            git.insert("ahead_origin", Value::int_from_text(strip(parts[1])));
        }
    }
    profile.insert("git_status", Value::Dict(git));
    Ok(profile)
}

fn joined(value: &Value, empty: &str) -> Result<String, Crash> {
    let text = kernel::py::join(", ", value)?;
    Ok(if text.is_empty() {
        empty.to_string()
    } else {
        text
    })
}

fn or_text(v: &Value, default: &str) -> String {
    if v.truthy() {
        v.py_str()
    } else {
        default.to_string()
    }
}

fn dict(v: &Value) -> Dict {
    v.as_dict().cloned().unwrap_or_default()
}

fn print_profile_report(profile: &Dict) -> Result<(), Crash> {
    let get = |k: &str| profile.get_or_none(k);
    let langs = joined(get("languages"), "Generic / Undetermined")?;
    let manifests = joined(get("package_manifests"), "None")?;
    let runners = joined(get("test_runners"), "None detected")?;
    let docs = dict(get("doc_files"));
    let git = dict(get("git_status"));
    let rule = |c: &str| c.repeat(85);

    println(&rule("="));
    println(&format!(
        "🛡️ DEV-CYCLE PROJECT PROFILE: {}",
        get("project_name").py_str()
    ));
    println(&format!(
        "   • Workspace Root : {}",
        get("workspace_root").py_str()
    ));
    println(&format!("   • Tech Stack     : {langs}"));
    println(&format!("   • Manifests      : {manifests}"));
    println(&format!("   • Test Runners   : {runners}"));
    let arch = dict(get("architecture"));
    let devops = dict(get("devops"));
    println(&format!(
        "   • Config         : {}",
        or_text(get("config_manifest"), "none (.dev-cycle.json recommended)")
    ));
    println(&format!(
        "   • Architecture   : {}",
        or_text(
            arch.get_or_none("style"),
            "not set (architecture.style in .dev-cycle.json)"
        )
    ));
    if !devops.is_empty() {
        let rc = if devops.get_or_none("release_candidates").truthy() {
            "on"
        } else {
            "off"
        };
        println(&format!(
            "   • Branching      : {} (release candidates: {rc})",
            branch_summary(&devops)
        ));
        println(&format!(
            "   • Releases Repo  : {}",
            or_text(devops.get_or_none("releases_repo"), "not set")
        ));
    }
    for problem in get("config_problems")
        .as_list()
        .cloned()
        .unwrap_or_default()
    {
        println(&format!("   ⚠️ Config: {}", problem.py_str()));
    }
    println(&rule("-"));

    let backlog = dict(get("backlog"));
    let b = |k: &str| backlog.get_or_none(k);
    if b("exists").truthy() {
        let counts: Vec<String> = dict(b("open"))
            .str_iter()
            .map(|(kind, n)| format!("{} {kind}", n.py_str()))
            .collect();
        println(&format!(
            "📥 BACKLOG ({}): open {}",
            b("file").py_str(),
            counts.join(", ")
        ));
        for bug in b("blocking_bugs").as_list().cloned().unwrap_or_default() {
            println(&format!("   🐞 Comes before new work: {}", bug.py_str()));
        }
        for bug in b("untriaged_bugs").as_list().cloned().unwrap_or_default() {
            println(&format!(
                "   🏷️ Not triaged (comes before new work): {}",
                bug.py_str()
            ));
        }
        println(
            "   Triage with the user: promote, re-prioritize or drop (plan-memory/backlog.py summary).",
        );
    } else if b("file").truthy() {
        println(&format!(
            "📥 BACKLOG: none yet ({} is created by the first /backlog or backlog.py add)",
            b("file").py_str()
        ));
    } else if b("error").truthy() {
        println(&format!(
            "📥 BACKLOG: could not be read ({})",
            b("error").py_str()
        ));
    }

    let roadmap = dict(get("roadmap"));
    let r = |k: &str| roadmap.get_or_none(k);
    if r("exists").truthy() {
        println(&format!(
            "🗺️ ROADMAP ({}): current release {}",
            r("file").py_str(),
            or_text(r("current"), "none (every release is released)")
        ));
        for item in r("open_items").as_list().cloned().unwrap_or_default() {
            println(&format!("   · {}", item.py_str()));
        }
        if r("unscheduled").truthy() {
            println(&format!(
                "   ℹ️ {} open backlog item(s) in no release (plan-memory/roadmap.py current)",
                r("unscheduled").py_str()
            ));
        }
    } else if r("file").truthy() {
        println(&format!(
            "🗺️ ROADMAP: none yet ({}; Phase 0 or plan-memory/roadmap.py add-release)",
            r("file").py_str()
        ));
    } else if r("error").truthy() {
        println(&format!(
            "🗺️ ROADMAP: could not be read ({})",
            r("error").py_str()
        ));
    }

    let product = dict(get("product"));
    if !product.is_empty() {
        let docs_state: Vec<(String, bool)> = product
            .str_iter()
            .map(|(_, d)| {
                let d = dict(d);
                (
                    d.get_or_none("file").py_str(),
                    d.get_or_none("exists").truthy(),
                )
            })
            .collect();
        let state: Vec<String> = docs_state
            .iter()
            .map(|(f, e)| format!("{f} {}", if *e { "✅" } else { "❌" }))
            .collect();
        let suffix = if docs_state.iter().all(|(_, e)| *e) {
            ""
        } else {
            " (Phase 0: product-inception)"
        };
        println(&format!(
            "🌱 PRODUCT BASELINE: {}{suffix}",
            state.join(", ")
        ));
    }

    let speckit = dict(get("speckit"));
    let k = |key: &str| speckit.get_or_none(key);
    if k("enabled").truthy() {
        println(&format!(
            "🧩 SPEC KIT: {} ({}, branches {}/NNN-slug)",
            or_text(k("pinned"), "version not set"),
            k("integration").py_str(),
            or_text(k("branch_prefix"), "?")
        ));
        for problem in k("problems").as_list().cloned().unwrap_or_default() {
            println(&format!("   ⚠️ {}", problem.py_str()));
        }
    }
    println(&rule("-"));

    let doc = |key: &str, missing: &str| docs.get(key).map_or(missing.to_string(), Value::py_str);
    println("📚 DOCUMENTATION STANDARD STATUS (Dual-Doc + Manual Tests):");
    println(&format!(
        "   • UserDoc (Public Guide) : {}",
        doc("user_doc", "❌ MISSING (README.md recommended)")
    ));
    println(&format!(
        "   • TecDoc (Architecture)  : {}",
        doc("tec_doc", "❌ MISSING (TECDOC.md recommended)")
    ));
    println(&format!(
        "   • Manual Tests Suite     : {}",
        doc(
            "manual_tests",
            "❌ MISSING (tests/MANUAL_TESTS.md recommended)"
        )
    ));
    println(&rule("-"));

    if !git.is_empty() {
        let g = |key: &str| git.get(key);
        let clean = if g("is_clean").is_some_and(Value::truthy) {
            "🟢 Clean".to_string()
        } else {
            format!(
                "⚠️ {} uncommitted changes",
                g("dirty_files_count").map_or("0".to_string(), Value::py_str)
            )
        };
        println("🌿 GIT REPOSITORY HEALTH:");
        println(&format!(
            "   • Active Branch  : {}",
            g("branch").map_or("unknown".to_string(), Value::py_str)
        ));
        println(&format!("   • Working Tree   : {clean}"));
        println(&format!(
            "   • Current Tag    : {}",
            g("last_tag").map_or("none".to_string(), Value::py_str)
        ));
        if let (Some(ahead), Some(behind)) = (g("ahead_origin"), g("behind_origin")) {
            println(&format!(
                "   • Remote Sync    : Ahead: {} | Behind: {}",
                ahead.py_str(),
                behind.py_str()
            ));
        }
    } else {
        println("🌿 GIT REPOSITORY: ❌ Not a Git workspace.");
    }
    println(&format!("{}\n", rule("=")));
    Ok(())
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "inspect_project.py",
        "Inspect workspace project profile and dev-cycle readiness",
    )
    .option("--dir", "Workspace directory (defaults to current dir)")
    .flag("--json", "Output raw JSON profile");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let Some(skills) = Skills::find() else {
        err(&format!(
            "dev-cycle: cannot find the skills folder; set {} to it\n",
            kernel::skills::ENV
        ));
        return Ok(2);
    };
    let dir = args.value("dir").filter(|d| !d.is_empty());
    let profile = detect_project_profile(&skills, dir)?;
    if args.flag("json") {
        println(&dumps_ascii(&Value::Dict(profile), Some(2))?);
    } else {
        print_profile_report(&profile)?;
    }
    Ok(0)
}
