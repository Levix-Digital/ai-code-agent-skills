//! `manual_tests.py`: validates the curated manual test suite, reports results per run, starts
//! release candidate runs and gates them.
//!
//! Format (see MANUAL_TESTS_TEMPLATE.md):
//!     ## <Feature>
//!     ### MT-<AREA>-NNN: <title>
//!     - **Covers**: ...
//!     - **Prerequisites**: ...          (optional)
//!     - **Steps**: ...
//!     - **Expected**: ...
//!     - **Result** (<run>): [ ] Pass [ ] Fail [ ] Blocked
//!     - **Notes**: ...

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use architecture_docs::docs::py_regex;
use kernel::out::println;
use kernel::project_config::{ConfigError, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::num::{is_space, strip};
use kernel::py::path::{isabs, isfile, join};
use kernel::py::{Crash, Value, lower, utf8_error_text, write_text};
use regex::Regex;

static RESULT_RE: LazyLock<Regex> = LazyLock::new(|| {
    py_regex(concat!(
        r"^\s*-\s+\*\*Result\*\*(?:\s*\(([^)]*)\))?\s*:\s*",
        r"\[([ xX])\]\s*Pass\s+\[([ xX])\]\s*Fail\s+\[([ xX])\]\s*Blocked\s*$"
    ))
});
static NOTES_PREFIX: LazyLock<Regex> = LazyLock::new(|| py_regex(r"^\s*-\s+\*\*Notes\*\*\s*:?"));

const REQUIRED_FIELDS: [&str; 5] = ["Covers", "Steps", "Expected", "Result", "Notes"];
const STATUSES: [&str; 3] = ["Pass", "Fail", "Blocked"];

fn known_field(name: &str) -> bool {
    REQUIRED_FIELDS.contains(&name) || name == "Prerequisites"
}

/// The length of the run of whitespace at the start of `s`.
fn space_run(s: &str) -> usize {
    s.find(|c| !is_space(c)).unwrap_or(s.len())
}

/// The end of `s` without its trailing whitespace, but at least one character after `start`.
fn lazy_end(s: &str, start: usize) -> usize {
    let trimmed = s.trim_end_matches(is_space).len();
    let one = start + s[start..].chars().next().map_or(0, char::len_utf8);
    trimmed.max(one)
}

/// `FEATURE_RE = ^##\s+(?!#)(.+?)\s*$`: group 1.
fn feature_heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("##")?;
    let run: Vec<usize> = rest
        .char_indices()
        .take_while(|(_, c)| is_space(*c))
        .map(|(i, c)| i + c.len_utf8())
        .collect();
    // `\s+` backtracks from the longest run: the lookahead and `.+?` need a non-`#` after it.
    for &start in run.iter().rev() {
        match rest[start..].chars().next() {
            Some(c) if c != '#' => return Some(&rest[start..lazy_end(rest, start)]),
            _ => {}
        }
    }
    None
}

/// `TEST_RE = ^###\s+(?!#)(\S+?):\s*(.+?)\s*$`: groups 1 and 2.
fn test_heading(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("###")?;
    let start = space_run(rest);
    if start == 0 {
        return None;
    }
    let word_end = start + space_run_end(&rest[start..]);
    let first = rest[start..].chars().next()?;
    if first == '#' {
        return None;
    }
    let first_end = start + first.len_utf8();
    for (i, c) in rest[first_end..word_end].char_indices() {
        if c != ':' {
            continue;
        }
        let colon = first_end + i;
        let after = &rest[colon + 1..];
        if after.is_empty() {
            continue;
        }
        let lead = space_run(after);
        let title = if lead < after.len() {
            &after[lead..lazy_end(after, lead)]
        } else {
            let last = after.chars().next_back().map_or(0, char::len_utf8);
            &after[after.len() - last..]
        };
        return Some((&rest[start..colon], title));
    }
    None
}

/// The length of the run of non-whitespace at the start of `s`.
fn space_run_end(s: &str) -> usize {
    s.find(is_space).unwrap_or(s.len())
}

/// `FIELD_RE = ^\s*-\s+\*\*([A-Za-z ]+)\*\*`: group 1.
fn field(line: &str) -> Option<&str> {
    let rest = line[space_run(line)..].strip_prefix('-')?;
    let lead = space_run(rest);
    if lead == 0 {
        return None;
    }
    let rest = rest[lead..].strip_prefix("**")?;
    let n = rest
        .find(|c: char| !(c.is_ascii_alphabetic() || c == ' '))
        .unwrap_or(rest.len());
    if n == 0 || !rest[n..].starts_with("**") {
        return None;
    }
    Some(&rest[..n])
}

/// `FENCE_RE = ^\s{0,3}(`{3,}|~{3,})`: the fence character.
fn fence(line: &str) -> Option<char> {
    let lead = space_run(line);
    if line[..lead].chars().count() > 3 {
        return None;
    }
    let rest = &line[lead..];
    ['`', '~']
        .into_iter()
        .find(|&f| rest.chars().take(3).filter(|&c| c == f).count() == 3)
}

/// `re.match(r"^\s*", line).group(0)`.
fn indent(line: &str) -> &str {
    &line[..space_run(line)]
}

#[derive(Debug)]
pub struct ManualTest {
    pub id: String,
    pub title: String,
    pub feature: Option<String>,
    /// 0-based.
    pub line: usize,
    /// Field name and its first line, in the order they appear.
    pub fields: Vec<(String, usize)>,
    pub result_line: Option<usize>,
    pub run: Option<String>,
    pub status: Option<&'static str>,
    pub marked: usize,
    pub result_malformed: bool,
    /// The Notes line and its continuation (0-based).
    pub notes_lines: Vec<usize>,
}

impl ManualTest {
    fn new(id: &str, title: &str, feature: Option<String>, line: usize) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            feature,
            line,
            fields: Vec::new(),
            result_line: None,
            run: None,
            status: None,
            marked: 0,
            result_malformed: false,
            notes_lines: Vec::new(),
        }
    }

    fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|(n, _)| n == name)
    }

    /// `str(t.feature)`.
    fn feature_text(&self) -> &str {
        self.feature.as_deref().unwrap_or("None")
    }

    /// `(t.feature or "").lower()`.
    fn feature_key(&self) -> String {
        lower(self.feature.as_deref().unwrap_or(""))
    }

    fn status_label(&self) -> &str {
        self.status.unwrap_or("Not run")
    }
}

pub struct Suite {
    pub path: String,
    newline: &'static str,
    pub lines: Vec<String>,
    pub tests: Vec<ManualTest>,
    pub errors: Vec<String>,
}

impl Suite {
    pub fn new(path: &str, text: &str) -> Self {
        let mut suite = Self {
            path: path.to_string(),
            newline: if text.contains("\r\n") { "\r\n" } else { "\n" },
            lines: text
                .replace("\r\n", "\n")
                .split('\n')
                .map(str::to_string)
                .collect(),
            tests: Vec::new(),
            errors: Vec::new(),
        };
        suite.parse();
        suite
    }

    fn parse(&mut self) {
        let mut feature: Option<String> = None;
        let mut current: Option<usize> = None;
        let mut open_fence: Option<char> = None;
        let mut in_notes = false;
        for (i, line) in self.lines.iter().enumerate() {
            let m_fence = fence(line);
            if let Some(f) = open_fence {
                if m_fence == Some(f) {
                    open_fence = None;
                }
                continue;
            }
            if m_fence.is_some() {
                open_fence = m_fence;
                in_notes = false;
                continue;
            }

            let m_test = test_heading(line);
            if let Some(title) = feature_heading(line).filter(|_| !line.starts_with("###")) {
                feature = Some(title.to_string());
                current = None;
                in_notes = false;
                continue;
            }
            if line.starts_with("###") {
                in_notes = false;
                let Some((id, title)) = m_test else {
                    self.errors.push(format!(
                        "line {}: test heading must be '### MT-<AREA>-NNN: <title>'",
                        i + 1
                    ));
                    current = None;
                    continue;
                };
                let test = ManualTest::new(id, title, feature.clone(), i);
                if feature.is_none() {
                    self.errors.push(format!(
                        "line {}: test {} is not under a '## <Feature>' section",
                        i + 1,
                        test.id
                    ));
                }
                self.tests.push(test);
                current = Some(self.tests.len() - 1);
                continue;
            }
            if line.starts_with('#') {
                current = None;
                in_notes = false;
                continue;
            }
            let Some(k) = current else { continue };
            let test = &mut self.tests[k];

            if let Some(name) = field(line) {
                let name = strip(name);
                in_notes = name == "Notes";
                if !test.has_field(name) {
                    test.fields.push((name.to_string(), i));
                }
                if name == "Result" {
                    test.result_line = Some(i);
                    match RESULT_RE.captures(line) {
                        None => test.result_malformed = true,
                        Some(m) => {
                            let run = m.get(1).map_or("", |g| g.as_str());
                            let run = strip(run);
                            test.run = (!run.is_empty()).then(|| run.to_string());
                            let boxes: Vec<bool> =
                                (2..=4).map(|g| lower(strip(&m[g])) == "x").collect();
                            test.marked = boxes.iter().filter(|b| **b).count();
                            if test.marked == 1 {
                                test.status = boxes.iter().position(|b| *b).map(|p| STATUSES[p]);
                            }
                        }
                    }
                }
                if in_notes {
                    test.notes_lines.push(i);
                }
                continue;
            }
            if !strip(line).is_empty() {
                if in_notes {
                    test.notes_lines.push(i);
                }
            } else if in_notes {
                in_notes = false;
            }
        }
    }

    pub fn notes_text(&self, test: &ManualTest) -> String {
        let mut parts = Vec::new();
        for (k, &i) in test.notes_lines.iter().enumerate() {
            let line = &self.lines[i];
            let line = if k == 0 {
                NOTES_PREFIX.replace(line, "").into_owned()
            } else {
                line.clone()
            };
            parts.push(strip(&line).to_string());
        }
        parts.retain(|p| !p.is_empty());
        parts.join(" ")
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.errors.clone();
        let mut seen: Vec<(&str, usize)> = Vec::new();
        for t in &self.tests {
            let w = format!("line {}: {}", t.line + 1, t.id);
            if !test_id(&t.id) {
                problems.push(format!(
                    "{w}: id must look like MT-<AREA>-NNN (for example MT-SYNC-001)"
                ));
            }
            match seen.iter().find(|(id, _)| *id == t.id) {
                Some((_, first)) => {
                    problems.push(format!("{w}: duplicate id (also at line {})", first + 1))
                }
                None => seen.push((&t.id, t.line)),
            }
            for name in REQUIRED_FIELDS {
                if !t.has_field(name) {
                    problems.push(format!("{w}: missing field '{name}'"));
                }
            }
            for (name, _) in &t.fields {
                if !known_field(name) {
                    problems.push(format!("{w}: unknown field '{name}'"));
                }
            }
            if t.result_malformed {
                problems.push(format!(
                    "{w}: Result line must be '- **Result** (<run>): [ ] Pass [ ] Fail [ ] Blocked'"
                ));
            } else if t.marked > 1 {
                problems.push(format!("{w}: more than one result box is marked"));
            }
        }
        problems
    }

    pub fn select(&self, run: Option<&str>) -> Vec<&ManualTest> {
        self.tests
            .iter()
            .filter(|t| run.is_none() || t.run.as_deref() == run)
            .collect()
    }

    /// Labels the selected tests with `run`, clears their results and notes; returns their
    /// indexes.
    pub fn start_run(
        &mut self,
        run: &str,
        features: &[String],
        ids: &[String],
        everything: bool,
    ) -> Vec<usize> {
        let wanted: BTreeSet<String> = features.iter().map(|f| lower(f)).collect();
        let chosen: Vec<usize> = (0..self.tests.len())
            .filter(|&k| {
                let t = &self.tests[k];
                everything || ids.contains(&t.id) || wanted.contains(&t.feature_key())
            })
            .collect();
        let mut removals = BTreeSet::new();
        for &k in &chosen {
            let t = &self.tests[k];
            let Some(r) = t.result_line else { continue };
            self.lines[r] = format!(
                "{}- **Result** ({run}): [ ] Pass [ ] Fail [ ] Blocked",
                indent(&self.lines[r])
            );
            if let Some((&first, rest)) = t.notes_lines.split_first() {
                self.lines[first] = format!("{}- **Notes**:", indent(&self.lines[first]));
                removals.extend(rest.iter().copied());
            }
        }
        let lines = std::mem::take(&mut self.lines);
        self.lines = lines
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !removals.contains(i))
            .map(|(_, l)| l)
            .collect();
        chosen
    }

    pub fn text(&self) -> String {
        self.lines.join(self.newline)
    }
}

/// `TEST_ID_RE = ^MT-[A-Z][A-Z0-9]*-\d{3}$`.
fn test_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("MT-") else {
        return false;
    };
    let mut chars = rest.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_uppercase()) {
        return false;
    }
    let rest = chars.as_str();
    let n = rest
        .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit()))
        .unwrap_or(rest.len());
    let Some(digits) = rest[n..].strip_prefix('-') else {
        return false;
    };
    digits.chars().count() == 3
        && digits
            .chars()
            .all(|c| kernel::py::num::decimal_value(c).is_some())
}

fn config_crash(e: ConfigError) -> Crash {
    match e {
        ConfigError::Invalid(e) => Crash::new(format!("ValueError: {e}")),
        ConfigError::Crash(c) => c,
    }
}

/// `open(path, encoding="utf-8", newline="").read()`: no newline translation.
fn read_raw(path: &Path) -> Result<String, Crash> {
    let bytes = std::fs::read(path).map_err(|e| kernel::py::io_crash(&e, path))?;
    String::from_utf8(bytes).map_err(|e| {
        Crash::new(format!(
            "UnicodeDecodeError: {}",
            utf8_error_text(e.as_bytes(), &e.utf8_error())
        ))
    })
}

/// `load_suite`: `Err(message)` when the suite file is not there.
pub fn load_suite(
    project_dir: Option<&str>,
    file: Option<&str>,
) -> Result<Result<Suite, String>, Crash> {
    let cfg = load_project_config(project_dir).map_err(config_crash)?;
    let configured = cfg.section("tests").get_or_none("manual_tests");
    let rel = match file.filter(|f| !f.is_empty()) {
        Some(f) => Value::str(f),
        None if configured.truthy() => configured.clone(),
        None => Value::str("tests/MANUAL_TESTS.md"),
    };
    let Value::Str(rel) = rel else {
        return Err(Crash::new(format!(
            "TypeError: expected str, bytes or os.PathLike object, not {}",
            rel.type_name()
        )));
    };
    let path = if isabs(&rel) {
        rel.clone()
    } else {
        join(&cfg.root.to_string_lossy(), &rel)
    };
    if !isfile(&path) {
        return Ok(Err(format!("manual test suite not found at {rel}")));
    }
    let text = read_raw(Path::new(&path))?;
    Ok(Ok(Suite::new(&path, &text)))
}

/// The `{status: count}` of `tests` for Pass, Fail, Blocked and Not run.
fn counts(tests: &[&ManualTest]) -> Vec<(&'static str, usize)> {
    STATUSES
        .iter()
        .chain(&["Not run"])
        .map(|&s| (s, tests.iter().filter(|t| t.status_label() == s).count()))
        .collect()
}

fn cmd_summary(suite: &Suite, run: Option<&str>) {
    let tests = suite.select(run);
    let title = match run.filter(|r| !r.is_empty()) {
        Some(r) => format!("run {r}"),
        None => "all tests".to_string(),
    };
    println(&format!(
        "📋 Manual tests ({title}): {} test(s)",
        tests.len()
    ));
    let mut by_feature: Vec<(&str, Vec<&ManualTest>)> = Vec::new();
    for t in &tests {
        let name = t
            .feature
            .as_deref()
            .filter(|f| !f.is_empty())
            .unwrap_or("(no feature)");
        match by_feature.iter_mut().find(|(f, _)| *f == name) {
            Some((_, items)) => items.push(t),
            None => by_feature.push((name, vec![t])),
        }
    }
    for (feature, items) in &by_feature {
        let shown: Vec<String> = counts(items)
            .into_iter()
            .filter(|(_, v)| *v > 0)
            .map(|(k, v)| format!("{k} {v}"))
            .collect();
        println(&format!("   • {feature}: {}", shown.join(", ")));
    }
    let totals: Vec<String> = counts(&tests)
        .into_iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    println(&format!("   Total: {}", totals.join(", ")));
}

fn cmd_failed(suite: &Suite, run: Option<&str>) -> u8 {
    let bad: Vec<&ManualTest> = suite
        .select(run)
        .into_iter()
        .filter(|t| matches!(t.status, Some("Fail" | "Blocked")))
        .collect();
    if bad.is_empty() {
        let tail = match run.filter(|r| !r.is_empty()) {
            Some(r) => format!(" in run {r}."),
            None => ".".to_string(),
        };
        println(&format!("✅ No Fail or Blocked tests{tail}"));
        return 0;
    }
    for t in bad {
        let notes = suite.notes_text(t);
        let notes = if notes.is_empty() {
            "(no notes)"
        } else {
            &notes
        };
        println(&format!(
            "❌ {} [{}] {}  ({})",
            t.id,
            t.status_label(),
            t.title,
            t.feature_text()
        ));
        println(&format!("   Notes: {notes}"));
    }
    1
}

fn cmd_gate(suite: &Suite, run: &str) -> u8 {
    let tests = suite.select(Some(run));
    if tests.is_empty() {
        println(&format!(
            "❌ Run {run} has no tests. Start it with: manual_tests.py start-run --run {run} ..."
        ));
        return 1;
    }
    let pending: Vec<&&ManualTest> = tests.iter().filter(|t| t.status != Some("Pass")).collect();
    if !pending.is_empty() {
        for t in &pending {
            println(&format!("❌ {} {}: {}", t.id, t.status_label(), t.title));
        }
        println(&format!(
            "\n🚨 Acceptance gate for {run}: {}/{} Pass. Blocked.",
            tests.len() - pending.len(),
            tests.len()
        ));
        return 1;
    }
    println(&format!(
        "🟢 Acceptance gate for {run}: all {} test(s) Pass.",
        tests.len()
    ));
    0
}

fn print_problems(problems: &[String]) {
    for p in problems {
        println(&format!("❌ {p}"));
    }
}

/// `python manual_tests.py <command> [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "manual_tests.py",
        "Curated manual test suite: validate, report, start and gate runs",
    )
    .positional_choice(
        "command",
        &["validate", "summary", "failed", "list", "start-run", "gate"],
        "",
    )
    .option("--dir", "Project directory (default: current directory)")
    .option(
        "--file",
        "Suite path (default: tests.manual_tests or tests/MANUAL_TESTS.md)",
    )
    .option("--run", "Run label, for example v1.4.0-rc.1")
    .append_option("--feature", "Feature (H2 title); repeatable")
    .append_option("--id", "Test id; repeatable")
    .flag("--all", "With start-run: every test");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let run = args.value("run");
    let features = args.list("feature");
    let ids = args.list("id");

    let mut suite = match load_suite(args.value("dir"), args.value("file"))? {
        Ok(s) => s,
        Err(message) => {
            println(&format!("❌ {message}"));
            return Ok(1);
        }
    };

    match args.value("command").unwrap_or_default() {
        "validate" => {
            let problems = suite.validate();
            print_problems(&problems);
            println(&format!(
                "\n{} manual tests: {} test(s), {} problem(s).",
                if problems.is_empty() { "✅" } else { "❌" },
                suite.tests.len(),
                problems.len()
            ));
            Ok(u8::from(!problems.is_empty()))
        }
        "summary" => {
            cmd_summary(&suite, run);
            Ok(0)
        }
        "failed" => Ok(cmd_failed(&suite, run)),
        "list" => {
            let wanted: BTreeSet<String> = features.iter().map(|f| lower(f)).collect();
            for t in &suite.tests {
                if wanted.is_empty() || wanted.contains(&t.feature_key()) {
                    let run = t
                        .run
                        .as_deref()
                        .map_or(String::new(), |r| format!(", run {r}"));
                    println(&format!(
                        "{}  {}  ({}; {}{run})",
                        t.id,
                        t.title,
                        t.feature_text(),
                        t.status_label()
                    ));
                }
            }
            Ok(0)
        }
        "gate" => match run.filter(|r| !r.is_empty()) {
            None => {
                println("❌ gate needs --run");
                Ok(1)
            }
            Some(r) => Ok(cmd_gate(&suite, r)),
        },
        "start-run" => {
            let label = run.filter(|r| !r.is_empty());
            let Some(label) =
                label.filter(|_| !features.is_empty() || !ids.is_empty() || args.flag("all"))
            else {
                println("❌ start-run needs --run and at least one --feature, --id or --all");
                return Ok(1);
            };
            let problems = suite.validate();
            if !problems.is_empty() {
                print_problems(&problems);
                println("Fix the suite before starting a run.");
                return Ok(1);
            }
            let chosen = suite.start_run(label, &features, &ids, args.flag("all"));
            let mut missing: Vec<&str> = ids
                .iter()
                .filter(|i| !suite.tests.iter().any(|t| &t.id == *i))
                .map(String::as_str)
                .collect();
            let keys: BTreeSet<String> = suite.tests.iter().map(ManualTest::feature_key).collect();
            missing.extend(
                features
                    .iter()
                    .filter(|f| !keys.contains(&lower(f)))
                    .map(String::as_str),
            );
            if !missing.is_empty() {
                println(&format!(
                    "❌ Not found: {}. Nothing was written.",
                    missing.join(", ")
                ));
                return Ok(1);
            }
            write_text(Path::new(&suite.path), &suite.text())?;
            let names: Vec<&str> = chosen.iter().map(|&k| suite.tests[k].id.as_str()).collect();
            println(&format!(
                "✅ Run {label} started with {} test(s): {}",
                chosen.len(),
                names.join(", ")
            ));
            Ok(0)
        }
        _ => Ok(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_like_python() {
        assert_eq!(feature_heading("## Sync  "), Some("Sync"));
        assert_eq!(feature_heading("##  #x"), Some(" #x"));
        assert_eq!(feature_heading("## #x"), None);
        assert_eq!(feature_heading("##   "), Some(" "));
        assert_eq!(feature_heading("## "), None);
        assert_eq!(feature_heading("##x"), None);
        assert_eq!(
            test_heading("### MT-A-001: Title  "),
            Some(("MT-A-001", "Title"))
        );
        assert_eq!(test_heading("### a:b:"), Some(("a", "b:")));
        assert_eq!(test_heading("### a: "), Some(("a", " ")));
        assert_eq!(test_heading("### a:"), None);
        assert_eq!(test_heading("### a::"), Some(("a", ":")));
        assert_eq!(test_heading("### :a"), None);
        assert_eq!(test_heading("### #a: b"), None);
    }

    #[test]
    fn fields_and_ids() {
        assert_eq!(field("  - **Notes**: x"), Some("Notes"));
        assert_eq!(field("-**Notes**"), None);
        assert_eq!(field("- **Bad1**"), None);
        assert!(test_id("MT-SYNC-001"));
        assert!(test_id("MT-A1-123"));
        assert!(!test_id("MT-a-001"));
        assert!(!test_id("MT-A-01"));
        assert!(!test_id("MT-A-B-001"));
        assert!(test_id("MT-A-٠٠١"));
        assert_eq!(fence("   ```"), Some('`'));
        assert_eq!(fence("    ```"), None);
        assert_eq!(fence("~~"), None);
    }
}
