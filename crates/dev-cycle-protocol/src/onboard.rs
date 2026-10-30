//! `onboard.py`: the diagnosis behind `/dev-cycle`. Reads the repository (configuration,
//! branches, product baseline, plans, release candidate, bugs, roadmap, Spec Kit) and prints
//! where to start, in the recommended order. It only reads; every change happens after the
//! user's choice.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use architecture_docs::docs::join_value;
use devops_release::branching::{detect, gate_of};
use kernel::out::{err, println};
use kernel::project_config::{ConfigError, MANIFEST_NAME, ProjectConfig, load_project_config};
use kernel::py::argparse::{Parsed, Parser};
use kernel::py::json::{self, LoadError, dumps};
use kernel::py::num::{decimal_value, strip};
use kernel::py::path::{
    abspath, basename, dirname, exists, isdir, isfile, join, makedirs, relpath,
};
use kernel::py::re::Regex;
use kernel::py::subprocess::{run_replace, spawn_error, which};
use kernel::py::unicode::lower;
use kernel::py::{Crash, Dict, Value, repr_str, splitlines, write_text};
use plan_memory::backlog::{Backlog, BacklogFile};
use plan_memory::roadmap::{RoadmapError, RoadmapFile, current_data};
use plan_memory::store::PlanStore;
use quality_assurance::manual_tests::load_suite;

use crate::onboard_rules::recommendation;
use crate::pr_model::{needs_action, summary_line};
use crate::pr_providers::{Fail, cli_available};
use crate::pr_sync::{Context, pending_of};
use crate::speckit_setup::{self, Skills};
use crate::{constitution, inspect_project};

const SOURCE_EXT: [&str; 22] = [
    ".py", ".rs", ".go", ".ts", ".tsx", ".js", ".jsx", ".java", ".kt", ".cs", ".cpp", ".cc", ".c",
    ".h", ".hpp", ".swift", ".rb", ".php", ".dart", ".scala", ".vue", ".svelte",
];
const SKIP_DIRS: [&str; 17] = [
    ".git",
    ".specify",
    ".claude",
    ".github",
    "node_modules",
    "docs",
    "tests",
    "test",
    "specs",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    "target",
    ".idea",
    ".vscode",
];
const TEST_COMMANDS: [(&str, &str); 4] = [
    ("pytest", "python -m pytest"),
    ("cargo test", "cargo test"),
    ("npm test", "npm test"),
    ("go test ./...", "go test ./..."),
];
const VERSION_FILES: [&str; 6] = [
    "pyproject.toml",
    "package.json",
    "Cargo.toml",
    "setup.cfg",
    "VERSION",
    "version.txt",
];

static RC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"^(v\d+\.\d+\.\d+)-rc\.(\d+)$", ""));

/// `str(e)` for an exception the script caught: its message without the type.
fn exc_str(c: &Crash) -> String {
    c.0.split_once(": ")
        .map_or(String::new(), |(_, m)| m.to_string())
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

/// `d.get(key) or default`.
fn or(d: &Dict, key: &str, default: &str) -> Value {
    match d.get(key) {
        Some(v) if v.truthy() => v.clone(),
        _ => s(default),
    }
}

fn config(root: &str) -> Result<ProjectConfig, Crash> {
    load_project_config(Some(root)).map_err(ConfigError::into_crash)
}

/// `git(root, *args)`: the exit code and stdout stripped; 127 when Git cannot be started.
fn git(root: &str, args: &[Value]) -> Result<(i32, String), Crash> {
    let mut cmd = vec!["git".to_string()];
    for a in args {
        match a {
            Value::Str(t) => cmd.push(t.clone()),
            other => {
                return Err(Crash::new(format!(
                    "TypeError: expected str, bytes or os.PathLike object, not {}",
                    other.type_name()
                )));
            }
        }
    }
    if !exists(root) {
        return Ok((127, String::new()));
    }
    if !isdir(root) {
        return Err(Crash::new(format!(
            "NotADirectoryError: [Errno 20] Not a directory: {}",
            repr_str(root)
        )));
    }
    match run_replace(&cmd, Some(root)) {
        Ok((code, out, _)) => Ok((code, strip(&out).to_string())),
        Err(Ok(_)) => Ok((127, String::new())),
        Err(Err(c)) => Err(c),
    }
}

fn git_str(root: &str, args: &[&str]) -> Result<(i32, String), Crash> {
    git(root, &args.iter().map(|a| s(a)).collect::<Vec<_>>())
}

/// Git repositories directly under a folder that is not one itself (a cloud session that
/// starts in the home folder).
fn child_repos(folder: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
        .into_iter()
        .filter(|n| !n.starts_with('.') && exists(&join(&join(folder, n), ".git")))
        .map(|n| join(folder, &n))
        .collect()
}

/// (root, repositories): the repository holding the start folder; outside a repository, the
/// only repository under it. With more than one, root is `None` and the user chooses
/// (`--dir`); with none, the folder itself.
fn resolve_root(start: &str) -> Result<(Option<String>, Vec<String>), Crash> {
    let start = abspath(start).to_string_lossy().into_owned();
    let (code, out) = git_str(&start, &["rev-parse", "--show-toplevel"])?;
    if code == 0 && !out.is_empty() {
        return Ok((
            Some(abspath(&out).to_string_lossy().into_owned()),
            Vec::new(),
        ));
    }
    let repos = child_repos(&start);
    if repos.len() == 1 {
        return Ok((Some(repos[0].clone()), repos));
    }
    Ok((if repos.is_empty() { Some(start) } else { None }, repos))
}

/// `os.path.splitext(name)[1]`.
fn ext(name: &str) -> &str {
    let Some(dot) = name.rfind('.') else {
        return "";
    };
    if name[..dot].chars().any(|c| c != '.') {
        &name[dot..]
    } else {
        ""
    }
}

/// Any source file outside the folders that hold no product code (`os.walk`, without
/// following links into folders).
fn has_code(root: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    let mut dirs = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let is_link = entry.file_type().is_ok_and(|t| t.is_symlink());
        if path.is_dir() {
            if !is_link && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                dirs.push(path.to_string_lossy().into_owned());
            }
        } else if SOURCE_EXT.contains(&lower(ext(&name)).as_str()) {
            return true;
        }
    }
    dirs.iter().any(|d| has_code(d))
}

fn branch_exists(root: &str, name: &str) -> Result<bool, Crash> {
    for r in [
        format!("refs/heads/{name}"),
        format!("refs/remotes/origin/{name}"),
    ] {
        if git_str(root, &["rev-parse", "--verify", "--quiet", &r])?.0 == 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Local branches and the branches of `origin` (as origin/<name>).
fn branch_names(root: &str) -> Result<Vec<String>, Crash> {
    let (code, out) = git_str(
        root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes/origin",
        ],
    )?;
    if code != 0 {
        return Ok(Vec::new());
    }
    Ok(splitlines(&out)
        .into_iter()
        .filter(|n| !strip(n).is_empty() && *n != "origin" && *n != "origin/HEAD")
        .map(str::to_string)
        .collect())
}

fn detect_branching(root: &str, production: &Value) -> Result<Dict, Crash> {
    Ok(detect(&branch_names(root)?, production))
}

/// A `.dev-cycle.json` filled from what can be detected. The architecture style is left out:
/// it is chosen with software-design.
fn detect_config(skills: &Skills, root: &str) -> Result<Dict, Crash> {
    let profile = inspect_project::detect_project_profile(skills, Some(root))?;
    let mut production = "main";
    for name in ["main", "master"] {
        if branch_exists(root, name)? {
            production = name;
            break;
        }
    }
    let mut data = Dict::new();
    data.insert("project_name", s(&basename(Path::new(root))));
    let mut commands = Vec::new();
    for r in profile
        .get_or_none("test_runners")
        .as_list()
        .cloned()
        .unwrap_or_default()
    {
        if !r.is_hashable() {
            return Err(Crash::new(format!(
                "TypeError: unhashable type: '{}'",
                r.type_name()
            )));
        }
        if let Some((_, c)) = TEST_COMMANDS.iter().find(|(k, _)| r.as_str() == Some(k)) {
            commands.push(s(c));
        }
    }
    if !commands.is_empty() {
        data.insert("test_commands", Value::List(commands));
    }
    let docs = profile
        .get_or_none("doc_files")
        .as_dict()
        .cloned()
        .unwrap_or_default();
    if docs.get_or_none("user_doc").truthy() || docs.get_or_none("tec_doc").truthy() {
        let mut d = Dict::new();
        for k in ["user_doc", "tec_doc"] {
            if docs.get_or_none(k).truthy() {
                d.insert(k, docs.get_or_none(k).clone());
            }
        }
        data.insert("docs", Value::Dict(d));
    }
    if docs.get_or_none("manual_tests").truthy() {
        let mut d = Dict::new();
        d.insert("manual_tests", docs.get_or_none("manual_tests").clone());
        data.insert("tests", Value::Dict(d));
    }
    let found = detect_branching(root, &s(production))?;
    let model = or(&found, "model", "integration");
    let mut devops = Dict::new();
    devops.insert("branching", model.clone());
    if model.as_str() == Some("integration") {
        devops.insert(
            "integration_branch",
            or(&found, "integration_branch", "dev"),
        );
    }
    devops.insert("production_branch", s(production));
    let files = VERSION_FILES
        .iter()
        .filter(|f| isfile(&join(root, f)))
        .map(|f| s(f))
        .collect();
    devops.insert("version_files", Value::List(files));
    data.insert("devops", Value::Dict(devops));
    Ok(data)
}

/// The part of `proposal` that `existing` does not set (one level into sections).
fn missing_keys(existing: &Dict, proposal: &Dict) -> Dict {
    let mut out = Dict::new();
    for (key, value) in proposal.iter() {
        match (existing.get_value(key), value) {
            (None, _) => out.insert_value(key.clone(), value.clone()),
            (Some(Value::Dict(have)), Value::Dict(add)) => {
                let sub: Dict = add
                    .iter()
                    .filter(|(k, _)| have.get_value(k).is_none())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !sub.is_empty() {
                    out.insert_value(key.clone(), Value::Dict(sub));
                }
            }
            _ => {}
        }
    }
    out
}

/// (merged, conflicts): additions are only added; a key the file already sets is never
/// changed.
fn merge_config(existing: &Dict, additions: &Dict) -> (Dict, Vec<String>) {
    let mut merged = existing.clone();
    let mut conflicts = Vec::new();
    for (key, value) in additions.iter() {
        let name = key.py_str();
        let Some(current) = merged.get_value(key).cloned() else {
            merged.insert_value(key.clone(), value.clone());
            continue;
        };
        match (current, value) {
            (Value::Dict(mut section), Value::Dict(add)) => {
                for (k, v) in add.iter() {
                    match section.get_value(k) {
                        None => section.insert_value(k.clone(), v.clone()),
                        Some(have) if !have.py_eq(v) => {
                            conflicts.push(format!("{name}.{}", k.py_str()));
                        }
                        Some(_) => {}
                    }
                }
                merged.insert_value(key.clone(), Value::Dict(section));
            }
            (current, value) => {
                if !current.py_eq(value) {
                    conflicts.push(name);
                }
            }
        }
    }
    (merged, conflicts)
}

fn manifest_file(cfg: &ProjectConfig, root: &str) -> String {
    cfg.manifest_path.as_ref().map_or_else(
        || join(root, MANIFEST_NAME),
        |p| p.to_string_lossy().into_owned(),
    )
}

fn propose_config(skills: &Skills, root: &str) -> Result<Dict, Crash> {
    let cfg = config(root)?;
    let proposal = missing_keys(&cfg.raw, &detect_config(skills, root)?);
    let mut d = Dict::new();
    d.insert("file", s(&manifest_file(&cfg, root)));
    d.insert("exists", Value::Bool(cfg.manifest_path.is_some()));
    d.insert("additions", Value::Dict(proposal));
    let production = or(cfg.section("devops"), "production_branch", "main");
    d.insert(
        "branching",
        Value::Dict(detect_branching(root, &production)?),
    );
    Ok(d)
}

/// Writes the confirmed additions. Returns (path, added keys); a conflict is a `ValueError`.
fn write_config(root: &str, additions: &Dict) -> Result<(String, Vec<String>), Crash> {
    let cfg = config(root)?;
    let existing = &cfg.raw;
    let (merged, conflicts) = merge_config(existing, additions);
    if !conflicts.is_empty() {
        return Err(Crash::new(format!(
            "ValueError: the file already sets {}; edit those keys by hand",
            conflicts.join(", ")
        )));
    }
    let path = manifest_file(&cfg, root);
    let merged = Value::Dict(merged);
    write_text(Path::new(&path), &format!("{}\n", dumps(&merged, Some(2))?))?;
    let Value::Dict(merged) = merged else {
        unreachable!()
    };
    let added = missing_keys(existing, &merged)
        .iter()
        .map(|(k, v)| match v {
            Value::Dict(d) => {
                let keys: Vec<String> = d.keys().map(Value::py_str).collect();
                format!("{}.{{{}}}", k.py_str(), keys.join(", "))
            }
            _ => k.py_str(),
        })
        .collect();
    Ok((path, added))
}

/// The paths (relative to the root) that the project's ignore rules keep out of Git. Files
/// already tracked are never reported: Git keeps versioning them.
fn ignored(root: &str, paths: &[String]) -> Result<Vec<String>, Crash> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if !isdir(root) {
        return Err(Crash::new(if exists(root) {
            format!(
                "NotADirectoryError: [Errno 20] Not a directory: {}",
                repr_str(root)
            )
        } else {
            format!(
                "FileNotFoundError: [Errno 2] No such file or directory: {}",
                repr_str(root)
            )
        }));
    }
    let child = Command::new("git")
        .args(["check-ignore", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let (not_found, msg) = spawn_error(&e, "git");
            return Err(Crash::new(if not_found {
                format!("FileNotFoundError: {msg}")
            } else {
                msg
            }));
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(paths.join("\n").as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| Crash::new(format!("OSError: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let found: Vec<String> = splitlines(&text)
        .into_iter()
        .filter(|l| !strip(l).is_empty())
        .map(|l| strip(l).replace('\\', "/"))
        .collect();
    Ok(paths
        .iter()
        .filter(|p| found.contains(p))
        .cloned()
        .collect())
}

/// The `.gitignore` line that lets `path` back in: its first folder, or the file at the root.
fn gitignore_entry(path: &str) -> String {
    match path.split_once('/') {
        Some((head, rest)) if !rest.is_empty() => format!("!/{head}/"),
        Some((head, _)) => format!("!/{head}"),
        None => format!("!/{path}"),
    }
}

/// Files the protocol keeps in the repository, a sample path for each folder, so the ignore
/// rules can be checked before anything is written there.
fn protocol_files(
    root: &str,
    cfg: &ProjectConfig,
    speckit: &Value,
    agent: Option<&str>,
) -> Result<Vec<String>, Crash> {
    let product = cfg.section("product");
    let docs = cfg.section("architecture_docs");
    let mut paths = vec![
        s(MANIFEST_NAME),
        or(cfg.section("backlog"), "file", "docs/BACKLOG.md"),
        or(cfg.section("roadmap"), "file", "docs/ROADMAP.md"),
        Value::Str(format!(
            "{}/plan.md",
            or(cfg.section("plans"), "dir", "docs/plans").py_str()
        )),
        or(product, "vision", "docs/product/VISION.md"),
        or(product, "tech_spec", "docs/product/TECH_SPEC.md"),
        or(docs, "index", "TECDOC.md"),
        Value::Str(format!(
            "{}/subsystem.md",
            or(docs, "architecture_dir", "docs/architecture").py_str()
        )),
        Value::Str(format!(
            "{}/0001-decision.md",
            or(docs, "adr_dir", "docs/adr").py_str()
        )),
        or(
            cfg.section("tests"),
            "manual_tests",
            "tests/MANUAL_TESTS.md",
        ),
    ];
    if !speckit.py_eq(&s("declined")) {
        paths.push(s(".specify/memory/constitution.md"));
        paths.push(s("specs/001-feature/spec.md"));
    }
    if agent == Some("claude-code") {
        paths.push(s(".claude/settings.json"));
    }
    // dict.fromkeys(paths) hashes them all first.
    if let Some(p) = paths.iter().find(|p| !p.is_hashable()) {
        return Err(Crash::new(format!(
            "TypeError: unhashable type: '{}'",
            p.type_name()
        )));
    }
    let mut seen: Vec<&Value> = Vec::new();
    let mut out = Vec::new();
    for p in &paths {
        if seen.iter().any(|q| q.py_eq(p)) {
            continue;
        }
        seen.push(p);
        out.push(relpath(&join_value(root, p)?, root)?.replace('\\', "/"));
    }
    Ok(out)
}

/// `[{path, entry}]` for the protocol's files that would never reach Git (an allowlist
/// `.gitignore`, a `docs/` entry): the decisions they hold would be lost in the next clone.
fn ignored_protocol_files(
    root: &str,
    cfg: &ProjectConfig,
    speckit: &Value,
    agent: Option<&str>,
) -> Result<Vec<Value>, Crash> {
    let files = protocol_files(root, cfg, speckit, agent)?;
    Ok(ignored(root, &files)?
        .into_iter()
        .map(|p| {
            let entry = Value::Str(gitignore_entry(&p));
            let mut d = Dict::new();
            d.insert("path", Value::Str(p));
            d.insert("entry", entry);
            Value::Dict(d)
        })
        .collect())
}

/// The exceptions a command catches: `ValueError` alone, or `OSError` too. Their text.
fn caught(c: &Crash, os_error: bool) -> Option<String> {
    let (kind, _) = c.0.split_once(": ").unwrap_or((&c.0, ""));
    let value_error = matches!(
        kind,
        "ValueError"
            | "UnicodeDecodeError"
            | "UnicodeEncodeError"
            | "UnicodeError"
            | "json.decoder.JSONDecodeError"
    );
    let os = matches!(
        kind,
        "OSError"
            | "FileNotFoundError"
            | "FileExistsError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "PermissionError"
            | "BlockingIOError"
            | "TimeoutError"
            | "InterruptedError"
    );
    (value_error || (os_error && os)).then(|| exc_str(c))
}

fn load_error(e: LoadError) -> Crash {
    match e {
        LoadError::Decode(d) => Crash::new(format!("json.decoder.JSONDecodeError: {d}")),
        LoadError::Crash(c) => c,
    }
}

fn create_integration_branch(root: &str) -> Result<String, Crash> {
    let cfg = config(root)?;
    let devops = cfg.section("devops");
    let model = or(devops, "branching", "integration");
    if !model.py_eq(&s("integration")) {
        return Err(Crash::new(format!(
            "ValueError: the '{}' branching model has no integration branch",
            devops.get_or_none("branching").py_str()
        )));
    }
    let integration = or(devops, "integration_branch", "dev");
    let production = or(devops, "production_branch", "main");
    if branch_exists(root, &integration.py_str())? {
        return Err(Crash::new(format!(
            "ValueError: '{}' already exists",
            integration.py_str()
        )));
    }
    let head = format!("refs/heads/{}", production.py_str());
    let base = if git_str(root, &["rev-parse", "--verify", "--quiet", &head])?.0 == 0 {
        production
    } else {
        Value::Str(format!("origin/{}", production.py_str()))
    };
    let (code, out) = git(root, &[s("branch"), integration.clone(), base.clone()])?;
    if code != 0 {
        return Err(Crash::new(format!(
            "ValueError: could not create '{}' from '{}': {out}",
            integration.py_str(),
            base.py_str()
        )));
    }
    let i = integration.py_str();
    Ok(format!(
        "{i} (from {}); push it with: git push -u origin {i}",
        base.py_str()
    ))
}

fn claude_settings() -> [String; 2] {
    [
        join(".claude", "settings.json"),
        join(".claude", "settings.local.json"),
    ]
}

/// True when the project's Claude Code settings turn off commit and pull request attribution.
fn attribution_hidden(root: &str) -> bool {
    for rel in claude_settings() {
        let Ok(Value::Dict(data)) = json::load_file(Path::new(&join(root, &rel))) else {
            continue;
        };
        match data.get_or_none("attribution") {
            Value::Bool(false) => return true,
            Value::Dict(d)
                if d.get_or_none("commit").py_eq(&s(""))
                    && d.get_or_none("pr").py_eq(&s(""))
                    && matches!(d.get_or_none("sessionUrl"), Value::Bool(false)) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Adds the hidden attribution to `.claude/settings.json`, keeping every other key.
fn set_attribution(root: &str) -> Result<String, Crash> {
    let [settings, _] = claude_settings();
    let path = join(root, &settings);
    let mut data = Dict::new();
    if isfile(&path) {
        match json::load_file(Path::new(&path)).map_err(load_error)? {
            Value::Dict(d) => data = d,
            _ => {
                return Err(Crash::new(format!(
                    "ValueError: {settings} must hold a JSON object"
                )));
            }
        }
    }
    let mut hidden = Dict::new();
    hidden.insert("commit", s(""));
    hidden.insert("pr", s(""));
    hidden.insert("sessionUrl", Value::Bool(false));
    data.insert("attribution", Value::Dict(hidden));
    makedirs(&dirname(&path))?;
    let text = dumps(&Value::Dict(data), Some(2))?;
    write_text(Path::new(&path), &format!("{text}\n"))?;
    Ok(path)
}

/// `str(e)` of what the backlog and roadmap readers raise.
fn roadmap_exc(e: RoadmapError) -> String {
    match e {
        RoadmapError::Error(m) => m,
        RoadmapError::Crash(c) => exc_str(&c),
    }
}

fn read_backlog(
    root: &str,
    state: &mut Dict,
    baseline: &mut Dict,
    backlog: &mut Option<Backlog>,
) -> Result<(), RoadmapError> {
    let bf = BacklogFile::new(Some(root))?;
    if !bf.exists() {
        return Ok(());
    }
    let b = backlog.insert(bf.load()?);
    baseline.insert("epics", Value::int(b.open_items_of("epic").len() as i64));
    let bugs = b
        .blocking_bugs(&bf.blocking_severities)?
        .into_iter()
        .map(|i| Value::Str(format!("{} ({}) {}", i.id, i.rank(), i.title)))
        .collect();
    state.insert("bugs", Value::List(bugs));
    let untriaged = bf
        .untriaged(b)?
        .into_iter()
        .map(|(i, why)| {
            let rank = i.rank();
            let rank = if rank.is_empty() { "?" } else { &rank };
            Value::Str(format!("{} ({rank}) {}: {why}", i.id, i.title))
        })
        .collect();
    state.insert("untriaged", Value::List(untriaged));
    Ok(())
}

fn read_roadmap(
    root: &str,
    state: &mut Dict,
    baseline: &mut Dict,
    backlog: Option<&Backlog>,
) -> Result<(), RoadmapError> {
    let rf = RoadmapFile::new(Some(root))?;
    if !rf.exists() {
        return Ok(());
    }
    baseline.insert("roadmap", Value::Bool(true));
    let data = current_data(&rf.load(false)?, backlog);
    let release = data.get_or_none("release");
    if release.truthy() {
        let field = |v: &Value, k: &str| {
            v.as_dict()
                .map_or(Value::Null, |d| d.get_or_none(k).clone())
                .py_str()
        };
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
        let mut d = Dict::new();
        d.insert(
            "current",
            Value::Str(format!(
                "{} {}",
                field(release, "version"),
                field(release, "name")
            )),
        );
        d.insert("open_items", Value::List(items));
        state.insert("roadmap", Value::Dict(d));
    }
    Ok(())
}

fn read_plans(root: &str) -> Result<Vec<Value>, Crash> {
    let store = PlanStore::new(Some(root), None)?;
    let mut out = Vec::new();
    for p in store.load_all()? {
        let status = p.meta.get_or_none("status");
        if !(status.py_eq(&s("active")) || status.py_eq(&s("in_progress"))) {
            continue;
        }
        let Some(id) = p.meta.get("plan_id") else {
            return Err(Crash::new("KeyError: 'plan_id'"));
        };
        let mut d = Dict::new();
        d.insert("id", id.clone());
        d.insert("title", p.meta.get_or_none("title").clone());
        d.insert("status", status.clone());
        d.insert("done", Value::int(p.done_milestones()?.len() as i64));
        d.insert("total", Value::int(p.milestones().len() as i64));
        out.push(Value::Dict(d));
    }
    Ok(out)
}

/// The state of the repository, as read by the diagnosis (see `onboard_rules`).
fn collect(skills: &Skills, root: &str, agent: Option<&str>) -> Result<Dict, Crash> {
    let mut state = Dict::new();
    let mut findings: Vec<Value> = Vec::new();
    state.insert("findings", Value::List(Vec::new()));
    let is_git = git_str(root, &["rev-parse", "--is-inside-work-tree"])?.0 == 0;
    let mut commits = 0usize;
    let mut commits_text = "0".to_string();
    if is_git {
        let (code, out) = git_str(root, &["rev-list", "--count", "HEAD"])?;
        if code == 0 && !out.is_empty() && out.chars().all(|c| decimal_value(c).is_some()) {
            let digits: String = out
                .chars()
                .filter_map(decimal_value)
                .map(|d| char::from_digit(d, 10).unwrap_or('0'))
                .collect();
            let trimmed = digits.trim_start_matches('0');
            commits_text = if trimmed.is_empty() {
                "0".into()
            } else {
                trimmed.into()
            };
            commits = usize::from(commits_text != "0");
        }
    }
    let branch = if commits > 0 {
        git_str(root, &["rev-parse", "--abbrev-ref", "HEAD"])?.1
    } else {
        String::new()
    };
    let status = if is_git {
        git_str(root, &["status", "--porcelain"])?.1
    } else {
        String::new()
    };
    let dirty = splitlines(&status)
        .into_iter()
        .filter(|l| !strip(l).is_empty())
        .count();
    let mut repo = Dict::new();
    repo.insert("git", Value::Bool(is_git));
    repo.insert("commits", Value::Int(commits_text));
    repo.insert("branch", Value::Str(branch));
    repo.insert("dirty", Value::int(dirty as i64));
    repo.insert("has_code", Value::Bool(has_code(root)));
    state.insert("repo", Value::Dict(repo));

    let cfg = config(root)?;
    let mut conf = Dict::new();
    conf.insert("exists", Value::Bool(cfg.manifest_path.is_some()));
    let problems = cfg
        .validate()
        .into_iter()
        .filter(|p| !p.starts_with("unknown top-level key"))
        .map(Value::Str)
        .collect();
    conf.insert("problems", Value::List(problems));
    state.insert("config", Value::Dict(conf));
    let devops = cfg.section("devops");
    let production = or(devops, "production_branch", "main");
    let mut branches = Dict::new();
    branches.insert(
        "integration",
        devops.get_or_none("integration_branch").clone(),
    );
    branches.insert(
        "production",
        devops.get_or_none("production_branch").clone(),
    );
    let integration_exists =
        commits > 0 && branch_exists(root, &or(devops, "integration_branch", "dev").py_str())?;
    branches.insert("integration_exists", Value::Bool(integration_exists));
    let production_exists = commits > 0 && branch_exists(root, &production.py_str())?;
    branches.insert("production_exists", Value::Bool(production_exists));
    let model = devops.get_or_none("branching");
    branches.insert("model", model.clone());
    branches.insert("decided", Value::Bool(cfg.is_set("devops", "branching")));
    branches.insert("gate", gate_of(model)?);
    let detected = if commits > 0 {
        Value::Dict(detect_branching(root, &production)?)
    } else {
        Value::Null
    };
    branches.insert("detected", detected);
    state.insert("branches", Value::Dict(branches));

    let product = cfg.section("product");
    let index = match cfg.section("architecture_docs").get_or_none("index") {
        v if v.truthy() => v.clone(),
        _ => cfg.section("docs").get_or_none("tec_doc").clone(),
    };
    let mut baseline = Dict::new();
    let vision = join_value(root, &or(product, "vision", "docs/product/VISION.md"))?;
    baseline.insert("vision", Value::Bool(isfile(&vision)));
    let tech_spec = join_value(root, &or(product, "tech_spec", "docs/product/TECH_SPEC.md"))?;
    baseline.insert("tech_spec", Value::Bool(isfile(&tech_spec)));
    baseline.insert(
        "style",
        cfg.section("architecture").get_or_none("style").clone(),
    );
    let tec_doc = index.truthy() && isfile(&join_value(root, &index)?);
    baseline.insert("tec_doc", Value::Bool(tec_doc));
    baseline.insert("epics", Value::int(0));
    baseline.insert("roadmap", Value::Bool(false));

    let mut backlog = None;
    if let Err(e) = read_backlog(root, &mut state, &mut baseline, &mut backlog) {
        findings.push(Value::Str(format!(
            "backlog: could not be read ({})",
            roadmap_exc(e)
        )));
    }
    if let Err(e) = read_roadmap(root, &mut state, &mut baseline, backlog.as_ref()) {
        findings.push(Value::Str(format!(
            "roadmap: could not be read ({})",
            roadmap_exc(e)
        )));
    }
    state.insert("baseline", Value::Dict(baseline));

    match read_plans(root) {
        Ok(plans) => state.insert("plans", Value::List(plans)),
        Err(c) => findings.push(Value::Str(format!(
            "plans: could not be read ({})",
            exc_str(&c)
        ))),
    }

    let rc = rc_state(root, &cfg)?;
    state.insert("rc", rc);
    let prs = prs_state(root, &cfg, &mut findings);
    state.insert("prs", Value::List(prs));
    let speckit = speckit_state(skills, root, &cfg)?;
    let speckit_name = speckit.get_or_none("state").clone();
    state.insert("speckit", Value::Dict(speckit));
    let mut agent_d = Dict::new();
    agent_d.insert("name", agent.map_or(Value::Null, s));
    agent_d.insert("attribution_hidden", Value::Bool(attribution_hidden(root)));
    state.insert("agent", Value::Dict(agent_d));
    let ignored = if is_git {
        ignored_protocol_files(root, &cfg, &speckit_name, agent)?
    } else {
        Vec::new()
    };
    state.insert("ignored", Value::List(ignored));
    state.insert("findings", Value::List(findings));
    Ok(state)
}

/// Open pull requests on the platform with what is pending; `[]` when the platform is not
/// reachable.
fn prs_state(root: &str, cfg: &ProjectConfig, findings: &mut Vec<Value>) -> Vec<Value> {
    if cfg
        .section("review")
        .get_or_none("provider")
        .py_eq(&s("none"))
    {
        return Vec::new();
    }
    let read = |findings: &mut Vec<Value>| -> Result<Vec<Value>, Fail> {
        let ctx = Context::new(Some(root))?;
        let name = &ctx.provider_name;
        if name.truthy() && !name.is_hashable() {
            return Err(Fail::Crash(Crash::new(format!(
                "TypeError: unhashable type: '{}'",
                name.type_name()
            ))));
        }
        if !name.truthy() || !cli_available(name.as_str().unwrap_or("")) {
            if name.truthy() {
                findings.push(Value::Str(format!(
                    "pull requests: the {} CLI is not installed, so they were not read",
                    name.py_str()
                )));
            }
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for pr in ctx.provider()?.open_pull_requests()? {
            let p = pending_of(&ctx, &pr)?;
            let mut d = Dict::new();
            d.insert("number", pr.number.clone());
            d.insert("title", pr.title.clone());
            d.insert("summary", Value::Str(summary_line(&pr, &p)?));
            d.insert("needs_action", Value::Bool(needs_action(&p)));
            let for_user = p
                .threads
                .iter()
                .filter(|(_, hint)| *hint == "large")
                .count();
            d.insert("for_user", Value::int(for_user as i64));
            out.push(Value::Dict(d));
        }
        Ok(out)
    };
    match read(findings) {
        Ok(out) => out,
        Err(e) => {
            let msg = match e {
                Fail::Provider(m) => m,
                Fail::Crash(c) => exc_str(&c),
            };
            findings.push(Value::Str(format!(
                "pull requests: could not be read ({msg})"
            )));
            Vec::new()
        }
    }
}

/// The digits of a number as a key that orders like the number.
fn number_key(digits: &str) -> (usize, String) {
    let ascii: String = digits
        .chars()
        .filter_map(decimal_value)
        .map(|d| char::from_digit(d, 10).unwrap_or('0'))
        .collect();
    let trimmed = ascii.trim_start_matches('0').to_string();
    (trimmed.len(), trimmed)
}

/// A run's version numbers and candidate number, ordered like numbers.
type RunKey = Vec<(usize, String)>;

/// The latest release candidate run of the manual test suite, or `None`.
fn rc_state(root: &str, cfg: &ProjectConfig) -> Result<Value, Crash> {
    let path = join_value(
        root,
        &or(
            cfg.section("tests"),
            "manual_tests",
            "tests/MANUAL_TESTS.md",
        ),
    )?;
    if !isfile(&path) {
        return Ok(Value::Null);
    }
    let suite =
        load_suite(Some(root), None)?.map_err(|m| Crash::new(format!("FileNotFoundError: {m}")))?;
    // (key, run, version) of the latest run.
    let mut best: Option<(RunKey, String, String)> = None;
    for t in &suite.tests {
        let Some(run) = t.run.as_deref().filter(|r| !r.is_empty()) else {
            continue;
        };
        let chars: Vec<char> = run.chars().collect();
        let Some(m) = RC_RE.match_start(&chars) else {
            continue;
        };
        let version = m.group(&chars, 1).unwrap_or_default();
        let mut key: RunKey = version[1..].split('.').map(number_key).collect();
        key.push(number_key(&m.group(&chars, 2).unwrap_or_default()));
        if best.as_ref().is_none_or(|(k, _, _)| key > *k) {
            best = Some((key, run.to_string(), version));
        }
    }
    let Some((_, run, version)) = best else {
        return Ok(Value::Null);
    };
    let tests = suite.select(Some(&run));
    let released =
        isdir(&join(root, ".git")) && !git_str(root, &["tag", "-l", &version])?.1.is_empty();
    let mut d = Dict::new();
    d.insert("run", Value::Str(run));
    d.insert("version", Value::Str(version));
    let not_run = tests.iter().filter(|t| t.status.is_none()).count();
    d.insert("not_run", Value::int(not_run as i64));
    let failed = tests
        .iter()
        .filter(|t| matches!(t.status, Some("Fail" | "Blocked")))
        .count();
    d.insert("failed", Value::int(failed as i64));
    d.insert("released", Value::Bool(released));
    Ok(Value::Dict(d))
}

/// undecided (never asked), declined (explicit false), enabled, or unmanaged (`.specify`
/// without the protocol).
fn speckit_state(skills: &Skills, root: &str, cfg: &ProjectConfig) -> Result<Dict, Crash> {
    let raw = match cfg.raw.get_or_none("speckit") {
        v if v.truthy() => v.clone(),
        _ => Value::Dict(Dict::new()),
    };
    let Value::Dict(raw) = raw else {
        return Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            raw.type_name()
        )));
    };
    let decided = cfg.is_set("speckit", "enabled");
    let initialized = isdir(&join(root, ".specify"));
    let mut out = Dict::new();
    out.insert("uv", Value::Bool(which("uvx").is_some()));
    out.insert("problems", Value::List(Vec::new()));
    out.insert("installed", Value::Null);
    let enabled = raw.get_or_none("enabled");
    if matches!(enabled, Value::Bool(true)) {
        let data = speckit_setup::status_data(skills, root).map_err(|e| match e {
            speckit_setup::Error::Setup(m) => Crash::new(format!("speckit_setup.SetupError: {m}")),
            speckit_setup::Error::Crash(c) => c,
        })?;
        let mut problems = data
            .get_or_none("problems")
            .as_list()
            .cloned()
            .unwrap_or_default();
        out.insert("installed", data.get_or_none("installed").clone());
        out.insert("state", s("enabled"));
        if initialized {
            let Some(skills_root) = kernel::skills::root() else {
                return Err(Crash::new("RuntimeError: the skills folder is unknown"));
            };
            let found = constitution::check(root, &skills_root).map_err(|e| match e {
                constitution::Error::Constitution(m) => {
                    Crash::new(format!("constitution.ConstitutionError: {m}"))
                }
                constitution::Error::Crash(c) => c,
            })?;
            problems.extend(found.into_iter().map(Value::Str));
        }
        out.insert("problems", Value::List(problems));
        return Ok(out);
    }
    if initialized && !(decided && matches!(enabled, Value::Bool(false))) {
        out.insert("state", s("unmanaged"));
        out.insert("installed", speckit_setup::installed_version(root));
        return Ok(out);
    }
    out.insert("state", s(if decided { "declined" } else { "undecided" }));
    Ok(out)
}

fn render(rec: &Dict, state: &Dict) -> String {
    let mut lines = vec![format!("🧭 {}", rec.get_or_none("summary").py_str())];
    if rec.get_or_none("ask").truthy() {
        let recommended = rec.get_or_none("recommended");
        for (n, opt) in rec
            .get_or_none("options")
            .as_list()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let opt = opt.as_dict().cloned().unwrap_or_default();
            let star = if opt.get_or_none("key").py_eq(recommended) {
                " (recommended)"
            } else {
                ""
            };
            let reason = opt.get_or_none("reason");
            let reason = if reason.truthy() {
                format!(" — {}", reason.py_str())
            } else {
                String::new()
            };
            lines.push(format!(
                "   {}. {}{star}{reason}",
                n + 1,
                opt.get_or_none("title").py_str()
            ));
        }
    }
    for finding in state
        .get_or_none("findings")
        .as_list()
        .cloned()
        .unwrap_or_default()
    {
        lines.push(format!("   ⚠️ {}", finding.py_str()));
    }
    let repo = state.get_or_none("repo");
    let field = |k: &str| {
        repo.as_dict()
            .map_or(Value::Null, |d| d.get_or_none(k).clone())
    };
    if field("dirty").truthy() {
        lines.push(format!(
            "   ℹ️ {} uncommitted change(s) on '{}'.",
            field("dirty").py_str(),
            field("branch").py_str()
        ));
    }
    lines.join("\n")
}

/// The session started above several repositories: nothing is diagnosed until the user picks
/// one.
fn choose_repository(start: &str, repos: &[String], as_json: bool) -> Result<u8, Crash> {
    let summary = format!(
        "{start} is not a repository and holds {}. Which one is the project?",
        repos.len()
    );
    let options: Vec<(String, String)> = repos
        .iter()
        .map(|r| (basename(Path::new(r)), r.clone()))
        .collect();
    if as_json {
        let list = options
            .iter()
            .map(|(title, r)| {
                let mut d = Dict::new();
                d.insert("kind", s("repository"));
                d.insert("key", Value::Str(format!("dir:{r}")));
                d.insert("title", s(title));
                d.insert("reason", s(r));
                Value::Dict(d)
            })
            .collect();
        let mut d = Dict::new();
        d.insert("mode", s("choose"));
        d.insert("ask", Value::Bool(true));
        d.insert("recommended", Value::Null);
        d.insert("options", Value::List(list));
        d.insert("summary", Value::Str(summary));
        println(&dumps(&Value::Dict(d), Some(2))?);
    } else {
        let mut lines = vec![format!("🧭 {summary}")];
        for (n, (title, r)) in options.iter().enumerate() {
            lines.push(format!("   {}. {title} — {r}", n + 1));
        }
        println(&lines.join("\n"));
    }
    Ok(0)
}

fn command_parser(name: &str) -> Parser {
    let p = Parser::new(&format!("onboard.py {name}"), "");
    match name {
        "diagnose" => p
            .option(
                "--request",
                "The user's text after /dev-cycle (an idea or a request)",
            )
            .option(
                "--agent",
                "The coding agent running the command, e.g. claude-code",
            )
            .option("--dir", "")
            .flag("--json", ""),
        "propose-config" => p.option("--dir", "").flag("--json", ""),
        "write-config" => p
            .option("--dir", "")
            .required_option("--file", "JSON file with the additions the user confirmed"),
        _ => p.option("--dir", ""),
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
        "onboard.py",
        "Diagnose a repository for the development cycle (/dev-cycle)",
    )
    .commands(
        "command",
        &[
            ("diagnose", "Where to start, in the recommended order"),
            (
                "propose-config",
                "The .dev-cycle.json keys that can be detected and are missing",
            ),
            (
                "write-config",
                "Write confirmed additions to .dev-cycle.json (never overwrites)",
            ),
            (
                "set-attribution",
                "Claude Code: turn off automatic commit and pull request attribution",
            ),
            (
                "create-branch",
                "Create the integration branch from the production branch",
            ),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
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
    let command = args.value("command").unwrap_or_default().to_string();
    let start = abspath(args.value("dir").unwrap_or(""))
        .to_string_lossy()
        .into_owned();
    let (root, repos) = resolve_root(&start)?;
    let Some(root) = root else {
        if command == "diagnose" {
            return choose_repository(&start, &repos, args.flag("json"));
        }
        println(&format!(
            "❌ {start} is not a repository and holds {}: choose one with --dir",
            repos.len()
        ));
        return Ok(1);
    };
    let note = (!repos.is_empty()).then(|| format!("Using {root}: {start} is not a repository."));
    if let Some(n) = note.as_ref().filter(|_| command != "diagnose") {
        println(&format!("ℹ️ {n}"));
    }
    match command.as_str() {
        "propose-config" => cmd_propose(&skills, &root, &args),
        "write-config" => cmd_write(&root, &args),
        "set-attribution" => match set_attribution(&root) {
            Ok(path) => {
                println(&format!(
                    "✅ {path}: commit and pull request attribution off; commit it with the configuration"
                ));
                Ok(0)
            }
            Err(c) => failed(c, true),
        },
        "create-branch" => match create_integration_branch(&root) {
            Ok(text) => {
                println(&format!("✅ Created {text}"));
                Ok(0)
            }
            Err(c) => failed(c, false),
        },
        _ => cmd_diagnose(&skills, &root, &start, note, &args),
    }
}

/// A caught exception is printed after `❌` (exit 1); any other is a crash.
fn failed(c: Crash, os_error: bool) -> Result<u8, Crash> {
    match caught(&c, os_error) {
        Some(msg) => {
            println(&format!("❌ {msg}"));
            Ok(1)
        }
        None => Err(c),
    }
}

fn cmd_propose(skills: &Skills, root: &str, args: &Parsed) -> Result<u8, Crash> {
    let data = propose_config(skills, root)?;
    let additions = data.get_or_none("additions");
    if args.flag("json") {
        println(&dumps(&Value::Dict(data.clone()), Some(2))?);
    } else if !additions.truthy() {
        println(&format!(
            "✅ {MANIFEST_NAME} already has every key that can be detected."
        ));
    } else {
        let what = if data.get_or_none("exists").truthy() {
            "additions to"
        } else {
            "new"
        };
        println(&format!(
            "📝 Proposed {what} {} (written only after you confirm):",
            data.get_or_none("file").py_str()
        ));
        println(&dumps(additions, Some(2))?);
        let found = data.get_or_none("branching");
        let field = |k: &str| {
            found
                .as_dict()
                .map_or(Value::Null, |d| d.get_or_none(k).clone())
        };
        if field("model").truthy() {
            println(&format!(
                "ℹ️ Branching model detected: {} ({}); the alternatives are in devops-release.",
                field("model").py_str(),
                field("reason").py_str()
            ));
        }
        println("ℹ️ architecture.style is chosen with software-design, not detected.");
    }
    Ok(0)
}

fn cmd_write(root: &str, args: &Parsed) -> Result<u8, Crash> {
    let file = args.value("file").unwrap_or_default();
    let written = (|| -> Result<(String, Vec<String>), Crash> {
        let Value::Dict(additions) = json::load_file(Path::new(file)).map_err(load_error)? else {
            return Err(Crash::new("ValueError: the file must hold a JSON object"));
        };
        write_config(root, &additions)
    })();
    let (path, added) = match written {
        Ok(w) => w,
        Err(c) => return failed(c, true),
    };
    let added = if added.is_empty() {
        "nothing".to_string()
    } else {
        added.join(", ")
    };
    println(&format!("✅ {path}: added {added}"));
    let rel = relpath(&path, root)?.replace('\\', "/");
    if !ignored(root, &[rel])?.is_empty() {
        println(&format!(
            "⚠️ .gitignore keeps {MANIFEST_NAME} out of Git: add '{}' to it, or the configuration is lost in the next clone",
            gitignore_entry(MANIFEST_NAME)
        ));
    }
    Ok(0)
}

fn cmd_diagnose(
    skills: &Skills,
    root: &str,
    start: &str,
    note: Option<String>,
    args: &Parsed,
) -> Result<u8, Crash> {
    let mut state = collect(skills, root, args.value("agent"))?;
    if let Some(n) = &note {
        let mut d = Dict::new();
        d.insert("path", s(root));
        d.insert("start", s(start));
        d.insert("note", s(n));
        state.insert("root", Value::Dict(d));
    }
    let request = strip(args.value("request").unwrap_or(""));
    let request = (!request.is_empty()).then_some(request);
    let state_value = Value::Dict(state);
    let mut rec = recommendation(&state_value, request);
    if let Some(n) = &note {
        let summary = format!("{n} {}", rec.get_or_none("summary").py_str());
        rec.insert("summary", Value::Str(summary));
    }
    let Value::Dict(state) = state_value else {
        unreachable!()
    };
    if args.flag("json") {
        rec.insert("state", Value::Dict(state));
        println(&dumps(&Value::Dict(rec), Some(2))?);
    } else {
        println(&render(&rec, &state));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder outside any repository.
    fn folder(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("onboard-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        abspath(&dir.to_string_lossy())
            .to_string_lossy()
            .into_owned()
    }

    fn make_repo(path: &str) {
        std::fs::create_dir_all(path).expect("repository folder");
        let st = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(path)
            .status()
            .expect("git");
        assert!(st.success());
    }

    #[test]
    fn outside_a_repository_the_only_repository_below_is_used() {
        let top = folder("one");
        let app = join(&top, "app");
        make_repo(&app);
        std::fs::create_dir_all(join(&top, "notes")).expect("folder");
        let (root, repos) = resolve_root(&top).expect("resolves");
        assert_eq!(root.as_deref(), Some(app.as_str()));
        assert_eq!(repos, [app]);
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn outside_several_repositories_the_user_chooses() {
        let top = folder("several");
        for name in ["web", "api", ".hidden"] {
            make_repo(&join(&top, name));
        }
        let (root, repos) = resolve_root(&top).expect("resolves");
        assert_eq!(root, None);
        assert_eq!(repos, [join(&top, "api"), join(&top, "web")]);
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn a_folder_without_repositories_is_still_a_new_project() {
        let top = folder("none");
        let (root, repos) = resolve_root(&top).expect("resolves");
        assert_eq!(root.as_deref(), Some(top.as_str()));
        assert!(repos.is_empty());
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn extensions_like_splitext() {
        assert_eq!(ext("app.py"), ".py");
        assert_eq!(ext(".py"), "");
        assert_eq!(ext("..py"), "");
        assert_eq!(ext(".hidden.py"), ".py");
        assert_eq!(ext("a."), ".");
        assert_eq!(ext("Makefile"), "");
    }

    #[test]
    fn gitignore_entries_open_the_first_folder() {
        assert_eq!(gitignore_entry(".dev-cycle.json"), "!/.dev-cycle.json");
        assert_eq!(gitignore_entry("docs/plans/plan.md"), "!/docs/");
        assert_eq!(gitignore_entry("docs/"), "!/docs");
    }

    #[test]
    fn additions_never_change_a_key_the_file_sets() {
        let Ok(Value::Dict(existing)) =
            json::loads(r#"{"project_name": "mine", "devops": {"integration_branch": "develop"}}"#)
        else {
            unreachable!()
        };
        let Ok(Value::Dict(add)) = json::loads(
            r#"{"project_name": "mine", "devops": {"integration_branch": "dev", "production_branch": "main"}, "x": 1}"#,
        ) else {
            unreachable!()
        };
        let (merged, conflicts) = merge_config(&existing, &add);
        assert_eq!(conflicts, ["devops.integration_branch"]);
        let missing = missing_keys(&existing, &merged);
        let keys: Vec<String> = missing.keys().map(Value::py_str).collect();
        assert_eq!(keys, ["devops", "x"]);
    }
}
