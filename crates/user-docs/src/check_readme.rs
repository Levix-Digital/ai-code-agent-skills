//! `check_readme.py`: validates that the project's README is end-user documentation: standard
//! sections, current state only (no changelog or version sections), no implementation detail,
//! English only, working relative links and a length limit. Exit code 1 on errors, or when
//! warnings exceed `--max-warnings`.

use std::cmp::Ordering;
use std::path::Path;

use architecture_docs::docs::{
    INLINE_CODE, LINK, SCHEME, anchors, cmp_int, fenced_lines, foreign_language_hits, has_version,
    headings, strip_code,
};
use kernel::out::println;
use kernel::project_config::load_project_config;
use kernel::py::argparse::Parser;
use kernel::py::num::{self, decimal_value, is_space, strip};
use kernel::py::path::{basename, dirname, exists, isabs, isfile, join, normpath};
use kernel::py::unicode::ignorecase_eq;
use kernel::py::{Crash, Value, is_word, lower, read_text};

const DEFAULT_MAX_LINES: &str = "400";
const EXEMPT_MARK: &str = "readme-ok";

/// Section name -> words that identify its heading (lower case).
const REQUIRED_SECTIONS: [(&str, &[&str]); 3] = [
    (
        "Installation",
        &["install", "installation", "setup", "getting started"],
    ),
    (
        "Quick Start or Usage",
        &[
            "quick start",
            "quickstart",
            "usage",
            "getting started",
            "how to use",
            "using",
        ],
    ),
    ("License", &["license", "licence"]),
];
const RECOMMENDED_SECTIONS: [(&str, &[&str]); 5] = [
    ("Features", &["features", "what it does", "highlights"]),
    ("Requirements", &["requirements", "prerequisites"]),
    ("Configuration", &["configuration", "settings", "config"]),
    (
        "Troubleshooting",
        &["troubleshooting", "faq", "common problems", "known issues"],
    ),
    (
        "Documentation",
        &["documentation", "docs", "learn more", "further reading"],
    ),
];
const SOURCE_EXTENSIONS: [&str; 18] = [
    "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "kt", "cs", "cpp", "cc", "c", "h", "hpp",
    "swift", "rb", "php",
];

/// `\b` before character `i`, with Python's word characters.
fn boundary(chars: &[char], i: usize) -> bool {
    let before = i > 0 && is_word(chars[i - 1]);
    let after = chars.get(i).is_some_and(|c| is_word(*c));
    before != after
}

fn digit_at(chars: &[char], i: usize) -> bool {
    chars.get(i).is_some_and(|c| decimal_value(*c).is_some())
}

/// `DATE_RE.search(text)`: `\b(19|20)\d{2}-\d{2}(-\d{2})?\b`.
fn has_date(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    (0..c.len()).any(|i| {
        let century = matches!(
            (c.get(i), c.get(i + 1)),
            (Some('1'), Some('9')) | (Some('2'), Some('0'))
        );
        if !boundary(&c, i) || !century {
            return false;
        }
        let month = i + 4;
        if !(digit_at(&c, i + 2) && digit_at(&c, i + 3) && c.get(month) == Some(&'-')) {
            return false;
        }
        if !(digit_at(&c, month + 1) && digit_at(&c, month + 2)) {
            return false;
        }
        let day = month + 3;
        let with_day = c.get(day) == Some(&'-')
            && digit_at(&c, day + 1)
            && digit_at(&c, day + 2)
            && boundary(&c, day + 3);
        with_day || boundary(&c, day)
    })
}

#[derive(Clone, Copy)]
enum Tok {
    Lit(&'static str),
    OptQuote,
    Spaces,
    OptSpaces,
}

/// The alternatives of `CHANGELOG_HEADING_RE`.
const CHANGELOG: [&[Tok]; 7] = [
    &[
        Tok::Lit("what"),
        Tok::OptQuote,
        Tok::Lit("s"),
        Tok::Spaces,
        Tok::Lit("new"),
    ],
    &[Tok::Lit("change"), Tok::OptSpaces, Tok::Lit("log")],
    &[Tok::Lit("changelog")],
    &[Tok::Lit("release"), Tok::Spaces, Tok::Lit("notes")],
    &[Tok::Lit("recent"), Tok::Spaces, Tok::Lit("changes")],
    &[Tok::Lit("latest"), Tok::Spaces, Tok::Lit("changes")],
    &[Tok::Lit("version"), Tok::Spaces, Tok::Lit("history")],
];

/// The end of `tokens` matched case-insensitively from `i`, when they match.
fn match_tokens(c: &[char], mut i: usize, tokens: &[Tok]) -> Option<usize> {
    for tok in tokens {
        match tok {
            Tok::Lit(word) => {
                for p in word.chars() {
                    if !c.get(i).is_some_and(|ch| ignorecase_eq(*ch, p)) {
                        return None;
                    }
                    i += 1;
                }
            }
            Tok::OptQuote => {
                if c.get(i) == Some(&'\'') {
                    i += 1;
                }
            }
            Tok::Spaces | Tok::OptSpaces => {
                let start = i;
                while c.get(i).is_some_and(|ch| is_space(*ch)) {
                    i += 1;
                }
                if matches!(tok, Tok::Spaces) && i == start {
                    return None;
                }
            }
        }
    }
    Some(i)
}

/// `CHANGELOG_HEADING_RE.search(title)`: `\b(what'?s\s+new|change\s*log|changelog|release\s+notes|
/// recent\s+changes|latest\s+changes|version\s+history)\b`, ignoring case.
fn is_changelog(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    (0..c.len()).any(|i| {
        boundary(&c, i)
            && CHANGELOG
                .iter()
                .any(|alt| match_tokens(&c, i, alt).is_some_and(|end| boundary(&c, end)))
    })
}

/// `ADR_RE.search(line)`: `\bADR[- ]?\d+\b`.
fn has_adr(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    (0..c.len()).any(|i| {
        if !boundary(&c, i) || c[i..].iter().take(3).collect::<String>() != "ADR" {
            return false;
        }
        let mut j = i + 3;
        if matches!(c.get(j), Some('-' | ' ')) {
            j += 1;
        }
        let start = j;
        while digit_at(&c, j) {
            j += 1;
        }
        j > start && boundary(&c, j)
    })
}

/// `SOURCE_EXT_RE.search(text)`: `\.(rs|py|...)\b`.
fn has_source_ext(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    (0..c.len()).any(|i| {
        c[i] == '.'
            && SOURCE_EXTENSIONS.iter().any(|ext| {
                let n = ext.chars().count();
                c.len() > i + n
                    && c[i + 1..=i + n].iter().copied().eq(ext.chars())
                    && boundary(&c, i + 1 + n)
            })
    })
}

fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

struct CheckResult {
    name: String,
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl CheckResult {
    fn new(path: &str) -> Self {
        CheckResult {
            name: basename(Path::new(path)),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn error(&mut self, line: usize, msg: &str) {
        self.errors.push(format!("{}:{line}: {msg}", self.name));
    }

    fn warn(&mut self, line: usize, msg: &str) {
        self.warnings.push(format!("{}:{line}: {msg}", self.name));
    }
}

fn has_section(titles: &[String], words: &[&str]) -> bool {
    titles.iter().any(|t| {
        let t = lower(t);
        words.iter().any(|w| t.contains(w))
    })
}

fn check_text(path: &str, text: &str, max_lines: &str) -> CheckResult {
    let mut res = CheckResult::new(path);
    let lines: Vec<&str> = text.split('\n').collect();
    let english = Value::str("en");

    // Title and description
    let hs = headings(text);
    let h1: Vec<&(usize, usize, String)> = hs.iter().filter(|h| h.1 == 1).collect();
    if let Some(first) = h1.first() {
        if let Some(second) = h1.get(1) {
            res.warn(second.0, "more than one H1 heading; keep a single title");
        }
        let start = first.0;
        let mut following = 0;
        for line in lines.iter().skip(start) {
            if line.starts_with('#') {
                break;
            }
            let s = strip(line);
            if !s.is_empty()
                && !["[![", "![", "<", "---", "|", "```"]
                    .iter()
                    .any(|p| s.starts_with(p))
            {
                following += 1;
            }
        }
        if following == 0 {
            res.error(
                start,
                "missing description: add one sentence on what the project is and for whom right after the title",
            );
        }
    } else {
        res.error(1, "missing title (a single '# Name' heading)");
    }

    // Headings: changelog style, versions, dates, language
    let titles: Vec<String> = hs
        .iter()
        .filter(|h| h.1 >= 2)
        .map(|h| h.2.clone())
        .collect();
    for (n, level, title) in &hs {
        if lines[n - 1].contains(EXEMPT_MARK) {
            continue;
        }
        if has_version(title) || has_date(title) {
            res.error(
                *n,
                &format!(
                    "version or date in heading '{title}': describe the current state; history goes to the release notes"
                ),
            );
        } else if *level >= 2 && is_changelog(title) {
            res.error(
                *n,
                &format!(
                    "changelog-style heading '{title}': history goes to the release notes, not the README"
                ),
            );
        }
        let hits = foreign_language_hits(title, &english);
        if !hits.is_empty() {
            res.error(*n, &format!("heading not in English: {}", hits.join(", ")));
        }
    }

    // Sections
    for (name, words) in REQUIRED_SECTIONS {
        if !has_section(&titles, words) {
            res.error(1, &format!("missing required section: {name}"));
        }
    }
    for (name, words) in RECOMMENDED_SECTIONS {
        if !has_section(&titles, words) {
            res.warn(1, &format!("missing recommended section: {name}"));
        }
    }

    // Body checks (outside code blocks and inline code)
    let stripped = strip_code(text);
    let stripped: Vec<&str> = stripped.split('\n').collect();
    let fenced = fenced_lines(&lines);
    for (i, ((line, plain), in_code)) in lines.iter().zip(&stripped).zip(&fenced).enumerate() {
        let n = i + 1;
        if line.contains(EXEMPT_MARK) {
            continue;
        }
        if lstrip(line).starts_with("```mermaid") {
            res.warn(
                n,
                "diagram in the README: architecture diagrams belong to the technical documentation",
            );
        }
        if *in_code || lstrip(plain).starts_with('#') {
            continue;
        }
        if has_version(plain) {
            res.warn(
                n,
                "version reference in body text: describe the current behavior (mark required versions with <!-- readme-ok -->)",
            );
        }
        if has_adr(line) {
            res.warn(
                n,
                "ADR reference: link to the technical documentation index instead",
            );
        }
        let no_code = INLINE_CODE.replace_all(line, "");
        for m in LINK.captures_iter(&no_code) {
            let target = &m[1];
            let file = target.split('#').next().unwrap_or_default();
            if has_source_ext(file) && !SCHEME.is_match(target) {
                res.warn(
                    n,
                    &format!(
                        "link to a source file ({target}): implementation detail does not belong in the README"
                    ),
                );
            }
        }
        let hits = foreign_language_hits(plain, &english);
        if !hits.is_empty() {
            res.warn(
                n,
                &format!("text may not be in English: {}", hits.join(", ")),
            );
        }
    }

    // Relative links and anchors
    let own_anchors = anchors(text);
    for (i, (line, in_code)) in lines.iter().zip(&fenced).enumerate() {
        let n = i + 1;
        if *in_code {
            continue;
        }
        let no_code = INLINE_CODE.replace_all(line, "");
        for m in LINK.captures_iter(&no_code) {
            let target = &m[1];
            if SCHEME.is_match(target) || target.starts_with("mailto:") {
                continue;
            }
            let (file_part, anchor) = target.split_once('#').unwrap_or((target, ""));
            if file_part.is_empty() {
                if !anchor.is_empty() && !own_anchors.contains(anchor) {
                    res.error(n, &format!("broken anchor: #{anchor}"));
                }
                continue;
            }
            let dest = normpath(&join(&dirname(path), file_part));
            if !exists(&dest) {
                res.error(n, &format!("broken link: {target}"));
            }
        }
    }

    let count = lines.len().to_string();
    if cmp_int(&count, max_lines) == Ordering::Greater {
        res.warn(
            lines.len(),
            &format!(
                "{count} lines (limit {max_lines}): move long material to guides under docs/ and link them"
            ),
        );
    }
    res
}

fn run_check(project_dir: Option<&str>, file: Option<&str>) -> Result<CheckResult, Crash> {
    let cfg = match load_project_config(project_dir) {
        Ok(c) => c,
        Err(e) => return Err(e.into_crash()),
    };
    let docs = cfg.section("docs");
    let user_doc = docs.get_or_none("user_doc");
    let rel = match file.filter(|f| !f.is_empty()) {
        Some(f) => Value::str(f),
        None if user_doc.truthy() => user_doc.clone(),
        None => Value::str("README.md"),
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
    let configured = docs.get_or_none("readme_max_lines");
    let max_lines = if configured.truthy() {
        num::int_value(configured)?
    } else {
        DEFAULT_MAX_LINES.to_string()
    };
    if !isfile(&path) {
        let mut res = CheckResult::new(&path);
        res.error(1, &format!("README not found at {rel}"));
        return Ok(res);
    }
    let text = read_text(Path::new(&path))?;
    Ok(check_text(&path, &text, &max_lines))
}

/// `python check_readme.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_readme.py",
        "Validate the project's README as end-user documentation",
    )
    .option("--dir", "Project directory (default: current directory)")
    .option(
        "--file",
        "README path (default: docs.user_doc or README.md)",
    )
    .int_option(
        "--max-warnings",
        "Fail when there are more warnings than this",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let max_warnings = match args.value("max_warnings") {
        Some(v) => Some(num::int(v, 10)?.to_string()),
        None => None,
    };

    let res = run_check(args.value("dir"), args.value("file"))?;
    for e in &res.errors {
        println(&format!("❌ {e}"));
    }
    for w in &res.warnings {
        println(&format!("⚠️ {w}"));
    }
    let too_many = max_warnings
        .is_some_and(|m| cmp_int(&res.warnings.len().to_string(), &m) == Ordering::Greater);
    let ok = res.errors.is_empty() && !too_many;
    println(&format!(
        "\n{} user-docs: {} error(s), {} warning(s).",
        if ok { "✅" } else { "❌" },
        res.errors.len(),
        res.warnings.len()
    ));
    Ok(u8::from(!ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_like_python() {
        assert!(has_date("Released 2024-05"));
        assert!(has_date("on 1999-12-31."));
        assert!(!has_date("x2024-05"));
        assert!(!has_date("2024-051"));
        assert!(has_date("2024-05-1x"));
        assert!(!has_date("2124-05"));
    }

    #[test]
    fn changelog_headings_like_python() {
        for t in [
            "What's New",
            "whats  new",
            "Change log",
            "CHANGELOG",
            "Release Notes",
            "Version\thistory",
            "Releaſe notes",
        ] {
            assert!(is_changelog(t), "{t}");
        }
        for t in [
            "Updating",
            "Prompt history",
            "Changelogs",
            "news",
            "what s new",
        ] {
            assert!(!is_changelog(t), "{t}");
        }
    }

    #[test]
    fn adrs_and_source_files() {
        assert!(has_adr("See ADR-0130."));
        assert!(has_adr("ADR 7"));
        assert!(has_adr("ADR12"));
        assert!(!has_adr("xADR-1"));
        assert!(!has_adr("ADR-1a"));
        assert!(has_source_ext("src/engine.rs"));
        assert!(has_source_ext("a.tsx"));
        assert!(!has_source_ext("a.rsx"));
        assert!(!has_source_ext("README.md"));
    }
}
