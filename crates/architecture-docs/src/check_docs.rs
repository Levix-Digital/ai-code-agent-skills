//! `check_docs.py`: validates the project's architecture documentation (index, subsystem pages
//! and ADRs). Errors (exit code 1): versions or manual numbering in headings, invalid ADR
//! frontmatter, file names and H1s, one-way ADR relations, broken links and anchors, an
//! outdated index, headings not in the configured language. Warnings: long files, body lines
//! that look like another language, ADRs missing a standard section, `code` paths that no
//! longer exist.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::num::{self, is_space};
use kernel::py::path::{basename, dirname, exists, isdir, isfile, join, normpath};
use kernel::py::{Crash, Value, read_text};
use regex::Regex;

use crate::build_index::build_index;
use crate::docs::{
    ADR_SECTIONS, ADR_STATUSES, Doc, INLINE_CODE, LINK, Project, RELATIONS, SCHEME, adr_label,
    anchors, as_id_list, fenced_lines, foreign_language_hits, get, has_version, headings, py_regex,
    read_doc, strip_code, zero_pad4,
};

struct Report<'a> {
    project: &'a Project,
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report<'_> {
    fn error(&mut self, path: &str, line: usize, msg: &str) -> Result<(), Crash> {
        let rel = self.project.rel(path)?;
        self.errors.push(format!("{rel}:{line}: {msg}"));
        Ok(())
    }

    fn warn(&mut self, path: &str, line: usize, msg: &str) -> Result<(), Crash> {
        let rel = self.project.rel(path)?;
        self.warnings.push(format!("{rel}:{line}: {msg}"));
        Ok(())
    }
}

static NUMBERED: LazyLock<Regex> = LazyLock::new(|| py_regex(r"^\d+(\.\d+)*[.)]?\s"));
/// `body_offset(path)`: the line where the body starts (after the frontmatter), so messages
/// point at real lines.
fn body_offset(path: &str) -> Result<usize, Crash> {
    let text = read_text(std::path::Path::new(path))?;
    if text.starts_with("---\n")
        && let Some(i) = text[4..].find("\n---\n")
    {
        let end = 4 + i;
        let rest = &text[end + 5..];
        return Ok(text[..end + 5].matches('\n').count()
            + (rest.len() - rest.trim_start_matches('\n').len()));
    }
    Ok(0)
}

fn check_headings(
    report: &mut Report,
    path: &str,
    body: &str,
    offset: usize,
    structural: bool,
    language: &Value,
) -> Result<(), Crash> {
    for (n, level, title) in headings(body) {
        let line = n + offset;
        if structural && level > 1 && has_version(&title) {
            report.error(
                path,
                line,
                &format!(
                    "version number in heading '{title}': describe the current state; versions belong in ADRs and releases"
                ),
            )?;
        }
        if structural && level > 1 && NUMBERED.is_match(&title) {
            report.error(
                path,
                line,
                &format!("manually numbered heading '{title}': remove the number"),
            )?;
        }
        let hits = foreign_language_hits(&title, language);
        if !hits.is_empty() && !title.contains("lang-ok") {
            report.error(
                path,
                line,
                &format!(
                    "heading not in '{}': {}",
                    language.py_str(),
                    hits.join(", ")
                ),
            )?;
        }
    }
    Ok(())
}

fn check_language_body(
    report: &mut Report,
    path: &str,
    body: &str,
    offset: usize,
    language: &Value,
) -> Result<(), Crash> {
    let raw: Vec<&str> = body.split('\n').collect();
    for (i, line) in strip_code(body).split('\n').enumerate() {
        if line.trim_start_matches(is_space).starts_with('#') || raw[i].contains("lang-ok") {
            continue;
        }
        let hits = foreign_language_hits(line, language);
        if !hits.is_empty() {
            report.warn(
                path,
                i + 1 + offset,
                &format!(
                    "text may not be in '{}': {}",
                    language.py_str(),
                    hits.join(", ")
                ),
            )?;
        }
    }
    Ok(())
}

fn check_links(
    report: &mut Report,
    path: &str,
    body: &str,
    offset: usize,
    anchor_cache: &mut HashMap<String, HashSet<String>>,
) -> Result<(), Crash> {
    let lines: Vec<&str> = body.split('\n').collect();
    for (i, (line, fenced)) in lines.iter().zip(fenced_lines(&lines)).enumerate() {
        if fenced {
            continue;
        }
        let n = i + 1;
        let plain = INLINE_CODE.replace_all(line, "");
        for m in LINK.captures_iter(&plain) {
            let target = &m[1];
            if SCHEME.is_match(target) || target.starts_with("mailto:") {
                continue;
            }
            let (file_part, anchor) = target.split_once('#').unwrap_or((target, ""));
            let dest = if file_part.is_empty() {
                path.to_string()
            } else {
                normpath(&join(&dirname(path), file_part))
            };
            if !file_part.is_empty() && !exists(&dest) {
                report.error(path, n + offset, &format!("broken link: {target}"))?;
                continue;
            }
            if !anchor.is_empty() && dest.ends_with(".md") && isfile(&dest) {
                if !anchor_cache.contains_key(&dest) {
                    let found = anchors(&read_doc(&dest)?.body);
                    anchor_cache.insert(dest.clone(), found);
                }
                if !anchor_cache[&dest].contains(anchor) {
                    report.error(path, n + offset, &format!("broken anchor: {target}"))?;
                }
            }
        }
    }
    Ok(())
}

/// `count > max_lines` for the configured limit.
fn over_limit(count: usize, max_lines: &Value) -> Result<bool, Crash> {
    match max_lines {
        Value::Int(i) => Ok(num::int(i, 10)?
            .add(&num::BigInt::from_i64(-(count as i64)))
            .to_string()
            .starts_with('-')),
        Value::Bool(b) => Ok(count > usize::from(*b)),
        Value::Float(f) => Ok((count as f64) > *f),
        other => Err(Crash::new(format!(
            "TypeError: '>' not supported between instances of 'int' and '{}'",
            other.type_name()
        ))),
    }
}

fn check_size(report: &mut Report, path: &str, max_lines: &Value) -> Result<(), Crash> {
    let text = read_text(std::path::Path::new(path))?;
    let count = text.matches('\n').count() + usize::from(!text.is_empty() && !text.ends_with('\n'));
    if over_limit(count, max_lines)? {
        report.warn(
            path,
            1,
            &format!(
                "{count} lines (limit {}): split it into smaller subsystem pages",
                max_lines.py_str()
            ),
        )?;
    }
    Ok(())
}

/// The items of `for x in value`.
fn iterate(value: &Value) -> Result<Vec<Value>, Crash> {
    match value {
        Value::List(items) | Value::Tuple(items) | Value::Set(items) => Ok(items.clone()),
        Value::Dict(d) => Ok(d.keys().cloned().collect()),
        Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Bytes(b) => Ok(b.iter().map(|x| Value::int(i64::from(*x))).collect()),
        other => Err(Crash::new(format!(
            "TypeError: '{}' object is not iterable",
            other.type_name()
        ))),
    }
}

/// `os.path.join(root, code)` for a value from a page's frontmatter.
fn join_code(root: &str, code: &Value) -> Result<String, Crash> {
    match code {
        Value::Str(s) => Ok(join(root, s)),
        Value::Bytes(_) => Err(Crash::new(
            "TypeError: Can't mix strings and bytes in path components",
        )),
        other => Err(Crash::new(format!(
            "TypeError: join() argument must be str, bytes, or os.PathLike object, not '{}'",
            other.type_name()
        ))),
    }
}

/// `m.get(key, default)` on a dict.
fn get_or<'a>(meta: &'a Value, key: &str, default: &'a Value) -> Result<&'a Value, Crash> {
    match meta {
        Value::Dict(d) => Ok(d.get(key).unwrap_or(default)),
        other => Err(crate::docs::no_attribute(other, "get")),
    }
}

/// An ADR id that `isinstance(id, int)` accepts, as an integer's decimal text.
fn int_id(value: &Value) -> Option<String> {
    match value {
        Value::Int(i) => Some(i.clone()),
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.into()),
        _ => None,
    }
}

fn check_adrs(report: &mut Report, project: &Project) -> Result<(), Crash> {
    let adrs = project.adrs()?;
    // `by_id`: (the id as first given, its integer text, the index of the last ADR with it).
    let mut by_id: Vec<(Value, String, usize)> = Vec::new();
    for (i, doc) in adrs.iter().enumerate() {
        let (m, path) = (&doc.meta, doc.path.as_str());
        let offset = body_offset(path)?;
        for field in ["id", "title", "status", "date"] {
            if !get(m, field)?.truthy() {
                report.error(path, 1, &format!("ADR frontmatter missing '{field}'"))?;
            }
        }
        let id_value = get(m, "id")?;
        let Some(adr_id) = int_id(id_value) else {
            report.error(path, 1, "ADR 'id' must be an integer")?;
            continue;
        };
        match by_id.iter_mut().find(|(_, text, _)| *text == adr_id) {
            Some(entry) => {
                let other = project.rel(&adrs[entry.2].path)?;
                report.error(
                    path,
                    1,
                    &format!("duplicate ADR id {} (also {other})", id_value.py_str()),
                )?;
                entry.2 = i;
            }
            None => by_id.push((id_value.clone(), adr_id.clone(), i)),
        }
        let pad = zero_pad4(&adr_id);
        if !basename(std::path::Path::new(path)).starts_with(&format!("{pad}-")) {
            report.error(path, 1, &format!("file name must start with {pad}-"))?;
        }
        let status = get(m, "status")?;
        if !ADR_STATUSES.iter().any(|s| status.as_str() == Some(s)) {
            report.error(
                path,
                1,
                &format!(
                    "invalid status '{}' (use {})",
                    status.py_str(),
                    ADR_STATUSES.join(", ")
                ),
            )?;
        }
        let expected = format!("{}: {}", adr_label(&adr_id), get(m, "title")?.py_str());
        if doc.h1().as_deref() != Some(expected.as_str()) {
            report.error(path, offset + 1, &format!("H1 must be '# {expected}'"))?;
        }
        if has_version(&get_or(m, "title", &Value::str(""))?.py_str()) {
            report.warn(
                path,
                1,
                "version number in the ADR title: use the 'version' field",
            )?;
        }
        let sections: HashSet<String> = headings(&doc.body)
            .into_iter()
            .filter(|(_, level, _)| *level == 2)
            .map(|(_, _, t)| t)
            .collect();
        let missing: Vec<&str> = ADR_SECTIONS
            .iter()
            .copied()
            .filter(|s| !sections.contains(*s))
            .collect();
        if !missing.is_empty() {
            report.warn(
                path,
                offset + 1,
                &format!("missing standard sections: {}", missing.join(", ")),
            )?;
        }
    }

    let find = |target: &str| by_id.iter().find(|(_, text, _)| text == target);
    for (_, adr_id, i) in &by_id {
        let doc: &Doc = &adrs[*i];
        let m = &doc.meta;
        for (forward, back) in RELATIONS {
            for (field, other_field) in [(forward, back), (back, forward)] {
                for target in as_id_list(get(m, field)?)? {
                    match find(&target) {
                        None => report.error(
                            &doc.path,
                            1,
                            &format!("'{field}' references missing {}", adr_label(&target)),
                        )?,
                        Some((_, _, j)) => {
                            let back_ids = as_id_list(get(&adrs[*j].meta, other_field)?)?;
                            if !back_ids.contains(adr_id) {
                                report.error(
                                    &doc.path,
                                    1,
                                    &format!(
                                        "'{field}: {}' but {} has no '{other_field}: {}'",
                                        adr_label(&target),
                                        adr_label(&target),
                                        adr_label(adr_id)
                                    ),
                                )?;
                            }
                        }
                    }
                }
            }
        }
        let superseded_by = as_id_list(get(m, "superseded_by")?)?;
        let status = get(m, "status")?;
        let superseded = status.as_str() == Some("superseded");
        if superseded && superseded_by.is_empty() {
            report.error(&doc.path, 1, "status 'superseded' requires 'superseded_by'")?;
        }
        if !superseded_by.is_empty() && !superseded {
            report.error(
                &doc.path,
                1,
                &format!("has 'superseded_by' but status is '{}'", status.py_str()),
            )?;
        }
    }
    Ok(())
}

/// `run(project, include_plans)`.
fn run_checks<'a>(project: &'a Project, include_plans: bool) -> Result<Report<'a>, Crash> {
    let mut report = Report {
        project,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let cfg = &project.config;
    let language = cfg.get_or_none("language").clone();
    let mut scope: Vec<Value> = Vec::new();
    for item in iterate(cfg.get_or_none("language_scope"))? {
        if !item.is_hashable() {
            return Err(Crash::new(format!(
                "TypeError: unhashable type: '{}'",
                item.type_name()
            )));
        }
        scope.push(item);
    }
    if include_plans {
        scope.push(Value::str("plans"));
    }
    let in_scope = |kind: &str| scope.iter().any(|s| s.as_str() == Some(kind));
    let mut anchor_cache: HashMap<String, HashSet<String>> = HashMap::new();

    let mut files: Vec<(String, &str)> = Vec::new();
    if isfile(&project.index_path) {
        files.push((project.index_path.clone(), "index"));
    } else {
        report.error(&project.index_path, 1, "index missing: run build_index.py")?;
    }
    for d in project.architecture_docs()? {
        files.push((d.path, "architecture"));
    }
    for d in project.adrs()? {
        files.push((d.path, "adr"));
    }
    if in_scope("plans") && isdir(&project.plans_dir) {
        for n in crate::docs::listdir(&project.plans_dir)? {
            if n.ends_with(".md") {
                files.push((join(&project.plans_dir, &n), "plans"));
            }
        }
    }

    for (path, kind) in &files {
        let doc = read_doc(path)?;
        let offset = body_offset(path)?;
        let language_here = if in_scope(kind) {
            language.clone()
        } else {
            Value::Null
        };
        check_headings(
            &mut report,
            path,
            &doc.body,
            offset,
            matches!(*kind, "index" | "architecture"),
            &language_here,
        )?;
        if in_scope(kind) {
            check_language_body(&mut report, path, &doc.body, offset, &language)?;
        }
        if *kind != "plans" {
            check_links(&mut report, path, &doc.body, offset, &mut anchor_cache)?;
            check_size(&mut report, path, cfg.get_or_none("max_lines"))?;
        }
        if *kind == "architecture" {
            let code = get(&doc.meta, "code")?;
            let codes = if code.truthy() {
                iterate(code)?
            } else {
                Vec::new()
            };
            for c in codes {
                if !exists(&join_code(&project.root, &c)?) {
                    report.warn(
                        path,
                        1,
                        &format!("documented source path no longer exists: {}", c.py_str()),
                    )?;
                }
            }
        }
    }

    check_adrs(&mut report, project)?;
    if isfile(&project.index_path) && !build_index(project, true)? {
        report.error(&project.index_path, 1, "index outdated: run build_index.py")?;
    }
    Ok(report)
}

/// `python check_docs.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

/// `warnings[:n]` for any integer `n`.
fn first(len: usize, n: &str) -> usize {
    let negative = n.starts_with('-');
    let digits = n.trim_start_matches('-');
    let magnitude = digits.parse::<usize>().unwrap_or(usize::MAX);
    if negative {
        len.saturating_sub(magnitude)
    } else {
        magnitude.min(len)
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_docs.py",
        "Validate the project's architecture documentation",
    )
    .option(
        "--dir",
        "Project directory (default: current; the Git root is used)",
    )
    .flag("--include-plans", "Also check the language of docs/plans")
    .int_option("--max-warnings", "Warnings to print (default 40; 0 = all)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let max_warnings = match args.value("max_warnings") {
        Some(v) => num::int(v, 10)?.to_string(),
        None => "40".to_string(),
    };

    let project = Project::new(args.value("dir"))?;
    if !project.adopted() {
        println(&format!(
            "ℹ️ {} does not use the architecture-docs layout yet ({}, {}). Nothing to check.",
            project.name.py_str(),
            project.config.get_or_none("architecture_dir").py_str(),
            project.config.get_or_none("adr_dir").py_str()
        ));
        return Ok(0);
    }

    let report = run_checks(&project, args.flag("include_plans"))?;
    for e in &report.errors {
        println(&format!("❌ {e}"));
    }
    let shown = if max_warnings == "0" {
        report.warnings.len()
    } else {
        first(report.warnings.len(), &max_warnings)
    };
    for w in &report.warnings[..shown] {
        println(&format!("⚠️ {w}"));
    }
    if shown < report.warnings.len() {
        println(&format!(
            "⚠️ ... {} more warning(s) (--max-warnings 0 to show all)",
            report.warnings.len() - shown
        ));
    }
    println(&format!(
        "\n{} architecture-docs: {} error(s), {} warning(s).",
        if report.errors.is_empty() {
            "✅"
        } else {
            "❌"
        },
        report.errors.len(),
        report.warnings.len()
    ));
    Ok(if report.errors.is_empty() { 0 } else { 1 })
}
