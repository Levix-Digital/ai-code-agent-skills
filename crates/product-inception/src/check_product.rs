//! `check_product.py`: validates the two product documents written in Phase 0 (Product
//! Inception): the business document (default docs/product/VISION.md) and the technical
//! specification (default docs/product/TECH_SPEC.md). Checks the required sections, leftover
//! template placeholders, empty sections, the language (English unless configured) and a length
//! limit. Open questions are reported as warnings: the user decides at gate 0 whether they block
//! the baseline. `--init` creates the missing documents from the templates (never overwrites).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use architecture_docs::docs::{
    cmp_int, fenced_lines, foreign_language_hits, headings, py_regex, strip_code,
};
use kernel::out::println;
use kernel::project_config::{ConfigError, ProjectConfig, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::num::{self, is_space, strip};
use kernel::py::path::{dirname, exists, isfile, join, makedirs};
use kernel::py::unicode::ignorecase_eq;
use kernel::py::{Crash, Dict, Value, io_crash, json, read_text};
use regex::Regex;

const DEFAULT_MAX_LINES: &str = "400";
const KINDS: [&str; 2] = ["vision", "tech_spec"];
const OPEN_QUESTIONS: &str = "Open Questions";

/// The templates `--init` copies, built into the engine.
fn template(kind: &str) -> &'static str {
    match kind {
        "vision" => include_str!("../../../product-inception/templates/VISION_TEMPLATE.md"),
        _ => include_str!("../../../product-inception/templates/TECH_SPEC_TEMPLATE.md"),
    }
}

fn label(kind: &str) -> &'static str {
    match kind {
        "vision" => "Business document",
        _ => "Technical specification",
    }
}

fn default_path(kind: &str) -> &'static str {
    match kind {
        "vision" => "docs/product/VISION.md",
        _ => "docs/product/TECH_SPEC.md",
    }
}

fn required_sections(kind: &str) -> &'static [&'static str] {
    match kind {
        "vision" => &[
            "Problem",
            "Target Users",
            "Value Proposition",
            "Business Model",
            "Success Metrics",
            "Scope",
            "Non-Goals",
            "Risks and Assumptions",
            "Open Questions",
        ],
        _ => &[
            "Platforms and Stack",
            "Architecture Overview",
            "Non-Functional Requirements",
            "Integrations",
            "Data",
            "Constraints",
            "Open Questions",
        ],
    }
}

struct CheckResult {
    kind: &'static str,
    rel: String,
    errors: Vec<String>,
    warnings: Vec<String>,
    open_questions: Vec<String>,
}

impl CheckResult {
    fn new(kind: &'static str, rel: &str) -> Self {
        CheckResult {
            kind,
            rel: rel.to_string(),
            errors: Vec::new(),
            warnings: Vec::new(),
            open_questions: Vec::new(),
        }
    }

    fn error(&mut self, line: usize, msg: &str) {
        self.errors.push(format!("{}:{line}: {msg}", self.rel));
    }

    fn warn(&mut self, line: usize, msg: &str) {
        self.warnings.push(format!("{}:{line}: {msg}", self.rel));
    }

    fn to_dict(&self) -> Value {
        let strs = |v: &[String]| Value::List(v.iter().map(Value::str).collect());
        let mut d = Dict::new();
        d.insert("kind", Value::str(self.kind));
        d.insert("file", Value::str(&self.rel));
        d.insert("errors", strs(&self.errors));
        d.insert("warnings", strs(&self.warnings));
        d.insert("open_questions", strs(&self.open_questions));
        Value::Dict(d)
    }
}

/// H2 title -> (heading line, body lines up to the next H2).
fn section_bodies<'a>(text: &str, lines: &[&'a str]) -> HashMap<String, (usize, Vec<&'a str>)> {
    let h2: Vec<(usize, String)> = headings(text)
        .into_iter()
        .filter(|(_, level, _)| *level == 2)
        .map(|(n, _, title)| (n, title))
        .collect();
    let mut out = HashMap::new();
    for (i, (n, title)) in h2.iter().enumerate() {
        let end = h2.get(i + 1).map_or(lines.len(), |(next, _)| next - 1);
        let body = lines
            .get(*n..end.min(lines.len()))
            .map(<[&str]>::to_vec)
            .unwrap_or_default();
        out.insert(strip(title).to_string(), (*n, body));
    }
    out
}

fn meaningful<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    lines
        .iter()
        .map(|l| strip(l))
        .filter(|l| !l.is_empty() && !l.starts_with("<!--"))
        .collect()
}

/// `re.compile(r"^(none|n/?a|no open questions)\.?$", re.I).match(text)` on stripped text,
/// with Python's case-insensitive matching (`İ` and `ı` match `i`, `ſ` matches `s`).
fn no_questions(text: &str) -> bool {
    let text = text.strip_suffix('.').unwrap_or(text);
    ["none", "na", "n/a", "no open questions"]
        .iter()
        .any(|word| {
            text.chars().count() == word.chars().count()
                && text
                    .chars()
                    .zip(word.chars())
                    .all(|(c, p)| ignorecase_eq(c, p))
        })
}

/// `PLACEHOLDER_RE.search(text)`: `(?<!!)\[(?![ xX]\])(?!\^)[^\]\n]{3,}\](?![(\[:])`, a
/// template placeholder: "[...]" that is not a link, a checkbox or a footnote.
fn placeholder(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        if chars[i] != '[' || (i > 0 && chars[i - 1] == '!') {
            continue;
        }
        let next = chars.get(i + 1).copied();
        if matches!(next, Some(' ' | 'x' | 'X')) && chars.get(i + 2) == Some(&']') {
            continue;
        }
        if next == Some('^') {
            continue;
        }
        let run = chars[i + 1..]
            .iter()
            .take_while(|c| **c != ']' && **c != '\n')
            .count();
        let close = i + 1 + run;
        if run >= 3
            && chars.get(close) == Some(&']')
            && !matches!(chars.get(close + 1), Some('(' | '[' | ':'))
        {
            return Some(chars[i..=close].iter().collect());
        }
    }
    None
}

fn check_text(
    kind: &'static str,
    rel: &str,
    text: &str,
    max_lines: &str,
    language: &Value,
) -> CheckResult {
    static LIST_MARK: LazyLock<Regex> = LazyLock::new(|| py_regex(r"^[-*]\s+|^\d+\.\s+"));
    static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new("`[^`]*`").expect("valid"));
    let mut res = CheckResult::new(kind, rel);
    let lines: Vec<&str> = text.split('\n').collect();
    let hs = headings(text);
    if !hs.iter().any(|h| h.1 == 1) {
        res.error(1, "missing title (a single '# ...' heading)");
    }

    let bodies = section_bodies(text, &lines);
    for name in required_sections(kind) {
        let Some((n, body)) = bodies.get(*name) else {
            res.error(1, &format!("missing section: ## {name}"));
            continue;
        };
        let content = meaningful(body);
        if content.is_empty() {
            res.error(*n, &format!("section '{name}' is empty"));
        } else if *name == OPEN_QUESTIONS && !(content.len() == 1 && no_questions(content[0])) {
            res.open_questions = content
                .iter()
                .map(|c| LIST_MARK.replace_all(c, "").into_owned())
                .collect();
            res.warn(
                *n,
                &format!(
                    "{} open question(s): answer them or let the user accept them at gate 0",
                    res.open_questions.len()
                ),
            );
        }
    }

    let plain = strip_code(text);
    let plain: Vec<&str> = plain.split('\n').collect();
    let fenced = fenced_lines(&lines);
    for (i, ((line, stripped), in_code)) in lines.iter().zip(&plain).zip(&fenced).enumerate() {
        let n = i + 1;
        if *in_code {
            continue;
        }
        if line.contains("ACTION REQUIRED") {
            res.error(n, "template instruction left in place");
        }
        if let Some(m) = placeholder(&INLINE_CODE.replace_all(line, "")) {
            let shown: String = m.chars().take(60).collect();
            res.error(n, &format!("template placeholder left in place: {shown}"));
        }
        let is_heading = line.trim_start_matches(is_space).starts_with('#');
        let hits = foreign_language_hits(if is_heading { line } else { stripped }, language);
        if !hits.is_empty() {
            let msg = format!("text may not be in English: {}", hits.join(", "));
            if is_heading {
                res.error(n, &msg);
            } else {
                res.warn(n, &msg);
            }
        }
    }

    let count = lines.len().to_string();
    if cmp_int(&count, max_lines) == Ordering::Greater {
        res.warn(
            lines.len(),
            &format!(
                "{count} lines (limit {max_lines}): keep the essentials; details go to the technical docs"
            ),
        );
    }
    res
}

/// `product_paths(cfg)`: kind -> configured path (relative to the root).
fn product_paths(cfg: &ProjectConfig) -> Vec<(&'static str, Value)> {
    let section = cfg.section("product");
    KINDS
        .iter()
        .map(|kind| {
            let v = section.get_or_none(kind);
            let rel = if v.truthy() {
                v.clone()
            } else {
                Value::str(default_path(kind))
            };
            (*kind, rel)
        })
        .collect()
}

/// `os.path.join(cfg.root, rel)`.
fn join_root(cfg: &ProjectConfig, rel: &Value) -> Result<(String, String), Crash> {
    match rel {
        Value::Str(s) => Ok((join(&cfg.root.to_string_lossy(), s), s.clone())),
        other => Err(Crash::new(format!(
            "TypeError: join() argument must be str, bytes, or os.PathLike object, not '{}'",
            other.type_name()
        ))),
    }
}

fn load(dir: Option<&str>) -> Result<ProjectConfig, Crash> {
    match load_project_config(dir) {
        Ok(c) => Ok(c),
        Err(ConfigError::Invalid(e)) => Err(Crash::new(format!("ValueError: {e}"))),
        Err(ConfigError::Crash(c)) => Err(c),
    }
}

fn run_checks(dir: Option<&str>) -> Result<Vec<CheckResult>, Crash> {
    let cfg = load(dir)?;
    let configured = cfg.section("product").get_or_none("max_lines");
    let max_lines = if configured.truthy() {
        num::int_value(configured)?
    } else {
        DEFAULT_MAX_LINES.to_string()
    };
    let default_language = Value::str("en");
    let language = cfg
        .section("architecture_docs")
        .get("language")
        .unwrap_or(&default_language);
    let mut results = Vec::new();
    for (kind, rel) in product_paths(&cfg) {
        let (path, rel) = join_root(&cfg, &rel)?;
        if !isfile(&path) {
            let mut res = CheckResult::new(kind, &rel);
            res.error(
                1,
                &format!(
                    "{} not found (create it with check_product.py --init)",
                    label(kind).to_lowercase()
                ),
            );
            results.push(res);
            continue;
        }
        let text = read_text(Path::new(&path))?;
        results.push(check_text(kind, &rel, &text, &max_lines, language));
    }
    Ok(results)
}

fn init_docs(dir: Option<&str>) -> Result<Vec<String>, Crash> {
    let cfg = load(dir)?;
    let mut created = Vec::new();
    for (kind, rel) in product_paths(&cfg) {
        let (path, rel) = join_root(&cfg, &rel)?;
        if exists(&path) {
            continue;
        }
        makedirs(&dirname(&path))?;
        std::fs::write(&path, template(kind)).map_err(|e| io_crash(&e, Path::new(&path)))?;
        created.push(rel);
    }
    Ok(created)
}

/// `python check_product.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_product.py",
        "Validate the product documents of Phase 0 (product inception)",
    )
    .option("--dir", "Project directory (default: current directory)")
    .flag("--init", "Create the missing documents from the templates")
    .flag("--json", "");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    if args.flag("init") {
        let created = init_docs(args.value("dir"))?;
        for rel in &created {
            println(&format!("✅ Created {rel} from the template"));
        }
        if created.is_empty() {
            println("ℹ️ Both product documents already exist.");
        }
        return Ok(0);
    }

    let results = run_checks(args.value("dir"))?;
    if args.flag("json") {
        let list = Value::List(results.iter().map(CheckResult::to_dict).collect());
        println(&json::dumps(&list, Some(2))?);
    } else {
        for r in &results {
            for e in &r.errors {
                println(&format!("❌ {e}"));
            }
            for w in &r.warnings {
                println(&format!("⚠️ {w}"));
            }
            for q in &r.open_questions {
                println(&format!("   ❓ {q}"));
            }
        }
        let errors: usize = results.iter().map(|r| r.errors.len()).sum();
        let warnings: usize = results.iter().map(|r| r.warnings.len()).sum();
        println(&format!(
            "\n{} product-inception: {errors} error(s), {warnings} warning(s).",
            if errors == 0 { "✅" } else { "❌" }
        ));
    }
    Ok(u8::from(results.iter().any(|r| !r.errors.is_empty())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_not_links_checkboxes_or_footnotes() {
        assert_eq!(
            placeholder("a [Product name] b").as_deref(),
            Some("[Product name]")
        );
        assert_eq!(placeholder("[the survey](survey.md)"), None);
        assert_eq!(placeholder("- [x] done"), None);
        assert_eq!(placeholder("![an image]"), None);
        assert_eq!(placeholder("[^note] and [ab]"), None);
        assert_eq!(placeholder("[abc]: ref"), None);
        assert_eq!(placeholder("[ab] [abcd]").as_deref(), Some("[abcd]"));
    }

    #[test]
    fn no_questions_matches_like_python() {
        for t in [
            "None",
            "N/A",
            "na.",
            "No open questions.",
            "NONE",
            "No open questİons",
        ] {
            assert!(no_questions(t), "{t}");
        }
        for t in ["None yet", "n//a", "-", "None.."] {
            assert!(!no_questions(t), "{t}");
        }
    }
}
