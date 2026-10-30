//! `verify_gate.py`: the automated gate checks of the role-based development cycle.
//!   gate 0  - before asking for the product baseline approval (Phase 0, product inception)
//!   gate 3  - before asking for plan approval (bugs first: open blocking bugs hold back other
//!             plans)
//!   gate rc - release candidate acceptance (all manual tests of the run Pass)
//!   gate 8  - release readiness, before the release gate of the branching model: the release
//!             pull request (integration) or the promotion of the candidate (trunk,
//!             release-branches)
//! Project-agnostic: adapts to the project's .dev-cycle.json and detected stack.

use std::process::Command;
use std::sync::LazyLock;

use architecture_docs::docs::join_value;
use kernel::out::println;
use kernel::project_config::{ConfigError, ProjectConfig, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::num::{decimal_value, strip};
use kernel::py::path::{abspath, exists, isdir, isfile, join};
use kernel::py::re::Regex;
use kernel::py::subprocess::{run_replace, which};
use kernel::py::{Crash, Value, read_text, repr_str, splitlines, universal_newlines};
use plan_memory::backlog::{Backlog, BacklogFile, Item};
use plan_memory::roadmap::{Roadmap, RoadmapError, RoadmapFile};
use plan_memory::store::{Plan, PlanStore};

/// Credential files are matched on the file name only, never on a directory or on the
/// contents: source code and docs about secrets (secrets.rs, an ADR on redaction) are fine.
const CREDENTIAL_NAMES: [&str; 9] = [
    ".env",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "credentials.json",
    ".netrc",
    ".pgpass",
    ".htpasswd",
];
const CREDENTIAL_EXTENSIONS: [&str; 8] = [
    ".env",
    ".pem",
    ".p12",
    ".pfx",
    ".key",
    ".jks",
    ".keystore",
    ".ppk",
];
const SECRET_DATA_EXTENSIONS: [&str; 6] =
    [".json", ".yaml", ".yml", ".toml", ".ini", ".properties"];
const SECRET_WORDS: [&str; 3] = ["secret", "private_key", "private-key"];
const SAFE_MARKERS: [&str; 4] = [".example", ".sample", ".template", ".dist"];

/// Checkers already translated to the native engine (IMP-005): they run as
/// `dev-cycle <skill> <script>` instead of `python <skill>/<script>.py`.
const ENGINE_CHECKERS: [(&str, &str); 12] = [
    ("architecture-docs", "check_docs.py"),
    ("product-inception", "check_product.py"),
    ("software-design", "check_design.py"),
    ("user-docs", "check_readme.py"),
    ("quality-assurance", "manual_tests.py"),
    ("devops-release", "check_ci_policy.py"),
    ("devops-release", "release_status.py"),
    ("devops-release", "branching.py"),
    ("plan-memory", "list_plans.py"),
    ("plan-memory", "roadmap.py"),
    ("dev-cycle-protocol", "constitution.py"),
    ("dev-cycle-protocol", "speckit_setup.py"),
];

const NEEDS_CLARIFICATION: &str = "[NEEDS CLARIFICATION";
const SPEC_REQUIRED: [&str; 3] = ["spec.md", "plan.md", "tasks.md"];

static RC_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"-rc\.\d+$", ""));

/// `str(e)` for an exception the script caught: its message without the type.
fn exc_str(c: &Crash) -> String {
    c.0.split_once(": ")
        .map_or(String::new(), |(_, m)| m.to_string())
}

fn roadmap_crash(e: RoadmapError) -> Crash {
    match e {
        RoadmapError::Error(m) => Crash::new(format!("ValueError: {m}")),
        RoadmapError::Crash(c) => c,
    }
}

fn config(cwd: &str) -> Result<ProjectConfig, Crash> {
    load_project_config(Some(cwd)).map_err(ConfigError::into_crash)
}

fn line(text: &str) {
    println(text);
}

fn rule(c: char) -> String {
    c.to_string().repeat(80)
}

/// `run_cmd(cmd, cwd)`: the exit code and stdout stripped; -1 and the exception's text when
/// the command cannot be started.
fn run_cmd(cmd: &[&str], cwd: &str) -> (i32, String) {
    if !exists(cwd) {
        return (
            -1,
            if cfg!(windows) {
                "[WinError 267] The directory name is invalid".to_string()
            } else {
                format!("[Errno 2] No such file or directory: {}", repr_str(cwd))
            },
        );
    }
    if !isdir(cwd) {
        return (
            -1,
            if cfg!(windows) {
                "[WinError 267] The directory name is invalid".to_string()
            } else {
                format!("[Errno 20] Not a directory: {}", repr_str(cwd))
            },
        );
    }
    let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
    match run_replace(&cmd, Some(cwd)) {
        Ok((code, out, _)) => (code, strip(&out).to_string()),
        Err(Ok(msg)) => (-1, msg),
        Err(Err(c)) => (-1, exc_str(&c)),
    }
}

/// `os.path.splitext(name)`: the extension keeps its dot; leading dots stay in the name.
fn splitext(name: &str) -> (&str, &str) {
    let lead = name.len() - name.trim_start_matches('.').len();
    match name[lead..].rfind('.') {
        Some(i) => (&name[..lead + i], &name[lead + i..]),
        None => (name, ""),
    }
}

/// True when a path looks like a real credential file: an exact credential name, a key or
/// certificate store extension, or a secret-named data file. Templates are never flagged.
pub fn is_credential_file(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let name = kernel::py::lower(path.rsplit('/').next().unwrap_or(""));
    if SAFE_MARKERS.iter().any(|m| name.contains(m)) {
        return false;
    }
    if CREDENTIAL_NAMES.contains(&name.as_str()) || name.starts_with(".env.") {
        return true;
    }
    let (stem, ext) = splitext(&name);
    if CREDENTIAL_EXTENSIONS.contains(&ext) {
        return true;
    }
    SECRET_DATA_EXTENSIONS.contains(&ext) && SECRET_WORDS.iter().any(|w| stem.contains(w))
}

fn check_secret_hygiene(cwd: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let git = ["git", "-c", "core.quotepath=off"];

    // 1. Credential files committed to Git
    let (code, out) = run_cmd(
        &[&git[..], &["ls-tree", "-r", "--name-only", "HEAD"]].concat(),
        cwd,
    );
    let mut committed: Vec<&str> = if code == 0 {
        splitlines(&out)
    } else {
        Vec::new()
    };
    committed.sort();
    committed.dedup();
    for f in &committed {
        if is_credential_file(f) {
            violations.push(format!("Tracked credential file committed to Git: {f}"));
        }
    }

    // 2. Credential files staged for commit (added, modified or renamed, not yet committed)
    let (code, out) = run_cmd(
        &[
            &git[..],
            &["diff", "--cached", "--name-only", "--diff-filter=AMR"],
        ]
        .concat(),
        cwd,
    );
    if code == 0 {
        for f in splitlines(&out) {
            if !f.is_empty() && !committed.contains(&f) && is_credential_file(f) {
                violations.push(format!("Staged credential file ready to be committed: {f}"));
            }
        }
    }
    violations
}

/// `sys.executable` for the checkers still written in Python: the engine has no interpreter
/// of its own, so the one on PATH runs them.
fn python() -> Option<String> {
    let names = if cfg!(windows) {
        ["python", "python3"]
    } else {
        ["python3", "python"]
    };
    names.iter().find_map(|n| which(n))
}

/// Runs a sibling skill's checker: (exit code, output), or (None, reason) when missing.
fn run_checker(skill: &str, script: &str, args: &[&str]) -> Result<(Option<i32>, String), Crash> {
    let root = kernel::skills::root()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = join(&join(&root, skill), script);
    let missing = || Ok((None, format!("{skill}/{script} not installed")));
    let mut command = if ENGINE_CHECKERS.contains(&(skill, script)) {
        let launcher = join(
            &join(&join(&root, "dev-cycle-protocol"), "bin"),
            "dev-cycle",
        );
        if !isfile(&launcher) {
            return missing();
        }
        let exe = std::env::current_exe().map_err(|e| Crash::new(format!("OSError: {e}")))?;
        let mut c = Command::new(exe);
        c.args([skill, &script[..script.len() - ".py".len()]]);
        c
    } else if isfile(&path) {
        let Some(python) = python() else {
            return missing();
        };
        let mut c = Command::new(python);
        c.arg(&path);
        c
    } else {
        return missing();
    };
    // The engine reads the skills' files from this skills folder, as the scripts do.
    command.args(args).env(kernel::skills::ENV, &root);
    let out = command
        .output()
        .map_err(|e| Crash::new(format!("OSError: {e}")))?;
    let text = |b: &[u8]| universal_newlines(&String::from_utf8_lossy(b));
    let both = text(&out.stdout) + &text(&out.stderr);
    Ok((
        Some(out.status.code().unwrap_or(-1)),
        strip(&both).to_string(),
    ))
}

/// Prints a checker's outcome; returns false when it blocks the gate.
fn report_checker(
    label: &str,
    code: Option<i32>,
    output: &str,
    blocking: bool,
    max_lines: usize,
) -> bool {
    let lines = splitlines(output);
    let Some(code) = code else {
        line(&format!("⚠️ {label}: skipped ({output})."));
        return true;
    };
    if code == 0 {
        line(&format!(
            "✅ {label} passed. {}",
            lines.last().copied().unwrap_or("")
        ));
        return true;
    }
    line(&format!(
        "{} {label} failed:",
        if blocking { "❌" } else { "⚠️" }
    ));
    for l in lines.iter().take(max_lines) {
        line(&format!("   {l}"));
    }
    if lines.len() > max_lines {
        line(&format!("   ... ({} more lines)", lines.len() - max_lines));
    }
    !blocking
}

fn report(label: &str, result: (Option<i32>, String)) -> bool {
    report_checker(label, result.0, &result.1, true, 20)
}

/// `(BacklogFile, Backlog)` of the project; no backlog when there is no backlog file.
fn load_backlog(cwd: &str) -> Result<(BacklogFile, Option<Backlog>), Crash> {
    let bf = BacklogFile::new(Some(cwd))?;
    let backlog = if bf.exists() {
        Some(bf.load().map_err(roadmap_crash)?)
    } else {
        None
    };
    Ok((bf, backlog))
}

/// `(RoadmapFile, Roadmap)` of the project; no roadmap when there is no roadmap file.
fn load_roadmap(cwd: &str) -> Result<(RoadmapFile, Option<Roadmap>), Crash> {
    let rf = RoadmapFile::new(Some(cwd))?;
    let roadmap = if rf.exists() {
        Some(rf.load(false).map_err(roadmap_crash)?)
    } else {
        None
    };
    Ok((rf, roadmap))
}

fn contains_item(items: &[&Item], item: &Item) -> bool {
    items.iter().any(|i| std::ptr::eq(*i, item))
}

/// `"sep".join(value)` of a configured list.
fn join_config(sep: &str, value: &Value) -> Result<String, Crash> {
    kernel::py::join(sep, value)
}

fn untriaged_line(b: &Item, why: &str) -> String {
    let rank = b.rank();
    let rank = if rank.is_empty() {
        "?".to_string()
    } else {
        rank
    };
    format!("   {} ({rank}) {}: {why}", b.id, b.title)
}

/// Open bugs with a blocking severity, and bugs not triaged yet, come before any plan that is
/// not a bug plan.
fn check_bugs_first(
    cwd: &str,
    plan: &mut Plan,
    store: &PlanStore,
    accept_reason: Option<&str>,
) -> Result<bool, Crash> {
    let (bf, backlog) = load_backlog(cwd)?;
    let Some(backlog) = backlog else {
        return Ok(true);
    };
    let rel = bf.rel()?;
    let problems = backlog.validate();
    if !problems.is_empty() {
        line(&format!(
            "⚠️ Backlog {rel} has {} problem(s); run plan-memory/backlog.py validate.",
            problems.len()
        ));
    }
    let mut blocking = Vec::new();
    for b in backlog.blocking_bugs(&bf.blocking_severities)? {
        if !plan.backlog_items()?.contains(&b.id) {
            blocking.push(b);
        }
    }
    let triage = bf
        .triage(&backlog, &plan.backlog_items()?)
        .map_err(roadmap_crash)?;
    let untriaged: Vec<(&Item, String)> = triage
        .untriaged
        .iter()
        .filter(|(b, _)| !contains_item(&blocking, b))
        .map(|(b, why)| (*b, why.clone()))
        .collect();
    let untriaged_items: Vec<&Item> = untriaged.iter().map(|(b, _)| *b).collect();
    let mut others = Vec::new();
    for b in backlog.open_items_of("bug") {
        if !contains_item(&blocking, b)
            && !plan.backlog_items()?.contains(&b.id)
            && !contains_item(&untriaged_items, b)
        {
            others.push(b);
        }
    }
    if !others.is_empty() {
        let ids: Vec<&str> = others.iter().map(|b| b.id.as_str()).collect();
        line(&format!(
            "ℹ️ Other open bugs in {rel} (triaged): {}",
            ids.join(", ")
        ));
    }
    let next_release = bf.bug_deadline.as_str() == Some("next_release");
    if next_release && !triage.deadline_checked && !others.is_empty() {
        line(
            "⚠️ No roadmap: the deadline of the bugs that do not block (the next release) is not checked.",
        );
    }
    if blocking.is_empty() && untriaged.is_empty() {
        let severities = join_config("/", &bf.blocking_severities)?;
        line(&format!(
            "✅ Bugs first: no open {} bug outside this plan{}",
            if severities.is_empty() {
                "blocking"
            } else {
                &severities
            },
            if next_release {
                ", every other bug is triaged."
            } else {
                "."
            }
        ));
        return Ok(true);
    }
    let mut listing: Vec<String> = blocking
        .iter()
        .map(|b| format!("{} ({})", b.id, b.rank()))
        .collect();
    listing.extend(
        untriaged
            .iter()
            .map(|(b, _)| format!("{} (not triaged)", b.id)),
    );
    let listing = listing.join(", ");
    if plan.kind().py_eq(&Value::str("bug")) {
        line(&format!(
            "ℹ️ Bug plan; other bugs still come before new work: {listing}"
        ));
        return Ok(true);
    }
    if let Some(reason) = accept_reason.filter(|r| !r.is_empty()) {
        let mut ids: Vec<&str> = blocking.iter().map(|b| b.id.as_str()).collect();
        ids.extend(untriaged.iter().map(|(b, _)| b.id.as_str()));
        plan.add_log(
            store.author(),
            &format!(
                "Approved with open bugs before new work {}: {reason}",
                ids.join(", ")
            ),
        )?;
        store.save(plan)?;
        line(&format!(
            "⚠️ Bugs first overridden by the user: {listing}. Recorded in the plan's status log; commit the plan file."
        ));
        return Ok(true);
    }
    let kind = plan.kind().py_str();
    if !blocking.is_empty() {
        let severities = join_config(", ", &bf.blocking_severities)?;
        let ranked: Vec<String> = blocking
            .iter()
            .map(|b| format!("{} ({})", b.id, b.rank()))
            .collect();
        line(&format!(
            "❌ Bugs first: open {severities} bugs come before this {kind} plan: {}",
            ranked.join(", ")
        ));
        for b in &blocking {
            line(&format!("   {}: {}", b.id, b.title));
        }
        line("   Plan them first (plan-memory/backlog.py promote).");
    }
    if !untriaged.is_empty() {
        let nxt = triage.next_release.clone();
        line(&format!(
            "❌ Bugs first: open bugs not triaged come before this {kind} plan:"
        ));
        for (b, why) in &untriaged {
            line(&untriaged_line(b, why));
        }
        line(
            "   Triage them: a severity (backlog.py set <ID> --severity <s>) and, for the bugs that do not block,",
        );
        line(&format!(
            "   the next release{} at the latest (roadmap.py assign <ID> --release {} [--move]); or fix them first.",
            nxt.as_ref().map_or(String::new(), |n| format!(" {n}")),
            nxt.as_deref().unwrap_or("<vX.Y.Z>")
        ));
    }
    line(
        "   Only when the user explicitly accepts the risk, run this gate again with --accept-open-bugs \"<the user's reason>\".",
    );
    Ok(false)
}

/// `re.escape(prefix)` needs a string.
fn prefix_text(prefix: &Value) -> Result<String, Crash> {
    match prefix {
        Value::Str(s) => Ok(s.clone()),
        other => Err(Crash::new(format!(
            "TypeError: decoding to str: need a bytes-like object, {} found",
            other.type_name()
        ))),
    }
}

/// `re.match(rf"^{re.escape(prefix)}/\d{3,}-", branch)`.
fn is_feature_branch(branch: &str, prefix: &str) -> bool {
    let Some(rest) = branch
        .strip_prefix(prefix)
        .and_then(|r| r.strip_prefix('/'))
    else {
        return false;
    };
    let digits = rest
        .chars()
        .take_while(|c| decimal_value(*c).is_some())
        .count();
    digits >= 3 && rest.chars().nth(digits) == Some('-')
}

/// Spec Kit artifacts of a feature plan: complete, without open clarifications, on a protocol
/// branch.
fn check_spec_dir(cwd: &str, plan: &Plan) -> Result<bool, Crash> {
    let cfg = config(cwd)?;
    let speckit = cfg.section("speckit");
    if !speckit.get_or_none("enabled").truthy() || plan.kind().py_eq(&Value::str("bug")) {
        return Ok(true);
    }
    let Some(spec_dir) = plan.spec_dir().filter(|d| !d.is_empty()) else {
        line(
            "⚠️ Spec Kit is enabled but this feature plan has no spec_dir (save_plan.py --spec-dir specs/NNN-slug); the user decides whether this change goes without Spec Kit.",
        );
        return Ok(true);
    };
    let folder = join(cwd, &spec_dir);
    let missing: Vec<&str> = SPEC_REQUIRED
        .iter()
        .copied()
        .filter(|n| !isfile(&join(&folder, n)))
        .collect();
    if !missing.is_empty() {
        line(&format!(
            "❌ Spec Kit artifacts missing in {spec_dir}: {} (/speckit-specify, /speckit-plan, /speckit-tasks).",
            missing.join(", ")
        ));
        return Ok(false);
    }
    let mut passed = true;
    for name in ["spec.md", "plan.md"] {
        let text = read_text(std::path::Path::new(&join(&folder, name)))?;
        let count = text.matches(NEEDS_CLARIFICATION).count();
        if count > 0 {
            line(&format!(
                "❌ {spec_dir}/{name} has {count} open [NEEDS CLARIFICATION] marker(s): run /speckit-clarify."
            ));
            passed = false;
        }
    }
    let text = read_text(std::path::Path::new(&join(&folder, "plan.md")))?;
    if !text.contains("## Dev Cycle Protocol") {
        line(&format!(
            "⚠️ {spec_dir}/plan.md has no 'Dev Cycle Protocol' sections (is the dev-cycle preset installed?)."
        ));
    }
    let configured = speckit.get_or_none("branch_prefix");
    let prefix = if configured.truthy() {
        configured.clone()
    } else {
        Value::str("feat")
    };
    let (code, branch) = run_cmd(&["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd);
    if code == 0 {
        let prefix = prefix_text(&prefix)?;
        if !is_feature_branch(&branch, &prefix) {
            line(&format!(
                "⚠️ Current branch '{branch}' is not a Spec Kit feature branch ({prefix}/NNN-slug)."
            ));
        }
    }
    if passed {
        line(&format!(
            "✅ Spec Kit artifacts complete in {spec_dir} (spec, plan, tasks; no open clarification)."
        ));
    }
    Ok(passed)
}

/// When Spec Kit is enabled: the setup matches the pin and the constitution is current.
fn check_speckit(cwd: &str) -> Result<bool, Crash> {
    let cfg = config(cwd)?;
    if !cfg.section("speckit").get_or_none("enabled").truthy() {
        return Ok(true);
    }
    let mut passed = report(
        "Spec Kit setup (speckit_setup.py status)",
        run_checker(
            "dev-cycle-protocol",
            "speckit_setup.py",
            &["status", "--dir", cwd],
        )?,
    );
    passed &= report(
        "Constitution (constitution.py check)",
        run_checker(
            "dev-cycle-protocol",
            "constitution.py",
            &["check", "--dir", cwd],
        )?,
    );
    Ok(passed)
}

/// `record.strip().split("\x1f", 5)` unpacked into six fields.
fn commit_fields(record: &str) -> Result<[&str; 6], Crash> {
    let parts: Vec<&str> = strip(record).splitn(6, '\x1f').collect();
    <[&str; 6]>::try_from(parts.as_slice()).map_err(|_| {
        Crash::new(format!(
            "ValueError: not enough values to unpack (expected 6, got {})",
            parts.len()
        ))
    })
}

/// Commit messages and identities since the last tag (all commits when there is none) pass
/// the principle 9 guard.
fn check_commit_attribution(cwd: &str, cfg: &ProjectConfig) -> Result<bool, Crash> {
    let (_, tag) = run_cmd(&["git", "describe", "--tags", "--abbrev=0"], cwd);
    let range = format!("{tag}..HEAD");
    let mut cmd = vec![
        "git",
        "log",
        "--format=%h%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e",
    ];
    if !tag.is_empty() && !tag.starts_with("fatal") {
        cmd.push(&range);
    }
    let (code, out) = run_cmd(&cmd, cwd);
    if code != 0 {
        line("ℹ️ Could not read the commit messages for the attribution check.");
        return Ok(true);
    }
    let configured = cfg.section("review").get_or_none("allowed_terms");
    let allowed = if configured.truthy() {
        configured.clone()
    } else {
        Value::List(Vec::new())
    };
    let mut bad = Vec::new();
    for record in out.split('\x1e') {
        if strip(record).is_empty() {
            continue;
        }
        let [
            sha,
            author,
            author_email,
            committer,
            committer_email,
            message,
        ] = commit_fields(record)?;
        let mut found = kernel::no_ai::findings(message, &allowed)?;
        found.extend(kernel::no_ai::identity_findings(author, author_email));
        if (committer, committer_email) != (author, author_email) {
            found.extend(kernel::no_ai::identity_findings(committer, committer_email));
        }
        for (hit, reason) in found {
            bad.push(format!("{sha}: '{hit}' ({reason})"));
        }
    }
    let since = if tag.is_empty() {
        "the first commit"
    } else {
        tag.as_str()
    };
    if !bad.is_empty() {
        line(&format!(
            "❌ AI attribution in {} place(s) of the commits since {since} (protocol principle 9); reword them or set the repository's Git identity before the release:",
            bad.len()
        ));
        for l in bad.iter().take(20) {
            line(&format!("   {l}"));
        }
        return Ok(false);
    }
    line(&format!(
        "✅ No AI attribution in the commits since {since}."
    ));
    Ok(true)
}

fn report_secrets(cwd: &str, ok: &str) -> bool {
    let secrets = check_secret_hygiene(cwd);
    if secrets.is_empty() {
        line(ok);
        return true;
    }
    for s in secrets {
        line(&format!("🚨 SECRET HYGIENE ALERT: {s}"));
    }
    false
}

/// Product baseline gate (Phase 0): vision, technical specification, architecture, backlog
/// and roadmap.
fn verify_gate_0(cwd: &str, project_name: &str) -> Result<bool, Crash> {
    line(&rule('='));
    line(&format!(
        "🌱 VERIFYING GATE 0 (PRODUCT BASELINE): {project_name}"
    ));
    line(&rule('='));

    let mut passed = true;
    let cfg = config(cwd)?;

    // 1. Business document and technical specification (product-inception)
    let (code, out) = run_checker("product-inception", "check_product.py", &["--dir", cwd])?;
    passed &= report_checker(
        "Product documents (product-inception)",
        code,
        &out,
        true,
        40,
    );
    let questions: Vec<&str> = splitlines(&out)
        .into_iter()
        .map(strip)
        .filter(|l| l.starts_with('❓'))
        .collect();
    if code == Some(0) && !questions.is_empty() {
        line(&format!(
            "⚠️ {} open question(s) remain; the user decides whether the baseline is approved with them:",
            questions.len()
        ));
        for q in questions {
            line(&format!("   {q}"));
        }
    }

    // 2. Architecture style (software-design) and technical documentation (architecture-docs)
    let style = cfg.section("architecture").get_or_none("style");
    if style.truthy() {
        line(&format!("✅ Architecture style set: {}", style.py_str()));
    } else {
        line("❌ architecture.style is not set in .dev-cycle.json (software-design).");
        passed = false;
    }
    let from_docs = cfg.section("architecture_docs").get_or_none("index");
    let index = if from_docs.truthy() {
        from_docs.clone()
    } else {
        cfg.section("docs").get_or_none("tec_doc").clone()
    };
    if index.truthy() && isfile(&join_value(cwd, &index)?) {
        let (code, out) = run_checker("architecture-docs", "check_docs.py", &["--dir", cwd])?;
        if code.is_some() && out.contains("does not use the architecture-docs layout") {
            line(
                "⚠️ TecDoc not migrated to the architecture-docs layout (docs/architecture + docs/adr) yet.",
            );
        } else {
            passed &= report_checker(
                "Architecture docs check (architecture-docs)",
                code,
                &out,
                true,
                20,
            );
        }
    } else {
        line(&format!(
            "❌ Technical documentation index missing ({}): create it with architecture-docs.",
            if index.truthy() {
                index.py_str()
            } else {
                "not configured".to_string()
            }
        ));
        passed = false;
    }

    // 3. Product backlog with epics (plan-memory)
    let mut loaded: Option<Backlog> = None;
    let result = (|| -> Result<(), Crash> {
        let (bf, backlog) = load_backlog(cwd)?;
        let Some(backlog) = backlog else {
            line(&format!(
                "❌ No backlog at {}: capture the product's epics and items (plan-memory/backlog.py).",
                bf.rel()?
            ));
            passed = false;
            return Ok(());
        };
        let problems = backlog.validate();
        let epics = backlog.open_items_of("epic");
        if !problems.is_empty() {
            line(&format!(
                "❌ Backlog {} has {} problem(s); run plan-memory/backlog.py validate.",
                bf.rel()?,
                problems.len()
            ));
            passed = false;
        } else if epics.is_empty() {
            line(&format!(
                "❌ Backlog {} has no epic: the complete product is planned as epics and their items.",
                bf.rel()?
            ));
            passed = false;
        } else {
            let open = backlog.open_items();
            let loose: Vec<&str> = open
                .iter()
                .filter(|i| !matches!(i.kind(), Some("epic" | "bug")) && i.epic().is_none())
                .map(|i| i.id.as_str())
                .collect();
            line(&format!(
                "✅ Backlog: {} epic(s), {} open item(s).",
                epics.len(),
                open.len() - epics.len()
            ));
            if !loose.is_empty() {
                line(&format!("ℹ️ Items without an epic: {}", loose.join(", ")));
            }
        }
        drop(epics);
        loaded = Some(backlog);
        Ok(())
    })();
    if let Err(c) = result {
        line(&format!("❌ Could not read the backlog: {}", exc_str(&c)));
        loaded = None;
        passed = false;
    }

    // 4. Roadmap (plan-memory)
    let result = (|| -> Result<(), Crash> {
        let (rf, roadmap) = load_roadmap(cwd)?;
        let Some(roadmap) = roadmap else {
            line(&format!(
                "❌ No roadmap at {}: plan the releases (plan-memory/roadmap.py add-release, assign).",
                rf.rel()?
            ));
            passed = false;
            return Ok(());
        };
        let problems = roadmap.validate(loaded.as_ref());
        match roadmap.current() {
            _ if !problems.is_empty() => {
                line(&format!(
                    "❌ Roadmap {} has {} problem(s); run plan-memory/roadmap.py validate.",
                    rf.rel()?,
                    problems.len()
                ));
                passed = false;
            }
            Some(current) if !current.items().is_empty() => {
                line(&format!(
                    "✅ Roadmap: {} release(s); first: {} {} ({} item(s)).",
                    roadmap.releases.len(),
                    current.version,
                    current.name,
                    current.items().len()
                ));
            }
            _ => {
                line(&format!(
                    "❌ Roadmap {} has no release with items to start from.",
                    rf.rel()?
                ));
                passed = false;
            }
        }
        Ok(())
    })();
    if let Err(c) = result {
        line(&format!("❌ Could not read the roadmap: {}", exc_str(&c)));
        passed = false;
    }

    // 5. Spec Kit and the generated constitution, when enabled
    passed &= check_speckit(cwd)?;

    // 6. Secret hygiene
    passed &= report_secrets(cwd, "✅ Secret hygiene check passed.");

    line(&rule('-'));
    line(if passed {
        "🟢 GATE 0 STATUS: READY FOR THE USER'S APPROVAL OF THE PRODUCT BASELINE"
    } else {
        "🚨 GATE 0 STATUS: PRODUCT BASELINE INCOMPLETE"
    });
    line(&format!("{}\n", rule('=')));
    Ok(passed)
}

/// Plan approval gate.
fn verify_gate_3(
    cwd: &str,
    project_name: &str,
    accept_open_bugs: Option<&str>,
) -> Result<bool, Crash> {
    line(&rule('='));
    line(&format!(
        "🔒 VERIFYING GATE 3 (PLAN APPROVAL): {project_name}"
    ));
    line(&rule('='));

    let mut passed = true;
    let (_, status_out) = run_cmd(&["git", "status", "--porcelain"], cwd);
    let dirty = splitlines(&status_out)
        .into_iter()
        .filter(|l| !strip(l).is_empty())
        .count();
    if dirty > 0 {
        line(&format!(
            "⚠️ Working tree has {dirty} uncommitted/untracked changes."
        ));
    } else {
        line("✅ Git working tree is clean.");
    }

    let result = (|| -> Result<(), Crash> {
        let store = PlanStore::new(Some(cwd), None)?;
        let active = [Value::str("active"), Value::str("in_progress")];
        let in_force: Vec<Plan> = store
            .load_all()?
            .into_iter()
            .filter(|p| {
                let status = p.meta.get_or_none("status");
                active.iter().any(|a| status.py_eq(a))
            })
            .collect();
        let count = in_force.len();
        let Some(mut latest) = PlanStore::latest_of(in_force) else {
            line(&format!(
                "⚠️ No active plan in {}. Save the plan with plan-memory in Phase 2 (full path).",
                store.plans_dir
            ));
            return Ok(());
        };
        line(&format!(
            "✅ Active plan found in {}: '{}' ({count} active/in progress)",
            store.plans_dir,
            latest.meta.get_or_none("plan_id").py_str()
        ));
        let (_, tracked) = run_cmd(&["git", "ls-files", "--", &latest.path], cwd);
        if tracked.is_empty() {
            line("⚠️ The active plan file is not committed yet. Commit it after approval.");
        }
        passed &= check_bugs_first(cwd, &mut latest, &store, accept_open_bugs)?;
        passed &= check_spec_dir(cwd, &latest)?;
        Ok(())
    })();
    if let Err(c) = result {
        line(&format!(
            "ℹ️ Could not check plan-memory plans: {}",
            exc_str(&c)
        ));
    }

    passed &= check_speckit(cwd)?;

    passed &= report_secrets(
        cwd,
        "✅ Secret hygiene check passed (no unencrypted credential files detected).",
    );

    line(&rule('-'));
    line(if passed {
        "🟢 GATE 3 STATUS: READY FOR EXPLICIT USER APPROVAL"
    } else {
        "🚨 GATE 3 STATUS: BLOCKED BY COMPLIANCE CHECKS"
    });
    line(&format!("{}\n", rule('=')));
    Ok(passed)
}

/// Release candidate acceptance gate: every manual test of the run is Pass.
fn verify_gate_rc(cwd: &str, project_name: &str, run: Option<&str>) -> Result<bool, Crash> {
    let run = run.filter(|r| !r.is_empty());
    line(&rule('='));
    line(&format!(
        "✅ VERIFYING RELEASE CANDIDATE ACCEPTANCE: {project_name} ({})",
        run.unwrap_or("no run")
    ));
    line(&rule('='));
    let Some(run) = run else {
        line(
            "❌ --run <rc tag> is required (the run started with quality-assurance/manual_tests.py start-run).",
        );
        line(&format!("{}\n", rule('=')));
        return Ok(false);
    };
    let (code, out) = run_checker(
        "quality-assurance",
        "manual_tests.py",
        &["gate", "--run", run, "--dir", cwd],
    )?;
    let passed =
        code.is_some() && report_checker(&format!("Manual test run {run}"), code, &out, true, 20);
    if code.is_none() {
        line(&format!("❌ {out}"));
    }
    line(&rule('-'));
    line(if passed {
        "🟢 RC GATE STATUS: CANDIDATE ACCEPTED"
    } else {
        "🚨 RC GATE STATUS: NOT ACCEPTED"
    });
    line(&format!("{}\n", rule('=')));
    Ok(passed)
}

/// Release readiness gate, before the release gate of the branching model.
fn verify_gate_8(cwd: &str, project_name: &str, run: Option<&str>) -> Result<bool, Crash> {
    let run = run.filter(|r| !r.is_empty());
    line(&rule('='));
    line(&format!(
        "🚀 VERIFYING GATE 8 (RELEASE READINESS): {project_name}"
    ));
    line(&rule('='));

    let mut passed = true;
    let cfg = config(cwd)?;
    let devops = cfg.section("devops");
    let promote = devops_release::branching::gate_of(devops.get_or_none("branching"))?
        .py_eq(&Value::str("promote"));

    // 1. Documentation and manual tests at the configured paths
    for (label, rel) in [
        ("UserDoc", cfg.section("docs").get_or_none("user_doc")),
        ("TecDoc", cfg.section("docs").get_or_none("tec_doc")),
        (
            "Manual test suite",
            cfg.section("tests").get_or_none("manual_tests"),
        ),
    ] {
        if rel.truthy() && exists(&join_value(cwd, rel)?) {
            line(&format!("✅ {label} verified ({} exists).", rel.py_str()));
        } else {
            line(&format!(
                "❌ {label} missing ({} required).",
                if rel.truthy() {
                    rel.py_str()
                } else {
                    "path not configured".to_string()
                }
            ));
            passed = false;
        }
    }

    // 2. Technical documentation (architecture-docs)
    let (code, out) = run_checker(
        "architecture-docs",
        "check_docs.py",
        &["--dir", cwd, "--max-warnings", "10"],
    )?;
    if code.is_some() && out.contains("does not use the architecture-docs layout") {
        line(
            "⚠️ TecDoc not migrated to the architecture-docs layout (docs/architecture + docs/adr) yet.",
        );
    } else {
        passed &= report_checker(
            "Architecture docs check (architecture-docs)",
            code,
            &out,
            true,
            20,
        );
    }

    // 3. README (user-docs)
    passed &= report(
        "README check (user-docs)",
        run_checker("user-docs", "check_readme.py", &["--dir", cwd])?,
    );

    // 4. Manual test suite format (quality-assurance)
    passed &= report(
        "Manual test suite (quality-assurance)",
        run_checker(
            "quality-assurance",
            "manual_tests.py",
            &["validate", "--dir", cwd],
        )?,
    );

    // 5. Design and core dependencies (software-design)
    passed &= report(
        "Design check (software-design)",
        run_checker("software-design", "check_design.py", &["--dir", cwd])?,
    );

    // 6. CI policy (devops-release), when the project uses GitHub Actions
    if isdir(&join(&join(cwd, ".github"), "workflows")) {
        passed &= report(
            "CI policy (devops-release)",
            run_checker("devops-release", "check_ci_policy.py", &["--dir", cwd])?,
        );
    } else {
        line("⚠️ No .github/workflows: CI policy check skipped.");
    }

    // 7. Version files agree (devops-release)
    if devops.get_or_none("version_files").truthy() {
        passed &= report(
            "Version files (devops-release)",
            run_checker(
                "devops-release",
                "release_status.py",
                &["version", "--dir", cwd],
            )?,
        );
    } else {
        line("⚠️ devops.version_files is not set: version consistency not checked.");
    }

    // 8. Release candidate acceptance (quality-assurance)
    if devops.get("release_candidates").is_none_or(Value::truthy) {
        if let Some(run) = run {
            let (code, out) = run_checker(
                "quality-assurance",
                "manual_tests.py",
                &["gate", "--run", run, "--dir", cwd],
            )?;
            let ok = code.is_some()
                && report_checker(
                    &format!("Release candidate run {run} (quality-assurance)"),
                    code,
                    &out,
                    true,
                    20,
                );
            if code.is_none() {
                line(&format!("❌ {out}"));
            }
            passed &= ok;
            if promote && run_cmd(&["git", "tag", "-l", run], cwd).1.is_empty() {
                line(&format!(
                    "⚠️ Tag {run} not found locally (git fetch --tags): the promote workflow releases that tag's commit."
                ));
            }
        } else {
            line(
                "⚠️ Release candidates are on but no --run was given: acceptance of the candidate not checked.",
            );
        }
    }

    // 9. Backlog format (plan-memory) and open blocking bugs for the release notes
    let (code, out) = run_checker("plan-memory", "backlog.py", &["validate", "--dir", cwd])?;
    passed &= report_checker("Backlog (plan-memory)", code, &out, true, 20);
    if code.is_some() {
        let result = (|| -> Result<(), Crash> {
            let (bf, backlog) = load_backlog(cwd)?;
            let Some(backlog) = backlog else {
                return Ok(());
            };
            let open_bugs = backlog.blocking_bugs(&bf.blocking_severities)?;
            if !open_bugs.is_empty() {
                let listing: Vec<String> = open_bugs
                    .iter()
                    .map(|b| format!("{} {}", b.id, b.title))
                    .collect();
                line(&format!(
                    "⚠️ Open {} bugs; list them as known limitations in the release notes: {}",
                    join_config("/", &bf.blocking_severities)?,
                    listing.join(", ")
                ));
            }
            let untriaged = bf.triage(&backlog, &[]).map_err(roadmap_crash)?.untriaged;
            if !untriaged.is_empty() {
                line(
                    "❌ Open bugs not triaged (a severity, and the next release at the latest for the bugs that do not block; roadmap.py assign <ID> --release <vX.Y.Z> [--move]):",
                );
                for (b, why) in &untriaged {
                    line(&untriaged_line(b, why));
                }
                passed = false;
            }
            Ok(())
        })();
        if let Err(c) = result {
            line(&format!("ℹ️ Could not read the backlog: {}", exc_str(&c)));
        }
    }

    // 10. Roadmap (plan-memory): items of the release being shipped that are still open
    let (code, out) = run_checker("plan-memory", "roadmap.py", &["validate", "--dir", cwd])?;
    passed &= report_checker("Roadmap (plan-memory)", code, &out, true, 20);
    if code.is_some() {
        let result = (|| -> Result<(), Crash> {
            let (rf, roadmap) = load_roadmap(cwd)?;
            let Some(roadmap) = roadmap else {
                return Ok(());
            };
            let version = run.map(|r| RC_SUFFIX.replace(r, ""));
            let release = match version.as_deref().filter(|v| !v.is_empty()) {
                Some(v) => roadmap.releases.iter().find(|r| r.version == v),
                None => roadmap.current(),
            };
            match (release, version.as_deref().filter(|v| !v.is_empty())) {
                (None, Some(v)) => line(&format!(
                    "⚠️ {v} is not in the roadmap {}; add it or check the version.",
                    rf.rel()?
                )),
                (Some(release), _) => {
                    let (_, backlog) = load_backlog(cwd)?;
                    let mut still_open = Vec::new();
                    for item_id in release.items() {
                        let item = backlog.as_ref().and_then(|b| b.find(&item_id).ok());
                        if let Some(item) = item.filter(|i| !i.closed()) {
                            still_open.push(format!(
                                "{} [{}] {}",
                                item.id,
                                item.status(),
                                item.title
                            ));
                        }
                    }
                    if still_open.is_empty() {
                        line(&format!("✅ Every item of {} is closed.", release.version));
                    } else {
                        line(&format!(
                            "⚠️ Items of {} still open; the user decides whether they move to the next release (roadmap.py assign --move): {}",
                            release.version,
                            still_open.join("; ")
                        ));
                    }
                }
                (None, None) => {}
            }
            Ok(())
        })();
        if let Err(c) = result {
            line(&format!("ℹ️ Could not read the roadmap: {}", exc_str(&c)));
        }
    }

    // 11. No AI attribution in the commits of the release (protocol principle 9)
    passed &= check_commit_attribution(cwd, &cfg)?;

    // 12. Secret hygiene
    passed &= report_secrets(cwd, "✅ Secret hygiene check passed.");

    // 13. Git status
    let (_, status_out) = run_cmd(&["git", "status", "--porcelain"], cwd);
    let dirty = splitlines(&status_out)
        .into_iter()
        .filter(|l| !strip(l).is_empty())
        .count();
    if dirty > 0 {
        line(&format!(
            "⚠️ Uncommitted changes remaining before release: {dirty} files."
        ));
    } else {
        line("✅ Working directory clean for release commit.");
    }

    line(&rule('-'));
    if !passed {
        line("🚨 GATE 8 STATUS: BLOCKED BY COMPLIANCE CHECKS");
    } else if promote {
        line(&format!(
            "🟢 GATE 8 STATUS: READY FOR THE PROMOTION (the user runs the promote workflow{})",
            run.map_or(String::new(), |r| format!(" for {r}"))
        ));
    } else {
        line("🟢 GATE 8 STATUS: READY FOR THE RELEASE PULL REQUEST (the user merges)");
    }
    line(&format!("{}\n", rule('=')));
    Ok(passed)
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "verify_gate.py",
        "Verify gate compliance for dev-cycle-protocol",
    )
    .required_choice_option(
        "--gate",
        &["0", "3", "8", "rc", "gate0", "gate3", "gate8", "gaterc"],
        "Gate to verify (0=product baseline, 3=plan approval, rc=candidate acceptance, 8=release readiness)",
    )
    .option("--dir", "Workspace directory")
    .option("--project", "Project name override")
    .option("--run", "Release candidate run label, for example v1.4.0-rc.1")
    .option(
        "--accept-open-bugs",
        "Gate 3: the user explicitly accepts approving a non-bug plan with open blocking bugs",
    )
    .metavar("REASON");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(e) => return Ok(e.report()),
    };
    let dir = args.value("dir").filter(|d| !d.is_empty()).unwrap_or("");
    let cwd = abspath(dir).to_string_lossy().into_owned();
    let project_name = match args.value("project").filter(|p| !p.is_empty()) {
        Some(p) => p.to_string(),
        None => config(&cwd)?.project_name(),
    };
    let gate = args.value("gate").unwrap_or("").replace("gate", "");
    let run = args.value("run");
    let success = match gate.as_str() {
        "0" => verify_gate_0(&cwd, &project_name)?,
        "3" => verify_gate_3(&cwd, &project_name, args.value("accept_open_bugs"))?,
        "rc" => verify_gate_rc(&cwd, &project_name, run)?,
        _ => verify_gate_8(&cwd, &project_name, run)?,
    };
    Ok(if success { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_credential_files_are_flagged() {
        for path in [
            ".env",
            ".env.local",
            ".env.production",
            "config/prod.env",
            "id_rsa",
            "deploy/id_ed25519",
            "credentials.json",
            ".netrc",
            "certs/server.pem",
            "certs/client.p12",
            "store.pfx",
            "tls/server.key",
            "android/release.keystore",
            "client_secret.json",
            "k8s/secrets.yaml",
            "config/db-secret.yml",
            "private_key.json",
            "gcp-private-key.toml",
            "C:\\repo\\.env",
        ] {
            assert!(is_credential_file(path), "{path}");
        }
    }

    #[test]
    fn code_docs_and_templates_are_not_flagged() {
        for path in [
            // Source code and docs about secrets
            "crates/any-context-core-rs/src/ingestion/secrets.rs",
            "docs/adr/0149-secret-values-are-redacted-before-chunking-with-no-exception.md",
            "docs/plans/any_context_ingestion_secret_files_env_tfvars_key_are_indexed_in_clear.md",
            "src/secret_store.py",
            "src/private_key_loader.ts",
            "tests/test_secrets.py",
            // A secret word in a directory, not in the file name
            "secrets/README.md",
            "private_key/handler.go",
            // Templates
            ".env.example",
            ".env.sample",
            "config/.env.template",
            "secrets.example.yaml",
            "credentials.json.sample",
            // Look-alikes
            "environment.md",
            "keys.md",
            "src/env.rs",
            "monkey.json",
        ] {
            assert!(!is_credential_file(path), "{path}");
        }
    }

    /// A fresh Git repository outside any other.
    fn repo(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("verify-gate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        let root = abspath(&dir.to_string_lossy())
            .to_string_lossy()
            .into_owned();
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.email", "test@example.com"]);
        git(&root, &["config", "user.name", "Test"]);
        root
    }

    fn git(root: &str, args: &[&str]) {
        let st = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git");
        assert!(st.status.success(), "git {args:?}");
    }

    fn write(root: &str, path: &str) {
        let full = std::path::Path::new(root).join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("folders");
        std::fs::write(full, "x\n").expect("file");
    }

    #[test]
    fn secret_hygiene_ignores_code_and_docs_about_secrets() {
        let root = repo("docs");
        for path in [
            "src/ingestion/secrets.rs",
            "docs/adr/0149-secret-values-are-redacted.md",
            "docs/plans/secret_files_plan.md",
            ".env.example",
        ] {
            write(&root, path);
        }
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        assert!(check_secret_hygiene(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn secret_hygiene_reports_committed_and_staged_credentials_once() {
        let root = repo("both");
        write(&root, "README.md");
        write(&root, "config/client_secret.json");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        write(&root, ".env");
        write(&root, "certs/server.pem");
        git(&root, &["add", "-A"]);
        assert_eq!(
            check_secret_hygiene(&root),
            [
                "Tracked credential file committed to Git: config/client_secret.json",
                "Staged credential file ready to be committed: .env",
                "Staged credential file ready to be committed: certs/server.pem",
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn secret_hygiene_works_before_the_first_commit() {
        let root = repo("first");
        write(&root, "id_rsa");
        write(&root, "src/secrets.rs");
        git(&root, &["add", "-A"]);
        assert_eq!(
            check_secret_hygiene(&root),
            ["Staged credential file ready to be committed: id_rsa"]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn feature_branches_need_three_digits_and_a_dash() {
        assert!(is_feature_branch("feat/001-login", "feat"));
        assert!(is_feature_branch("feat/1234-x", "feat"));
        assert!(!is_feature_branch("feat/01-login", "feat"));
        assert!(!is_feature_branch("feat/001login", "feat"));
        assert!(!is_feature_branch("work/001-login", "feat"));
        assert!(is_feature_branch("a.b/001-x", "a.b"));
        assert!(!is_feature_branch("axb/001-x", "a.b"));
    }

    #[test]
    fn commit_records_unpack_into_six_fields() {
        let ok = commit_fields("\nabc\x1fA\x1fa@x\x1fC\x1fc@x\x1fmsg\n").expect("six fields");
        assert_eq!(ok, ["abc", "A", "a@x", "C", "c@x", "msg"]);
        // An empty message leaves a trailing separator that `strip()` removes.
        let short = commit_fields("abc\x1fA\x1fa@x\x1fC\x1fc@x\x1f")
            .err()
            .map(|c| c.0);
        assert_eq!(
            short.as_deref(),
            Some("ValueError: not enough values to unpack (expected 6, got 5)")
        );
    }
}
