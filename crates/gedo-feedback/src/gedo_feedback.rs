//! Gedo Feedback (`gedo_feedback.py`): records what goes wrong with Gedo during real work
//! (missed or false collisions, merges that fall back to text, contract breaks that slip
//! through, slowness, crashes, workarounds) so it becomes input for Gedo's own backlog. One
//! Markdown file per entry, so parallel threads never conflict.
//!
//! Off by default: recording needs `gedo.enabled` true in `.dev-cycle.json` (and
//! `gedo.feedback`, true by default). Reading commands always work.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use kernel::no_ai;
use kernel::out::{out, println};
use kernel::project_config::{BUG_SEVERITIES, ConfigError, load_project_config};
use kernel::py::argparse::{Parsed, Parser};
use kernel::py::json;
use kernel::py::num::strip;
use kernel::py::path::{basename, exists, isdir, isfile, join, makedirs, relpath};
use kernel::py::re::Regex;
use kernel::py::subprocess::spawn_error;
use kernel::py::{
    Crash, Dict, Value, io_crash, lower, read_text, splitlines, today, universal_newlines,
    utf8_error_text, write_text,
};

const KINDS: [(&str, &str); 13] = [
    (
        "missed-collision",
        "A collision between parallel work that Gedo did not report",
    ),
    (
        "false-collision",
        "A collision reported where there was none",
    ),
    (
        "merge-fallback",
        "The merge driver fell back to Git's textual merge",
    ),
    (
        "wrong-merge",
        "A merge without conflict markers that is wrong",
    ),
    (
        "contract-miss",
        "A contract break that the post-merge check did not report",
    ),
    ("identity", "A node identity lost, split or merged wrongly"),
    (
        "performance",
        "An operation slower than Gedo's requirements",
    ),
    ("crash", "A Gedo command or the daemon failed"),
    ("data-loss", "Work or Gedo data lost or corrupted"),
    (
        "setup",
        "Installation, gedo init, hooks or environment problems",
    ),
    (
        "interface",
        "Unclear output, missing JSON, prompts, exit codes",
    ),
    (
        "workaround",
        "Something done around Gedo because it could not do it",
    ),
    ("other", "Anything else"),
];
const STATUSES: [&str; 3] = ["open", "reported", "closed"];
const OPEN_STATUSES: [&str; 2] = ["open", "reported"];
const FIELDS: [&str; 11] = [
    "Id",
    "Kind",
    "Severity",
    "Status",
    "Gedo version",
    "Command",
    "Found",
    "Related",
    "Issue",
    "Recorded",
    "Closed",
];
const SECTIONS: [&str; 6] = [
    "Scenario",
    "Expected",
    "Actual",
    "Reproduction",
    "Workaround",
    "Diagnostics",
];
const REQUIRED_SECTIONS: [&str; 3] = ["Scenario", "Expected", "Actual"];
const DIAGNOSTICS_MAX_LINES: usize = 200;

static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"^# (.+)$", "m"));
static FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"^- \*\*([^*]+)\*\*: (.*)$", "m"));
static SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"^## (.+)$", "m"));
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"^https?://\S+$", ""));

/// What the script's `try` around a command can end with.
enum Fail {
    /// `FeedbackError`.
    Feedback(String),
    Crash(Crash),
}

impl From<Crash> for Fail {
    fn from(c: Crash) -> Self {
        Fail::Crash(c)
    }
}

type Res<T> = Result<T, Fail>;

fn feedback<T>(msg: impl Into<String>) -> Res<T> {
    Err(Fail::Feedback(msg.into()))
}

fn kind_names() -> String {
    KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")
}

fn kind_text(kind: &str) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("", |(_, text)| text)
}

/// The lower-case ASCII letter and digit runs of `text.lower()`: `re.findall(r"[a-z0-9]+")`.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in lower(text).chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            word.push(c);
        } else if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn slugify(text: &str) -> String {
    let parts = words(text);
    let slug = parts.iter().take(6).cloned().collect::<Vec<_>>().join("-");
    if slug.is_empty() {
        "entry".to_string()
    } else {
        slug
    }
}

fn normalize_title(text: &str) -> String {
    words(text).join(" ")
}

/// The `gedo` section of `.dev-cycle.json` and whether recording is on.
struct Settings {
    root: String,
    enabled: bool,
    feedback: bool,
    dir: String,
    repo: Value,
    allowed_terms: Value,
}

impl Settings {
    fn new(start_dir: Option<&str>) -> Res<Settings> {
        let cfg = match load_project_config(start_dir) {
            Ok(cfg) => cfg,
            Err(ConfigError::Invalid(e) | ConfigError::Decode(e)) => {
                return Err(Fail::Crash(Crash::new(format!("ValueError: {e}"))));
            }
            Err(ConfigError::Crash(c)) => return Err(Fail::Crash(c)),
        };
        let root = cfg.root.to_string_lossy().into_owned();
        let gedo = cfg.section("gedo");
        let is_true = |key: &str| matches!(gedo.get(key), Some(Value::Bool(true)));
        let dir = match gedo.get_or_none("feedback_dir") {
            v if !v.truthy() => join(&root, "docs/gedo-feedback"),
            Value::Str(d) => join(&root, d),
            other => {
                return Err(Fail::Crash(Crash::new(format!(
                    "TypeError: join() argument must be str, bytes, or os.PathLike object, not '{}'",
                    other.type_name()
                ))));
            }
        };
        let repo = match gedo.get_or_none("repo") {
            v if v.truthy() => v.clone(),
            _ => Value::str("Levix-Digital/gedo"),
        };
        let allowed_terms = match cfg.section("review").get_or_none("allowed_terms") {
            v if v.truthy() => v.clone(),
            _ => Value::List(Vec::new()),
        };
        Ok(Settings {
            root,
            enabled: is_true("enabled"),
            feedback: is_true("feedback"),
            dir,
            repo,
            allowed_terms,
        })
    }

    fn active(&self) -> bool {
        self.enabled && self.feedback
    }

    fn why_inactive(&self) -> &'static str {
        if !self.enabled {
            return "the project does not use Gedo yet (gedo.enabled is false in .dev-cycle.json)";
        }
        if !self.feedback {
            return "Gedo feedback is turned off (gedo.feedback is false in .dev-cycle.json)";
        }
        ""
    }

    fn require_active(&self) -> Res<()> {
        if !self.active() {
            return feedback(format!("Gedo feedback is off: {}.", self.why_inactive()));
        }
        Ok(())
    }
}

/// One entry: its title, its `- **Field**: value` lines and its `## Section` texts, in the
/// order the file gives them.
struct Entry {
    title: String,
    fields: Dict,
    sections: Dict,
    path: String,
}

impl Entry {
    /// `fields.get(name)`.
    fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).and_then(Value::as_str)
    }

    /// `f"{fields.get(name)}"`: `None` when missing.
    fn field_text(&self, name: &str) -> String {
        self.field(name).unwrap_or("None").to_string()
    }

    fn section(&self, name: &str) -> &str {
        self.sections
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    fn set(&mut self, name: &str, value: &str) {
        self.fields.insert(name, Value::str(value));
    }
}

fn render(entry: &Entry) -> String {
    let mut lines = vec![format!("# {}", entry.title), String::new()];
    for field in FIELDS {
        let value = entry.field(field).unwrap_or("");
        if !value.is_empty() || ["Id", "Kind", "Severity", "Status", "Recorded"].contains(&field) {
            let value = if value.is_empty() { "-" } else { value };
            lines.push(format!("- **{field}**: {value}"));
        }
    }
    for section in SECTIONS {
        let text = strip(entry.section(section));
        if !text.is_empty() {
            lines.extend([
                String::new(),
                format!("## {section}"),
                String::new(),
                text.to_string(),
            ]);
        }
    }
    lines.join("\n") + "\n"
}

fn parse(text: &str, path: &str) -> Res<Entry> {
    let t: Vec<char> = text.chars().collect();
    let Some(title) = TITLE.search(&t) else {
        return feedback(format!("{path}: missing '# <title>' line"));
    };
    let mut fields = Dict::new();
    for m in FIELD.find_iter(&t) {
        let name = m.group(&t, 1).unwrap_or_default();
        let value = m.group(&t, 2).unwrap_or_default();
        let value = strip(&value);
        let value = if value == "-" { "" } else { value };
        fields.insert(name, Value::str(value));
    }
    // `re.split(r"^## (.+)$", text, flags=re.M)`: every heading with the text up to the next.
    let mut sections = Dict::new();
    let headings = SECTION.find_iter(&t);
    for (i, m) in headings.iter().enumerate() {
        let name = m.group(&t, 1).unwrap_or_default();
        let end = headings.get(i + 1).map_or(t.len(), |next| next.start);
        let body: String = t[m.end..end].iter().collect();
        sections.insert(strip(&name), Value::str(strip(&body)));
    }
    Ok(Entry {
        title: strip(&title.group(&t, 1).unwrap_or_default()).to_string(),
        fields,
        sections,
        path: path.to_string(),
    })
}

/// The entries of one project: `<feedback_dir>/<id>.md`.
struct FeedbackStore {
    folder: String,
}

impl FeedbackStore {
    fn path(&self, entry_id: &str) -> String {
        join(&self.folder, &format!("{entry_id}.md"))
    }

    fn all(&self) -> Res<Vec<Entry>> {
        if !isdir(&self.folder) {
            return Ok(Vec::new());
        }
        let listing =
            std::fs::read_dir(&self.folder).map_err(|e| io_crash(&e, Path::new(&self.folder)))?;
        let mut names: Vec<String> = listing
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let mut out = Vec::new();
        for name in names {
            if name.ends_with(".md") && lower(&name) != "readme.md" {
                let path = join(&self.folder, &name);
                out.push(parse(&read_text(Path::new(&path))?, &path)?);
            }
        }
        Ok(out)
    }

    fn get(&self, entry_id: &str) -> Res<Entry> {
        let path = self.path(entry_id);
        if !isfile(&path) {
            return feedback(format!(
                "No Gedo feedback entry '{entry_id}' in {}",
                self.folder
            ));
        }
        parse(&read_text(Path::new(&path))?, &path)
    }

    fn write(&self, entry: &Entry) -> Res<()> {
        makedirs(&self.folder)?;
        let Some(id) = entry.fields.get("Id") else {
            return Err(Fail::Crash(Crash::new("KeyError: 'Id'")));
        };
        let path = self.path(id.as_str().unwrap_or_default());
        write_text(Path::new(&path), &render(entry))?;
        Ok(())
    }

    fn new_id(&self, title: &str, date: &str) -> String {
        let base = format!("{date}-{}", slugify(title));
        let mut candidate = base.clone();
        let mut n = 2;
        while exists(&self.path(&candidate)) {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        candidate
    }

    /// The `Id` of the first open entry with the same kind and title (empty when that entry
    /// has none).
    fn duplicate_of(&self, kind: &str, title: &str) -> Res<Option<String>> {
        let wanted = normalize_title(title);
        for entry in self.all()? {
            let open = entry
                .field("Status")
                .is_some_and(|s| OPEN_STATUSES.contains(&s));
            if open && entry.field("Kind") == Some(kind) && normalize_title(&entry.title) == wanted
            {
                return Ok(Some(entry.field("Id").unwrap_or("").to_string()));
            }
        }
        Ok(None)
    }
}

/// `os.path.splitext(name)[0]`: leading dots do not start an extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if !name[..i].chars().all(|c| c == '.') => &name[..i],
        _ => name,
    }
}

fn validate_entry(entry: &Entry) -> Vec<String> {
    let mut problems = Vec::new();
    let file = basename(Path::new(&entry.path));
    let name = if file.is_empty() {
        entry.field("Id").unwrap_or("?").to_string()
    } else {
        file.clone()
    };
    let one_of =
        |value: Option<&str>, options: &[&str]| value.is_some_and(|v| options.contains(&v));
    if !entry.path.is_empty() && entry.field("Id") != Some(stem(&file)) {
        problems.push(format!(
            "{name}: Id '{}' does not match the file name",
            entry.field_text("Id")
        ));
    }
    let kinds: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
    if !one_of(entry.field("Kind"), &kinds) {
        problems.push(format!(
            "{name}: Kind '{}' is not one of: {}",
            entry.field_text("Kind"),
            kind_names()
        ));
    }
    if !one_of(entry.field("Severity"), &BUG_SEVERITIES) {
        problems.push(format!(
            "{name}: Severity '{}' is not one of: {}",
            entry.field_text("Severity"),
            BUG_SEVERITIES.join(", ")
        ));
    }
    if !one_of(entry.field("Status"), &STATUSES) {
        problems.push(format!(
            "{name}: Status '{}' is not one of: {}",
            entry.field_text("Status"),
            STATUSES.join(", ")
        ));
    }
    let empty = |field: &str| entry.field(field).unwrap_or("").is_empty();
    if entry.field("Status") == Some("reported") && empty("Issue") {
        problems.push(format!("{name}: a reported entry needs its Issue link"));
    }
    if entry.field("Status") == Some("closed") && empty("Closed") {
        problems.push(format!("{name}: a closed entry needs its Closed reason"));
    }
    for section in REQUIRED_SECTIONS {
        if entry.section(section).is_empty() {
            problems.push(format!("{name}: missing section '## {section}'"));
        }
    }
    problems
}

/// `subprocess.run(["gedo", "doctor", "--json"], capture_output=True, text=True, timeout=30)`:
/// `None` when it timed out.
fn doctor_output() -> Res<Option<(i32, String, String)>> {
    let mut child = match Command::new("gedo")
        .args(["doctor", "--json"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let (not_found, msg) = spawn_error(&e, "gedo");
            return Err(Fail::Crash(Crash::new(if not_found {
                format!("FileNotFoundError: {msg}")
            } else {
                msg
            })));
        }
    };
    let pipe = |mut p: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = p.read_to_end(&mut bytes);
            bytes
        })
    };
    let stdout = child.stdout.take().map(|p| pipe(Box::new(p)));
    let stderr = child.stderr.take().map(|p| pipe(Box::new(p)));
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| Crash::new(format!("OSError: {e}")))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let collect = |h: Option<std::thread::JoinHandle<Vec<u8>>>| -> Result<String, Crash> {
        let bytes = h.and_then(|h| h.join().ok()).unwrap_or_default();
        match std::str::from_utf8(&bytes) {
            Ok(text) => Ok(universal_newlines(text)),
            Err(e) => Err(Crash::new(format!(
                "UnicodeDecodeError: {}",
                utf8_error_text(&bytes, &e)
            ))),
        }
    };
    let out = collect(stdout)?;
    let err = collect(stderr)?;
    Ok(Some((returncode(status), out, err)))
}

fn returncode(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return -signal;
        }
    }
    status.code().unwrap_or(-1)
}

/// Output of `gedo doctor --json`, or a note saying why it is missing; never fails.
fn run_doctor() -> Res<String> {
    let (code, stdout, stderr) = match doctor_output() {
        Ok(Some(done)) => done,
        Ok(None) => return Ok("gedo doctor: timed out after 30 s.".to_string()),
        Err(Fail::Crash(c)) if c.0.starts_with("FileNotFoundError: ") => {
            return Ok("gedo doctor: the gedo command was not found on PATH.".to_string());
        }
        Err(e) => return Err(e),
    };
    let output = match strip(&stdout) {
        "" => strip(&stderr),
        text => text,
    };
    Ok(format!(
        "gedo doctor --json (exit {code}):\n\n```json\n{output}\n```"
    ))
}

/// `open(path, encoding="utf-8", errors="replace").read()`.
fn read_replace(path: &str) -> Res<String> {
    let bytes = std::fs::read(path).map_err(|e| io_crash(&e, Path::new(path)))?;
    Ok(universal_newlines(&String::from_utf8_lossy(&bytes)))
}

fn read_diagnostics(path: &str) -> Res<String> {
    let text = read_replace(path)?;
    let mut lines = splitlines(&text);
    let mut note = String::new();
    if lines.len() > DIAGNOSTICS_MAX_LINES {
        note = format!(
            "\n\n(last {DIAGNOSTICS_MAX_LINES} of {} lines of {})",
            lines.len(),
            basename(Path::new(path))
        );
        lines = lines.split_off(lines.len() - DIAGNOSTICS_MAX_LINES);
    }
    Ok(format!("```text\n{}\n```{note}", lines.join("\n")))
}

/// (title, body) of the Gedo issue for an entry, checked against protocol principle 9.
fn issue_text(entry: &Entry, allowed: &Value) -> Res<(String, String)> {
    let kind = entry.field_text("Kind");
    let title = format!("[{kind}] {}", entry.title);
    let mut meta = vec![
        format!(
            "- **Kind**: {kind} ({})",
            entry.field("Kind").map_or("", kind_text)
        ),
        format!("- **Severity**: {}", entry.field_text("Severity")),
    ];
    for field in ["Gedo version", "Command", "Related"] {
        if let Some(value) = entry.field(field).filter(|v| !v.is_empty()) {
            meta.push(format!("- **{field}**: {value}"));
        }
    }
    meta.push(format!("- **Recorded**: {}", entry.field_text("Recorded")));
    let mut body = vec![
        "Found while using Gedo on real work.".to_string(),
        String::new(),
    ];
    body.extend(meta);
    for section in SECTIONS {
        let text = entry.section(section);
        if !text.is_empty() {
            body.extend([
                String::new(),
                format!("## {section}"),
                String::new(),
                text.to_string(),
            ]);
        }
    }
    let text = body.join("\n") + "\n";
    if let Err(msg) = no_ai::check(&format!("{title}\n{text}"), "the issue", allowed)? {
        return Err(Fail::Crash(Crash::new(format!("ValueError: {msg}"))));
    }
    Ok((title, text))
}

fn summary(entry: &Entry) -> Dict {
    let field = |name: &str| entry.field(name).map_or(Value::Null, Value::str);
    let mut d = Dict::new();
    d.insert("id", field("Id"));
    d.insert("title", Value::str(&entry.title));
    d.insert("kind", field("Kind"));
    d.insert("severity", field("Severity"));
    d.insert("status", field("Status"));
    d.insert(
        "issue",
        match entry.field("Issue") {
            Some(issue) if !issue.is_empty() => Value::str(issue),
            _ => Value::Null,
        },
    );
    d.insert("recorded", field("Recorded"));
    d
}

fn print_json(value: &Value) -> Res<()> {
    println(&json::dumps_ascii(value, Some(2))?);
    Ok(())
}

// ----------------------------------------------------------------- commands

fn cmd_status(settings: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    let entries = store.all()?;
    let count = |s: &str| {
        entries
            .iter()
            .filter(|e| e.field("Status") == Some(s))
            .count()
    };
    if args.flag("json") {
        let mut counts = Dict::new();
        for s in STATUSES {
            counts.insert(s, Value::int(count(s) as i64));
        }
        let mut data = Dict::new();
        data.insert("active", Value::Bool(settings.active()));
        data.insert("reason", Value::str(settings.why_inactive()));
        data.insert("dir", Value::str(&settings.dir));
        data.insert("repo", settings.repo.clone());
        data.insert("counts", Value::Dict(counts));
        print_json(&Value::Dict(data))?;
        return Ok(0);
    }
    let state = if settings.active() {
        "on".to_string()
    } else {
        format!("off ({})", settings.why_inactive())
    };
    println(&format!("🧪 Gedo feedback: {state}"));
    println(&format!(
        "   Entries: {} ({} open, {} reported, {} closed)",
        relpath(&settings.dir, &settings.root)?,
        count("open"),
        count("reported"),
        count("closed")
    ));
    println(&format!("   Issues go to: {}", settings.repo.py_str()));
    Ok(0)
}

fn cmd_add(settings: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    settings.require_active()?;
    let kind = args.value("kind").unwrap_or_default();
    let title = args.value("title").unwrap_or_default();
    let severity = args.value("severity").unwrap_or("medium");
    let text = |name: &str| args.value(name).unwrap_or_default().to_string();
    if !args.flag("allow_duplicate")
        && let Some(dup) = store.duplicate_of(kind, title)?
        && !dup.is_empty()
    {
        return feedback(format!(
            "An open entry with the same kind and title exists: {dup}. Add to it instead, or pass --allow-duplicate."
        ));
    }
    let mut diagnostics = Vec::new();
    if let Some(path) = args.value("diagnostics_file").filter(|p| !p.is_empty()) {
        diagnostics.push(read_diagnostics(path)?);
    }
    if args.flag("doctor") {
        diagnostics.push(run_doctor()?);
    }
    let date = today();
    let mut fields = Dict::new();
    for (name, value) in [
        ("Id", store.new_id(title, &date)),
        ("Kind", kind.to_string()),
        ("Severity", severity.to_string()),
        ("Status", "open".to_string()),
        ("Gedo version", text("gedo_version")),
        ("Command", text("command")),
        ("Found", text("found_in")),
        ("Related", text("related")),
        ("Issue", String::new()),
        ("Recorded", date.clone()),
    ] {
        fields.insert(name, Value::str(value));
    }
    let mut sections = Dict::new();
    for (name, value) in [
        ("Scenario", text("scenario")),
        ("Expected", text("expected")),
        ("Actual", text("actual")),
        ("Reproduction", text("reproduction")),
        ("Workaround", text("workaround")),
        ("Diagnostics", diagnostics.join("\n\n")),
    ] {
        sections.insert(name, Value::str(value));
    }
    let entry = Entry {
        title: strip(title).to_string(),
        fields,
        sections,
        path: String::new(),
    };
    store.write(&entry)?;
    let id = entry.field_text("Id");
    println(&format!("✅ {id}: {} ({kind}, {severity})", entry.title));
    println(&format!(
        "   {}",
        relpath(&store.path(&id), &settings.root)?
    ));
    Ok(0)
}

fn cmd_list(_: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    let mut entries = store.all()?;
    if let Some(status) = args.value("status").filter(|s| !s.is_empty()) {
        entries.retain(|e| e.field("Status") == Some(status));
    } else if !args.flag("all") {
        entries.retain(|e| {
            e.field("Status")
                .is_some_and(|s| OPEN_STATUSES.contains(&s))
        });
    }
    if let Some(kind) = args.value("kind").filter(|k| !k.is_empty()) {
        entries.retain(|e| e.field("Kind") == Some(kind));
    }
    if args.flag("json") {
        let rows = entries.iter().map(|e| Value::Dict(summary(e))).collect();
        print_json(&Value::List(rows))?;
        return Ok(0);
    }
    if entries.is_empty() {
        println("No Gedo feedback entries.");
    }
    for e in &entries {
        let issue = match e.field("Issue") {
            Some(issue) if !issue.is_empty() => format!(" -> {issue}"),
            _ => String::new(),
        };
        println(&format!(
            "{}  [{}] {}, {}: {}{issue}",
            e.field_text("Id"),
            e.field_text("Status"),
            e.field_text("Kind"),
            e.field_text("Severity"),
            e.title
        ));
    }
    Ok(0)
}

fn cmd_show(_: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    let entry = store.get(args.value("id").unwrap_or_default())?;
    if args.flag("json") {
        let mut data = summary(&entry);
        data.insert("fields", Value::Dict(entry.fields.clone()));
        data.insert("sections", Value::Dict(entry.sections.clone()));
        print_json(&Value::Dict(data))?;
    } else {
        out(&render(&entry));
    }
    Ok(0)
}

fn cmd_issue(settings: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    settings.require_active()?;
    let id = args.value("id").unwrap_or_default();
    let entry = store.get(id)?;
    if entry.field("Status") != Some("open") {
        return feedback(format!(
            "{id} is {}; only open entries become issues.",
            entry.field_text("Status")
        ));
    }
    let (title, body) = issue_text(&entry, &settings.allowed_terms)?;
    let body_file = args.value("body_file").filter(|f| !f.is_empty());
    if let Some(file) = body_file {
        write_text(Path::new(file), &body)?;
    }
    if args.flag("json") {
        let mut data = Dict::new();
        data.insert("repo", settings.repo.clone());
        data.insert("title", Value::str(&title));
        data.insert("body", Value::str(&body));
        print_json(&Value::Dict(data))?;
        return Ok(0);
    }
    let repo = settings.repo.py_str();
    println(&format!("Repository: {repo}"));
    println(&format!("Title: {title}"));
    if let Some(file) = body_file {
        println(&format!("Body: {file}"));
        println(&format!(
            "Create it: gh issue create --repo {repo} --title \"{title}\" --body-file \"{file}\""
        ));
    } else {
        println("");
        out(&body);
    }
    Ok(0)
}

fn cmd_reported(_: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    let id = args.value("id").unwrap_or_default();
    let issue = args.value("issue").unwrap_or_default();
    let mut entry = store.get(id)?;
    let t: Vec<char> = issue.chars().collect();
    if URL.match_start(&t).is_none() {
        return feedback("--issue must be the issue URL");
    }
    entry.set("Status", "reported");
    entry.set("Issue", issue);
    store.write(&entry)?;
    println(&format!("✅ {id} reported: {issue}"));
    Ok(0)
}

fn cmd_close(_: &Settings, store: &FeedbackStore, args: &Parsed) -> Res<u8> {
    let id = args.value("id").unwrap_or_default();
    let reason = args.value("reason").unwrap_or_default();
    let mut entry = store.get(id)?;
    entry.set("Status", "closed");
    entry.set("Closed", &format!("{}: {reason}", today()));
    store.write(&entry)?;
    println(&format!("✅ {id} closed: {reason}"));
    Ok(0)
}

fn cmd_validate(_: &Settings, store: &FeedbackStore, _: &Parsed) -> Res<u8> {
    let entries = match store.all() {
        Ok(entries) => entries,
        Err(Fail::Feedback(msg)) => {
            println(&format!("❌ {msg}"));
            return Ok(1);
        }
        Err(e) => return Err(e),
    };
    let problems: Vec<String> = entries.iter().flat_map(validate_entry).collect();
    for p in &problems {
        println(&format!("❌ {p}"));
    }
    if problems.is_empty() {
        println(&format!(
            "✅ {} Gedo feedback entries valid.",
            entries.len()
        ));
    }
    Ok(if problems.is_empty() { 0 } else { 1 })
}

fn command_parser(name: &str) -> Parser {
    let p = Parser::new(&format!("gedo_feedback.py {name}"), "");
    let kinds: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
    match name {
        "status" => p.flag("--json", ""),
        "add" => p
            .required_choice_option("--kind", &kinds, "")
            .required_option("--title", "")
            .choice_option("--severity", &BUG_SEVERITIES, "")
            .required_option(
                "--scenario",
                "What was being done, and how the work was split",
            )
            .required_option("--expected", "")
            .required_option("--actual", "")
            .option("--reproduction", "Minimal steps, without proprietary code")
            .option("--workaround", "What was done instead")
            .option("--command", "The Gedo or Git command involved")
            .option("--gedo-version", "")
            .option("--found-in", "Branch, pull request or context")
            .option(
                "--related",
                "Gedo backlog items or proposals, e.g. 'G3, FEAT-074'",
            )
            .option(
                "--diagnostics-file",
                "A log excerpt to attach (last 200 lines)",
            )
            .flag("--doctor", "Attach the output of 'gedo doctor --json'")
            .flag("--allow-duplicate", ""),
        "list" => p
            .choice_option("--status", &STATUSES, "")
            .choice_option("--kind", &kinds, "")
            .flag("--all", "")
            .flag("--json", ""),
        "show" => p.positional("id").flag("--json", ""),
        "issue" => p
            .positional("id")
            .option("--body-file", "Write the body to this file")
            .flag("--json", ""),
        "reported" => p.positional("id").required_option("--issue", ""),
        "close" => p.positional("id").required_option("--reason", ""),
        _ => p,
    }
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

/// The exceptions `main` catches besides `FeedbackError` (`ValueError`, `OSError`): their text.
fn caught(c: &Crash) -> Option<&str> {
    let (kind, msg) = c.0.split_once(": ").unwrap_or((&c.0, ""));
    let caught = matches!(
        kind,
        "ValueError"
            | "UnicodeDecodeError"
            | "UnicodeEncodeError"
            | "UnicodeError"
            | "JSONDecodeError"
            | "OSError"
            | "FileNotFoundError"
            | "FileExistsError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "PermissionError"
            | "BlockingIOError"
            | "TimeoutError"
            | "InterruptedError"
    );
    caught.then_some(msg)
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "gedo_feedback.py",
        "Record what goes wrong with Gedo, for Gedo's own backlog",
    )
    .option("--dir", "Project directory (default: current directory)")
    .commands(
        "action",
        &[
            ("status", "Whether recording is on, and the entry counts"),
            ("add", "Record a new entry"),
            ("list", "List entries (open and reported by default)"),
            ("show", "Show one entry"),
            (
                "issue",
                "Title and body of the Gedo issue for an open entry",
            ),
            ("reported", "Mark an entry as reported, with its issue URL"),
            (
                "close",
                "Close an entry (fixed in Gedo, not a Gedo problem, ...)",
            ),
            ("validate", "Check every entry"),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let result = Settings::new(args.value("dir")).and_then(|settings| {
        let store = FeedbackStore {
            folder: settings.dir.clone(),
        };
        let command = match args.value("action").unwrap_or_default() {
            "status" => cmd_status,
            "add" => cmd_add,
            "list" => cmd_list,
            "show" => cmd_show,
            "issue" => cmd_issue,
            "reported" => cmd_reported,
            "close" => cmd_close,
            _ => cmd_validate,
        };
        command(&settings, &store, &args)
    });
    match result {
        Ok(code) => Ok(code),
        Err(Fail::Feedback(msg)) => {
            println(&format!("❌ {msg}"));
            Ok(if msg.contains("is off") { 2 } else { 1 })
        }
        Err(Fail::Crash(c)) => match caught(&c) {
            Some(msg) => {
                println(&format!("❌ {msg}"));
                Ok(1)
            }
            None => Err(c),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_keep_six_ascii_words() {
        assert_eq!(
            slugify("Radar missed an edit of the same function"),
            "radar-missed-an-edit-of-the"
        );
        assert_eq!(slugify("¿¡!"), "entry");
        assert_eq!(normalize_title("  Radar,  MISSED! "), "radar missed");
    }

    #[test]
    fn stems_like_splitext() {
        assert_eq!(stem("2026-01-01-a.md"), "2026-01-01-a");
        assert_eq!(stem(".md"), ".md");
        assert_eq!(stem("...md"), "...md");
        assert_eq!(stem("a..md"), "a.");
    }

    #[test]
    fn parse_reads_what_render_writes() {
        let mut fields = Dict::new();
        fields.insert("Id", Value::str("x"));
        fields.insert("Kind", Value::str("crash"));
        let mut sections = Dict::new();
        sections.insert("Scenario", Value::str("  one\ntwo  "));
        let entry = Entry {
            title: "T".into(),
            fields,
            sections,
            path: String::new(),
        };
        let text = render(&entry);
        assert_eq!(
            text,
            "# T\n\n- **Id**: x\n- **Kind**: crash\n- **Severity**: -\n- **Status**: -\n- **Recorded**: -\n\n## Scenario\n\none\ntwo\n"
        );
        let back = parse(&text, "p").ok().expect("parses");
        assert_eq!(back.field("Severity"), Some(""));
        assert_eq!(back.section("Scenario"), "one\ntwo");
    }
}
