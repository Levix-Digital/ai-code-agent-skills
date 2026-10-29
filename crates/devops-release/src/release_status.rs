//! `release_status.py`: version and release helpers: read the version from the project's version
//! files, compute the next release candidate tag, and verify a published release with the
//! GitHub CLI. The TOML version files are read as Python 3.13's `tomllib` reads them.

use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use kernel::out::println;
use kernel::project_config::{ProjectConfig, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::fnmatch::fnmatch_name;
use kernel::py::json::{self, LoadError};
use kernel::py::num::{self, BigInt, strip};
use kernel::py::path::{basename, isfile, join};
use kernel::py::unicode::{SPACE, digits, lower};
use kernel::py::{
    Crash, Dict, Value, iterate, read_text, tomllib, universal_newlines, utf8_error_text,
};
use regex::Regex;

static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| {
    let d = format!("[{}]", digits());
    Regex::new(&format!(r"^{d}+\.{d}+\.{d}+(?:-[0-9A-Za-z.-]+)?$")).expect("valid pattern")
});
static RC_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    let d = format!("[{}]", digits());
    Regex::new(&format!(r"^v({d}+\.{d}+\.{d}+)-rc\.({d}+)$")).expect("valid pattern")
});
static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"version[{SPACE}]*[=:][{SPACE}]*["']([^"']+)["']"#
    ))
    .expect("valid pattern")
});

/// `re.match(pattern, s)` for a pattern ending in `$`: also before a final newline.
fn full_match<'a>(re: &Regex, s: &'a str) -> Option<regex::Captures<'a>> {
    re.captures(s)
        .or_else(|| s.strip_suffix('\n').and_then(|t| re.captures(t)))
}

fn toml_version(text: &str, tables: &[&str]) -> Result<Option<String>, Crash> {
    let data = tomllib::loads(text)?;
    let empty = Value::Dict(Dict::new());
    for table in tables {
        // `node = node.get(part, {}) if isinstance(node, dict) else {}`
        let mut node = &data;
        for part in table.split('.') {
            node = node.as_dict().and_then(|d| d.get(part)).unwrap_or(&empty);
        }
        if let Some(Value::Str(v)) = node.as_dict().and_then(|d| d.get("version")) {
            return Ok(Some(v.clone()));
        }
    }
    Ok(None)
}

/// The version string in one file, or `None` when the file holds no version.
pub fn read_version_file(path: &str) -> Result<Option<String>, Crash> {
    let text = read_text(Path::new(path))?;
    let name = lower(&basename(Path::new(path)));
    match name.as_str() {
        "cargo.toml" => toml_version(&text, &["package", "workspace.package"]),
        "pyproject.toml" => toml_version(&text, &["project", "tool.poetry"]),
        "package.json" => {
            let data = match json::loads(&text) {
                Ok(v) => v,
                Err(LoadError::Decode(e)) => {
                    return Err(Crash::new(format!("json.decoder.JSONDecodeError: {e}")));
                }
                Err(LoadError::Crash(c)) => return Err(c),
            };
            let Value::Dict(d) = &data else {
                return Err(no_attribute(&data, "get"));
            };
            Ok(d.get("version").and_then(Value::as_str).map(str::to_string))
        }
        "version.txt" | "version" => {
            let value = strip(&text)
                .trim_start_matches('\u{feff}')
                .trim_start_matches(['v', 'V']);
            Ok((!value.is_empty()).then(|| value.to_string()))
        }
        _ => Ok(VERSION_RE.captures(&text).map(|m| m[1].to_string())),
    }
}

fn no_attribute(value: &Value, name: &str) -> Crash {
    Crash::new(format!(
        "AttributeError: '{}' object has no attribute '{name}'",
        value.tp_name()
    ))
}

/// `os.path.join(cfg.root, rel)`.
fn join_root(root: &str, rel: &Value) -> Result<String, Crash> {
    match rel {
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

/// (version, problems). Problems when files are missing, have no version or disagree.
pub fn project_version(cfg: &ProjectConfig) -> Result<(Option<String>, Vec<String>), Crash> {
    let files = cfg.section("devops").get_or_none("version_files");
    if !files.truthy() {
        return Ok((
            None,
            vec!["devops.version_files is empty in .dev-cycle.json".to_string()],
        ));
    }
    let root = cfg.root.to_string_lossy().into_owned();
    let mut found = Dict::new();
    let mut problems = Vec::new();
    for rel in iterate(files)? {
        let path = join_root(&root, &rel)?;
        let shown = rel.py_str();
        if !isfile(&path) {
            problems.push(format!("{shown}: file not found"));
            continue;
        }
        match read_version_file(&path)? {
            None => problems.push(format!("{shown}: no version found")),
            Some(v) if full_match(&SEMVER_RE, &v).is_none() => {
                problems.push(format!("{shown}: '{v}' is not a semantic version"));
            }
            Some(v) => found.insert_value(rel, Value::Str(v)),
        }
    }
    let mut distinct: Vec<&str> = found.iter().filter_map(|(_, v)| v.as_str()).collect();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() > 1 {
        let pairs: Vec<String> = found
            .iter()
            .map(|(k, v)| format!("{}={}", k.py_str(), v.py_str()))
            .collect();
        problems.push(format!("version files disagree: {}", pairs.join(", ")));
    }
    let version = (distinct.len() == 1).then(|| distinct[0].to_string());
    Ok((version, problems))
}

/// `str.splitlines()`.
fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let boundary = matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !boundary {
            continue;
        }
        out.push(&s[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|(_, n)| *n == '\n') {
            chars.next();
            end += 1;
        }
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// The exception `subprocess.run` raises when the program cannot be started.
fn spawn_error(e: &std::io::Error, program: &str) -> (bool, String) {
    let quoted = kernel::py::repr_str(program);
    match e.kind() {
        std::io::ErrorKind::NotFound if cfg!(windows) => (
            true,
            "[WinError 2] The system cannot find the file specified".to_string(),
        ),
        std::io::ErrorKind::NotFound => (
            true,
            format!("[Errno 2] No such file or directory: {quoted}"),
        ),
        std::io::ErrorKind::PermissionDenied => (
            false,
            format!("PermissionError: [Errno 13] Permission denied: {quoted}"),
        ),
        _ => (false, format!("OSError: {e}")),
    }
}

/// `subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")`: (code, stdout,
/// stderr), or `Err(Ok(message))` for `FileNotFoundError`.
type Run = Result<(i32, String, String), Result<String, Crash>>;

fn run_command(program: &str, args: &[String], cwd: Option<&str>) -> Run {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            let (not_found, msg) = spawn_error(&e, program);
            return Err(if not_found {
                Ok(msg)
            } else {
                Err(Crash::new(msg))
            });
        }
    };
    let decode = |bytes: Vec<u8>| -> Result<String, Crash> {
        String::from_utf8(bytes)
            .map(|s| universal_newlines(&s))
            .map_err(|e| {
                Crash::new(format!(
                    "UnicodeDecodeError: {}",
                    utf8_error_text(e.as_bytes(), &e.utf8_error())
                ))
            })
    };
    let stdout = decode(out.stdout).map_err(Err)?;
    let stderr = decode(out.stderr).map_err(Err)?;
    Ok((out.status.code().unwrap_or(-1), stdout, stderr))
}

fn git_tags(root: &str, pattern: &str) -> Result<Vec<String>, Crash> {
    let args = ["tag".to_string(), "-l".to_string(), pattern.to_string()];
    match run_command("git", &args, Some(root)) {
        Ok((0, stdout, _)) => Ok(splitlines(&stdout)
            .into_iter()
            .map(strip)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()),
        Ok(_) => Ok(Vec::new()),
        Err(Ok(msg)) => Err(Crash::new(format!("FileNotFoundError: {msg}"))),
        Err(Err(c)) => Err(c),
    }
}

/// (tag, problem). Problem when the final tag already exists.
pub fn next_rc_tag(
    version: &str,
    existing: &[String],
) -> Result<(Option<String>, Option<String>), Crash> {
    let final_tag = format!("v{version}");
    if existing.contains(&final_tag) {
        return Ok((
            None,
            Some(format!(
                "{final_tag} is already released; bump the version before cutting a candidate"
            )),
        ));
    }
    // The numbers have no sign or leading zeros: longer is larger, then by digits.
    let mut highest = BigInt::zero();
    for t in existing {
        if let Some(m) = full_match(&RC_TAG_RE, t)
            && &m[1] == version
        {
            let n = num::int(&m[2], 10)?;
            let (a, b) = (n.to_string(), highest.to_string());
            if (a.len(), &a) > (b.len(), &b) {
                highest = n;
            }
        }
    }
    let next = highest.add(&BigInt::from_i64(1));
    Ok((Some(format!("v{version}-rc.{next}")), None))
}

/// `fnmatch.fnmatch(name, pattern)` for any name.
fn any_asset_matches(names: &[Value], pattern: &str) -> Result<bool, Crash> {
    for n in names {
        if fnmatch_name(n, pattern)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Problems with a release as returned by `gh release view --json`.
pub fn evaluate_release(
    tag: &str,
    data: &Value,
    patterns: &[String],
) -> Result<Vec<String>, Crash> {
    let Value::Dict(d) = data else {
        return Err(no_attribute(data, "get"));
    };
    let mut problems = Vec::new();
    let is_rc = full_match(&RC_TAG_RE, tag).is_some() || tag.contains("-rc");
    if d.get_or_none("isDraft").truthy() {
        problems.push("the release is still a draft".to_string());
    }
    if d.get_or_none("isPrerelease").truthy() != is_rc {
        let expected = if is_rc {
            "a pre-release"
        } else {
            "a final release (not a pre-release)"
        };
        problems.push(format!("{tag} should be {expected}"));
    }
    let assets = d.get_or_none("assets");
    let mut names = Vec::new();
    if assets.truthy() {
        for a in iterate(assets)? {
            let Value::Dict(a) = &a else {
                return Err(no_attribute(&a, "get"));
            };
            names.push(a.get("name").cloned().unwrap_or(Value::str("")));
        }
    }
    if names.is_empty() {
        problems.push("the release has no assets".to_string());
    }
    for pattern in patterns {
        if !any_asset_matches(&names, pattern)? {
            problems.push(format!("no asset matches '{pattern}'"));
        }
    }
    Ok(problems)
}

/// `fetch_release`: `Err(Ok(message))` for the errors `main` reports.
fn fetch_release(tag: &str, repo: &Value) -> Result<Value, Result<String, Crash>> {
    let mut args: Vec<String> = [
        "release",
        "view",
        tag,
        "--json",
        "tagName,isPrerelease,isDraft,assets,url",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if repo.truthy() {
        match repo {
            Value::Str(r) => args.extend(["--repo".to_string(), r.clone()]),
            other => {
                return Err(Err(Crash::new(format!(
                    "TypeError: expected str, bytes or os.PathLike object, not {}",
                    other.tp_name()
                ))));
            }
        }
    }
    let (code, stdout, stderr) = run_command("gh", &args, None)?;
    if code != 0 {
        let msg = strip(&stderr);
        return Err(Ok(if msg.is_empty() {
            format!("gh release view {tag} failed")
        } else {
            msg.to_string()
        }));
    }
    match json::loads(&stdout) {
        Ok(v) => Ok(v),
        Err(LoadError::Decode(e)) => Err(Ok(e.to_string())),
        Err(LoadError::Crash(c)) => Err(Err(c)),
    }
}

/// `python release_status.py <command> [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "release_status.py",
        "Version, release candidate and published release helpers",
    )
    .positional_choice("command", &["version", "next-rc", "verify"], "")
    .option("--dir", "Project directory (default: current directory)")
    .option("--tag", "With verify: the release tag")
    .option(
        "--repo",
        "With verify: repository (default: devops.releases_repo, else the current one)",
    )
    .append_option(
        "--asset",
        "With verify: asset name pattern that must exist; repeatable",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let cfg = match load_project_config(args.value("dir")) {
        Ok(c) => c,
        Err(e) => return Err(e.into_crash()),
    };
    let command = args.value("command").unwrap_or_default();

    if command == "version" || command == "next-rc" {
        let (version, problems) = project_version(&cfg)?;
        for p in &problems {
            println(&format!("❌ {p}"));
        }
        let Some(version) = version else {
            return Ok(1);
        };
        if command == "version" {
            println(&version);
            return Ok(u8::from(!problems.is_empty()));
        }
        let tags = git_tags(&cfg.root.to_string_lossy(), "v*")?;
        let (tag, problem) = next_rc_tag(&version, &tags)?;
        if let Some(problem) = problem {
            println(&format!("❌ {problem}"));
            return Ok(1);
        }
        println(&tag.unwrap_or_default());
        return Ok(u8::from(!problems.is_empty()));
    }

    let tag = match args.value("tag") {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            println("❌ verify needs --tag");
            return Ok(1);
        }
    };
    let repo = match args.value("repo") {
        Some(r) if !r.is_empty() => Value::str(r),
        _ => cfg.section("devops").get_or_none("releases_repo").clone(),
    };
    let data = match fetch_release(&tag, &repo) {
        Ok(d) => d,
        Err(Ok(e)) => {
            let where_ = if repo.truthy() {
                format!(" in {}", repo.py_str())
            } else {
                String::new()
            };
            println(&format!("❌ Release {tag} not found{where_}: {e}"));
            return Ok(1);
        }
        Err(Err(c)) => return Err(c),
    };
    let problems = evaluate_release(&tag, &data, &args.list("asset"))?;
    for p in &problems {
        println(&format!("❌ {p}"));
    }
    let d = data.as_dict().expect("checked by evaluate_release");
    let assets = match d.get_or_none("assets") {
        v if !v.truthy() => 0,
        Value::List(items) => items.len(),
        other => iterate(other)?.len(),
    };
    let status = if problems.is_empty() {
        "🟢 verified"
    } else {
        "🚨 not ready"
    };
    let kind = if d.get_or_none("isPrerelease").truthy() {
        "pre-release"
    } else {
        "release"
    };
    let url = d.get("url").map_or(String::new(), Value::py_str);
    println(&format!(
        "{status}: {tag} ({kind}, {assets} asset(s)) {url}"
    ));
    Ok(u8::from(!problems.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_like_python() {
        assert_eq!(
            splitlines("a\r\nb\rc\n\nd\u{2028}e"),
            vec!["a", "b", "c", "", "d", "e"]
        );
        assert_eq!(splitlines("a\n"), vec!["a"]);
        assert!(full_match(&SEMVER_RE, "1.2.3-rc.1").is_some());
        assert!(full_match(&SEMVER_RE, "1.2.3\n").is_some());
        assert!(full_match(&SEMVER_RE, "1.2").is_none());
        assert!(full_match(&SEMVER_RE, "١.٢.٣").is_some());
        let tags: Vec<String> = ["v1.4.0-rc.1", "v1.4.0-rc.2", "v1.3.9-rc.7"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            next_rc_tag("1.4.0", &tags).unwrap().0.as_deref(),
            Some("v1.4.0-rc.3")
        );
    }
}
