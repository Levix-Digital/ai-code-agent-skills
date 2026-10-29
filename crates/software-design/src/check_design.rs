//! `check_design.py`: validates the project's declared architecture and that the core does not
//! depend on names it must never import (`architecture.forbidden_core_imports` in
//! `.dev-cycle.json`). Exit code 1 on errors (and on warnings with `--strict`).

use std::path::Path;
use std::sync::LazyLock;

use architecture_docs::docs::py_regex;
use kernel::out::println;
use kernel::project_config::{ARCHITECTURE_STYLES, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::num::strip;
use kernel::py::path::{isdir, join};
use kernel::py::{Crash, Value, is_word, iterate, lower, universal_newlines};
use regex::Regex;

const SOURCE_EXTENSIONS: [&str; 19] = [
    ".rs", ".py", ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".go", ".c", ".h", ".cc", ".cpp",
    ".hpp", ".cs", ".java", ".kt", ".kts", ".swift",
];
/// Build output, dependencies and test folders (tests may import adapters on purpose).
const SKIPPED_DIRS: [&str; 14] = [
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    "tests",
    "test",
    "__tests__",
    "benches",
    "examples",
];

/// One pattern per kind of import statement; group 1 is the imported path or module. The
/// Python `from` pattern ends with `\b`, checked after the match (`from_import`).
static IMPORT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([A-Za-z_][\w:]*)",
        r"^\s*extern\s+crate\s+([A-Za-z_]\w*)",
        r"^\s*import\s+([\w.]+)",
        r"^\s*from\s+([\w.]+)\s+import",
        r#"^\s*import\s+.*?\s+from\s+['"]([^'"]+)['"]"#,
        r#"^\s*import\s+['"]([^'"]+)['"]"#,
        r#"require\(\s*['"]([^'"]+)['"]\s*\)"#,
        r#"^\s*["`]([^"`]+)["`]\s*$"#,
        r#"^\s*#\s*include\s*[<"]([^>"]+)[>"]"#,
        r"^\s*using\s+(?:static\s+)?([\w.]+)\s*;",
    ]
    .iter()
    .map(|p| py_regex(p))
    .collect()
});
const FROM_IMPORT: usize = 3;

/// `imported_names(line)`.
fn imported_names(line: &str) -> Vec<String> {
    let mut names = Vec::new();
    for (i, pattern) in IMPORT_PATTERNS.iter().enumerate() {
        if let Some(m) = pattern.captures(line) {
            let end = m.get(0).map_or(0, |g| g.end());
            if i == FROM_IMPORT && line[end..].chars().next().is_some_and(is_word) {
                continue;
            }
            names.push(m[1].to_string());
        }
    }
    names
}

/// `re.split(r"::|\.|/|\\", value.strip().replace("-", "_"))` without the empty parts.
fn parts(value: &str) -> Vec<String> {
    let normalized = strip(value).replace('-', "_");
    let mut out = Vec::new();
    let mut current = String::new();
    let mut rest = normalized.as_str();
    while let Some(c) = rest.chars().next() {
        if rest.starts_with("::") {
            out.push(std::mem::take(&mut current));
            rest = &rest[2..];
        } else if matches!(c, '.' | '/' | '\\') {
            out.push(std::mem::take(&mut current));
            rest = &rest[1..];
        } else {
            current.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out.push(current);
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// `matches_forbidden(name, forbidden)`: true when the import path starts with the forbidden
/// module (any separator).
fn matches_forbidden(name: &str, forbidden: &str) -> bool {
    let (normalized, target) = (parts(name), parts(forbidden));
    !target.is_empty()
        && normalized.len() >= target.len()
        && normalized[..target.len()] == target[..]
}

/// `bad.strip()` for an item of `forbidden_core_imports`.
fn as_text(bad: &Value) -> Result<&str, Crash> {
    match bad {
        Value::Str(s) => Ok(s),
        other => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'strip'",
            other.type_name()
        ))),
    }
}

/// `re.search(rf"(?<![\w:]){re.escape(crate)}::[\w:]*", code)`.
fn qualified_path(code: &str, krate: &str) -> Option<String> {
    let needle = format!("{krate}::");
    let mut from = 0;
    while let Some(off) = code[from..].find(&needle) {
        let i = from + off;
        let before = code[..i].chars().next_back();
        if !before.is_some_and(|c| is_word(c) || c == ':') {
            let tail = &code[i + needle.len()..];
            let len = tail
                .char_indices()
                .find(|(_, c)| !(is_word(*c) || *c == ':'))
                .map_or(tail.len(), |(j, _)| j);
            return Some(code[i..i + needle.len() + len].to_string());
        }
        from = i + code[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// `os.path.splitext(name)[1]`.
fn extension(name: &str) -> &str {
    let stem_start = name.len() - name.trim_start_matches('.').len();
    match name[stem_start..].rfind('.') {
        Some(i) => &name[stem_start + i..],
        None => "",
    }
}

/// The files `os.walk(top)` visits, in its order, with their path relative to `top`.
fn walk(top: &str, prefix: &str, out: &mut Vec<(String, String, String)>) {
    let Ok(entries) = std::fs::read_dir(top) else {
        return;
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = join(top, &name);
        if Path::new(&path).is_dir() {
            dirs.push(name);
        } else {
            out.push((path, format!("{prefix}{name}"), name));
        }
    }
    for name in dirs {
        if SKIPPED_DIRS.contains(&name.as_str()) {
            continue;
        }
        let path = join(top, &name);
        if Path::new(&path).is_symlink() {
            continue;
        }
        walk(&path, &format!("{prefix}{name}/"), out);
    }
}

/// `open(path, encoding="utf-8", errors="replace").readlines()`.
fn read_lines(path: &str) -> Option<Vec<String>> {
    let bytes = std::fs::read(path).ok()?;
    let text = universal_newlines(&String::from_utf8_lossy(&bytes));
    Some(text.split_inclusive('\n').map(str::to_string).collect())
}

fn is_comment(stripped: &str) -> bool {
    ["//", "#!", "*", "/*"]
        .iter()
        .any(|p| stripped.starts_with(p))
        || (stripped.starts_with('#')
            && !stripped.starts_with("#include")
            && !stripped.starts_with("# include"))
}

/// `scan_core(core_dir, forbidden)`: (relative path, line number, import, forbidden name) for
/// each violation.
fn scan_core(
    core_dir: &str,
    forbidden: &Value,
) -> Result<Vec<(String, usize, String, String)>, Crash> {
    let mut violations = Vec::new();
    let mut files = Vec::new();
    walk(core_dir, "", &mut files);
    for (path, rel, name) in files {
        if !SOURCE_EXTENSIONS.contains(&lower(extension(&name)).as_str()) {
            continue;
        }
        let Some(lines) = read_lines(&path) else {
            continue;
        };
        for (i, line) in lines.iter().enumerate() {
            let n = i + 1;
            if is_comment(strip(line)) {
                continue;
            }
            let mut found = false;
            for imp in imported_names(line) {
                for bad in iterate(forbidden)? {
                    let bad_text = as_text(&bad)?;
                    if matches_forbidden(&imp, bad_text) {
                        violations.push((rel.clone(), n, imp.clone(), bad_text.to_string()));
                        found = true;
                    }
                }
            }
            // Rust also reaches other crates through fully qualified paths without `use`.
            if !found && name.ends_with(".rs") {
                let code = line.split("//").next().unwrap_or_default();
                for bad in iterate(forbidden)? {
                    let bad_text = as_text(&bad)?;
                    let krate = strip(bad_text).replace('-', "_");
                    if let Some(m) = qualified_path(code, &krate) {
                        violations.push((rel.clone(), n, m, bad_text.to_string()));
                    }
                }
            }
        }
    }
    Ok(violations)
}

/// `run(project_dir)`: (errors, warnings).
fn run_checks(project_dir: Option<&str>) -> Result<(Vec<String>, Vec<String>), Crash> {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let cfg = match load_project_config(project_dir) {
        Ok(c) => c,
        Err(e) => return Err(e.into_crash()),
    };
    let arch = cfg.section("architecture");

    let style = arch.get_or_none("style");
    if !style.truthy() {
        warnings.push("architecture.style is not set in .dev-cycle.json; infer it, agree it with the user and record it in an ADR".to_string());
    } else if !style.is_one_of(&ARCHITECTURE_STYLES) {
        errors.push(format!(
            "architecture.style '{}' is unknown (use one of: {})",
            style.py_str(),
            ARCHITECTURE_STYLES.join(", ")
        ));
    }

    let core_rel = arch.get_or_none("core_path");
    let configured = arch.get_or_none("forbidden_core_imports");
    let forbidden = if configured.truthy() {
        configured.clone()
    } else {
        Value::List(Vec::new())
    };
    if !core_rel.truthy() {
        if forbidden.truthy() {
            errors.push(
                "architecture.forbidden_core_imports is set but architecture.core_path is not"
                    .to_string(),
            );
        } else {
            warnings.push(
                "architecture.core_path is not set; the core dependency check is skipped"
                    .to_string(),
            );
        }
        return Ok((errors, warnings));
    }

    let Value::Str(core_text) = core_rel else {
        return Err(Crash::new(format!(
            "TypeError: join() argument must be str, bytes, or os.PathLike object, not '{}'",
            core_rel.type_name()
        )));
    };
    let core_dir = join(&cfg.root.to_string_lossy(), core_text);
    if !isdir(&core_dir) {
        errors.push(format!(
            "architecture.core_path '{core_text}' does not exist"
        ));
        return Ok((errors, warnings));
    }

    if !forbidden.truthy() {
        warnings.push("architecture.forbidden_core_imports is empty; list the UI, transport and adapter modules the core must not import".to_string());
        return Ok((errors, warnings));
    }

    for (rel, line, imp, bad) in scan_core(&core_dir, &forbidden)? {
        errors.push(format!(
            "{core_text}/{rel}:{line}: core imports '{imp}' (forbidden: '{bad}')"
        ));
    }
    Ok((errors, warnings))
}

/// `python check_design.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_design.py",
        "Check the project's architecture style and core dependencies",
    )
    .option("--dir", "Project directory (default: current directory)")
    .flag("--strict", "Fail on warnings too");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    let (errors, warnings) = run_checks(args.value("dir"))?;
    for e in &errors {
        println(&format!("❌ {e}"));
    }
    for w in &warnings {
        println(&format!("⚠️ {w}"));
    }
    let failed = !errors.is_empty() || (args.flag("strict") && !warnings.is_empty());
    println(&format!(
        "\n{} software-design: {} error(s), {} warning(s).",
        if failed { "❌" } else { "✅" },
        errors.len(),
        warnings.len()
    ));
    Ok(u8::from(failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_names_per_language() {
        assert_eq!(
            imported_names("use ratatui::widgets::Block;"),
            ["ratatui::widgets::Block"]
        );
        assert_eq!(
            imported_names("pub(crate) use crate::store;"),
            ["crate::store"]
        );
        assert_eq!(imported_names("extern crate serde;"), ["serde"]);
        assert_eq!(imported_names("from flask import Flask"), ["flask"]);
        assert_eq!(imported_names("from flask importer"), Vec::<String>::new());
        assert_eq!(imported_names("import requests"), ["requests"]);
        assert!(imported_names("import React from 'react';").contains(&"react".to_string()));
        assert_eq!(imported_names("const x = require('express');"), ["express"]);
        assert_eq!(
            imported_names("#include <QtWidgets/QWidget>"),
            ["QtWidgets/QWidget"]
        );
        assert_eq!(
            imported_names("using System.Windows.Forms;"),
            ["System.Windows.Forms"]
        );
        assert_eq!(imported_names("\t\"fmt\"\n"), ["fmt"]);
    }

    #[test]
    fn matches_forbidden_by_module_prefix_only() {
        assert!(matches_forbidden("ratatui::widgets::Block", "ratatui"));
        assert!(matches_forbidden("actx_cli::tui", "actx-cli"));
        assert!(matches_forbidden("app.web.views", "app.web"));
        assert!(!matches_forbidden("apple::Tree", "app"));
        assert!(!matches_forbidden("crate::store", "ratatui"));
        assert!(!matches_forbidden("x", " :: "));
    }

    #[test]
    fn qualified_paths_and_extensions() {
        assert_eq!(
            qualified_path("let p = app_installer::paths::bin_dir();", "app_installer").as_deref(),
            Some("app_installer::paths::bin_dir")
        );
        assert_eq!(qualified_path("my_app_installer::x", "app_installer"), None);
        assert_eq!(qualified_path("a::app::b", "app"), None);
        assert_eq!(extension(".rs"), "");
        assert_eq!(extension("a.b.RS"), ".RS");
        assert_eq!(extension("..x.py"), ".py");
    }
}
