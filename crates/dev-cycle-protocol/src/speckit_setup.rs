//! `speckit_setup.py`: installs, pins, checks and upgrades GitHub Spec Kit in a project, the
//! optional executor of Phases 1, 2 and 4 of the full path. The CLI is always called at the
//! exact version pinned in `.dev-cycle.json` (`speckit.version`) through uvx, never through a
//! global install.
//!
//! `check` runs the compatibility check: Spec Kit is initialized in a temporary repository and
//! every behavior the protocol depends on is verified. `init` without --version checks the
//! latest release: when it passes, the project pins it (and the verified version moves
//! forward); when it fails, the project pins the last verified version and an adaptation
//! document is drafted.

use std::path::{Path, PathBuf};

use kernel::out::{err, println};
use kernel::project_config::{ConfigError, MANIFEST_NAME, find_manifest, load_project_config};
use kernel::py::argparse::{Parsed, Parser};
use kernel::py::json::{self, dumps, dumps_ascii};
use kernel::py::num::{decimal_value, is_space, strip};
use kernel::py::path::{abspath, exists, isdir, isfile, join, makedirs};
use kernel::py::subprocess::{mkdtemp, run_replace, which};
use kernel::py::unicode::is_word;
use kernel::py::{
    Crash, Dict, Value, read_text, repr_str, splitlines, today, utf8_error_text, write_text,
    write_text_native,
};

const SPECKIT_REPO: &str = "https://github.com/github/spec-kit.git";
const PRESET_ID: &str = "dev-cycle";
const PRESET_MARKER: &str = "## Dev Cycle Protocol";
const SCRIPT_TYPES: [&str; 3] = ["sh", "ps", "py"];
/// Spec Kit commands the protocol calls in Phases 1, 2 and 4 (and Phase 0 for the constitution).
const REQUIRED_COMMANDS: [&str; 7] = [
    "specify",
    "clarify",
    "plan",
    "tasks",
    "analyze",
    "implement",
    "constitution",
];
const REQUIRED_TEMPLATES: [&str; 4] = [
    "spec-template.md",
    "plan-template.md",
    "tasks-template.md",
    "constitution-template.md",
];

/// `SetupError` (a setup step that cannot continue; printed after `❌`), or an exception the
/// script does not catch.
enum Error {
    Setup(String),
    Crash(Crash),
}

impl From<Crash> for Error {
    fn from(c: Crash) -> Self {
        Error::Crash(c)
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Self {
        Error::Crash(e.into_crash())
    }
}

type Res<T> = Result<T, Error>;

fn setup<T>(msg: String) -> Res<T> {
    Err(Error::Setup(msg))
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

/// `a or b`.
fn or(a: &Value, b: Value) -> Value {
    if a.truthy() { a.clone() } else { b }
}

/// `value.get(key)` on what may not be a dict.
fn attr_get<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Crash> {
    match value {
        Value::Dict(d) => Ok(d.get_or_none(key)),
        other => Err(no_get(other)),
    }
}

fn no_get(value: &Value) -> Crash {
    Crash::new(format!(
        "AttributeError: '{}' object has no attribute 'get'",
        value.type_name()
    ))
}

/// The last `n` characters, as `text[-n:]`.
fn tail(text: &str, n: usize) -> &str {
    match text.char_indices().rev().nth(n - 1) {
        Some((i, _)) => &text[i..],
        None => text,
    }
}

/// Where the skills are: the preset, the verified version and the adaptation documents.
struct Skills {
    root: String,
}

impl Skills {
    fn here(&self) -> String {
        join(&self.root, "dev-cycle-protocol")
    }

    fn preset_dir(&self) -> String {
        join(&self.here(), "speckit-preset")
    }

    fn compat_file(&self) -> String {
        join(&self.preset_dir(), "compat.json")
    }

    fn adaptation_dir(&self) -> String {
        join(&join(&self.root, "docs"), "protocol-updates")
    }
}

// ---------------------------------------------------------------- versions

/// A version as `parse_version` returns it: each number as a key that orders like the numbers.
type Version = [(usize, String); 3];

fn number_key(digits: &str) -> (usize, String) {
    let ascii: String = digits
        .chars()
        .map(|c| char::from(b'0' + decimal_value(c).unwrap_or(0) as u8))
        .collect();
    let trimmed = ascii.trim_start_matches('0').to_string();
    (trimmed.len(), trimmed)
}

/// `VERSION_RE.match(tag)`: `^v(\d+)\.(\d+)\.(\d+)$`.
fn version_match(tag: &str) -> Option<Version> {
    let rest = tag.strip_prefix('v')?;
    let rest = rest.strip_suffix('\n').unwrap_or(rest);
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| decimal_value(c).is_some());
    if !parts.iter().all(|p| ok(p)) {
        return None;
    }
    Some([
        number_key(parts[0]),
        number_key(parts[1]),
        number_key(parts[2]),
    ])
}

fn parse_version(tag: &Value) -> Res<Version> {
    let text = match tag {
        Value::Str(t) => t.as_str(),
        other if other.truthy() => {
            return Err(Error::Crash(Crash::new(format!(
                "TypeError: expected string or bytes-like object, got '{}'",
                other.type_name()
            ))));
        }
        _ => "",
    };
    version_match(text).ok_or_else(|| {
        Error::Setup(format!(
            "'{}' is not a release tag like v1.2.3 (branches, hashes and 'latest' are not accepted)",
            tag.py_str()
        ))
    })
}

fn parse_str(tag: &str) -> Res<Version> {
    parse_version(&s(tag))
}

fn newest(tags: &[String]) -> Option<String> {
    let mut best: Option<(Version, &String)> = None;
    for t in tags {
        if let Some(v) = version_match(t)
            && best.as_ref().is_none_or(|(b, _)| v > *b)
        {
            best = Some((v, t));
        }
    }
    best.map(|(_, t)| t.clone())
}

fn latest_release() -> Res<String> {
    let (code, out) = run(
        &strs(&["git", "ls-remote", "--tags", "--refs", SPECKIT_REPO]),
        None,
    )?;
    if code != 0 {
        return setup(format!("could not list Spec Kit releases: {out}"));
    }
    let tags: Vec<String> = splitlines(&out)
        .into_iter()
        .filter(|l| l.contains("refs/tags/"))
        .map(|l| {
            let after = l.rsplit_once("refs/tags/").map_or(l, |(_, a)| a);
            strip(after).to_string()
        })
        .collect();
    match newest(&tags) {
        Some(t) => Ok(t),
        None => setup("no Spec Kit release tag found".into()),
    }
}

impl Skills {
    fn read_compat(&self) -> Value {
        match json::load_file(Path::new(&self.compat_file())) {
            Ok(v) => v,
            Err(_) => Value::Dict(Dict::new()),
        }
    }

    fn verified_version(&self) -> Result<Value, Crash> {
        Ok(attr_get(&self.read_compat(), "verified")?.clone())
    }

    /// Moves the verified version forward (never back). Returns true when it changed.
    fn record_verified(&self, version: &str, integration: &Value, script: &Value) -> Res<bool> {
        let current = self.verified_version()?;
        if current.truthy() && parse_version(&current)? >= parse_str(version)? {
            return Ok(false);
        }
        let mut checked = Dict::new();
        checked.insert("integration", integration.clone());
        checked.insert("script", script.clone());
        let mut data = Dict::new();
        data.insert("verified", s(version));
        data.insert("verified_at", s(&today()));
        data.insert("checked_with", Value::Dict(checked));
        let text = dumps_ascii(&Value::Dict(data), Some(2))? + "\n";
        write_text(Path::new(&self.compat_file()), &text)?;
        Ok(true)
    }
}

// ---------------------------------------------------------------- commands

fn default_script() -> &'static str {
    if cfg!(windows) { "ps" } else { "sh" }
}

fn strs(items: &[&str]) -> Vec<Value> {
    items.iter().map(|i| s(i)).collect()
}

fn specify_cmd(version: &str) -> Res<Vec<Value>> {
    parse_str(version)?;
    Ok(strs(&[
        "uvx",
        "--from",
        &format!("git+{SPECKIT_REPO}@{version}"),
        "specify",
    ]))
}

fn init_cmd(version: &str, integration: &Value, script: &Value) -> Res<Vec<Value>> {
    let mut cmd = specify_cmd(version)?;
    cmd.extend(strs(&["init", "--here", "--force", "--integration"]));
    cmd.push(integration.clone());
    cmd.push(s("--script"));
    cmd.push(script.clone());
    cmd.extend(strs(&[
        "--extension",
        "git",
        "--non-interactive",
        "--ignore-agent-tools",
    ]));
    Ok(cmd)
}

impl Skills {
    fn preset_add_cmd(&self, version: &str) -> Res<Vec<Value>> {
        let mut cmd = specify_cmd(version)?;
        cmd.extend(strs(&["preset", "add", "--dev", &self.preset_dir()]));
        Ok(cmd)
    }

    fn preset_update_cmd(&self, version: &str) -> Res<Vec<Value>> {
        let mut cmd = specify_cmd(version)?;
        cmd.extend(strs(&[
            "preset",
            "update",
            PRESET_ID,
            "--dev",
            &self.preset_dir(),
        ]));
        Ok(cmd)
    }

    fn upgrade_cmds(
        &self,
        version: &str,
        integration: &Value,
        script: &Value,
    ) -> Res<Vec<Vec<Value>>> {
        let mut first = specify_cmd(version)?;
        first.extend(strs(&["integration", "upgrade"]));
        first.push(integration.clone());
        first.push(s("--script"));
        first.push(script.clone());
        let mut second = specify_cmd(version)?;
        second.extend(strs(&["extension", "update"]));
        Ok(vec![first, second, self.preset_update_cmd(version)?])
    }
}

/// `sys.executable` for the `py` script type: the engine has no interpreter of its own, so
/// the one on PATH runs Spec Kit's Python scripts.
fn python() -> String {
    which("python3")
        .or_else(|| which("python"))
        .unwrap_or_else(|| "python3".to_string())
}

fn branch_script_cmd(script: &str, short_name: &str, description: &str) -> Vec<Value> {
    let base = join(&join(&join(".specify", "extensions"), "git"), "scripts");
    let script_path = |dir: &str, name: &str| join(&join(&base, dir), name);
    match script {
        "sh" => strs(&[
            "bash",
            &script_path("bash", "create-new-feature-branch.sh"),
            "--json",
            "--short-name",
            short_name,
            description,
        ]),
        "ps" => strs(&[
            "pwsh",
            "-NoProfile",
            "-File",
            &script_path("powershell", "create-new-feature-branch.ps1"),
            "-Json",
            "-ShortName",
            short_name,
            description,
        ]),
        _ => strs(&[
            &python(),
            &script_path("python", "create_new_feature_branch.py"),
            "--json",
            "--short-name",
            short_name,
            description,
        ]),
    }
}

fn setup_plan_cmd(script: &str) -> Vec<Value> {
    let base = join(".specify", "scripts");
    let script_path = |dir: &str, name: &str| join(&join(&base, dir), name);
    match script {
        "sh" => strs(&["bash", &script_path("bash", "setup-plan.sh"), "--json"]),
        "ps" => strs(&[
            "pwsh",
            "-NoProfile",
            "-File",
            &script_path("powershell", "setup-plan.ps1"),
            "-Json",
        ]),
        _ => strs(&[&python(), &script_path("python", "setup_plan.py"), "--json"]),
    }
}

/// `run(cmd, cwd)`: the exit code and the output (stdout then stderr, stripped); 127 when the
/// program is not found.
fn run(cmd: &[Value], cwd: Option<&str>) -> Result<(i32, String), Crash> {
    let args: Vec<String> = cmd
        .iter()
        .map(|v| match v {
            Value::Str(t) => Ok(t.clone()),
            other => Err(Crash::new(format!(
                "TypeError: expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ))),
        })
        .collect::<Result<_, _>>()?;
    match run_replace(&args, cwd) {
        Ok((code, stdout, stderr)) => Ok((code, strip(&(stdout + &stderr)).to_string())),
        Err(Ok(msg)) => Ok((127, format!("{}: not found ({msg})", args[0]))),
        Err(Err(c)) => Err(c),
    }
}

fn require_uv() -> Res<()> {
    if which("uvx").is_none() {
        return setup(
            "uv is required for Spec Kit (https://docs.astral.sh/uv/): \
             'pip install uv' or 'winget install astral-sh.uv' / 'curl -LsSf https://astral.sh/uv/install.sh | sh'"
                .into(),
        );
    }
    Ok(())
}

// ---------------------------------------------------------------- project files

/// `read_json(path)`: the file's object, or {} when it is missing, broken or not an object.
fn read_json(path: &str) -> Value {
    match json::load_file(Path::new(path)) {
        Ok(v @ Value::Dict(_)) => v,
        _ => Value::Dict(Dict::new()),
    }
}

fn specify(root: &str, name: &str) -> String {
    join(&join(root, ".specify"), name)
}

/// The Spec Kit version that initialized or last upgraded the project, as a tag (vX.Y.Z).
fn installed_version(root: &str) -> Value {
    for (name, key) in [
        ("integration.json", "version"),
        ("init-options.json", "speckit_version"),
    ] {
        let data = read_json(&specify(root, name));
        let value = attr_get(&data, key).cloned().unwrap_or(Value::Null);
        if value.truthy() {
            let text = value.py_str();
            return if text.starts_with('v') {
                value
            } else {
                Value::Str(format!("v{text}"))
            };
        }
    }
    Value::Null
}

fn git_config_path(root: &str) -> String {
    join(
        &join(&join(&join(root, ".specify"), "extensions"), "git"),
        "git-config.yml",
    )
}

/// The value of `branch_prefix:` in the git extension's settings, or None.
fn read_branch_prefix(root: &str) -> Result<Option<String>, Crash> {
    let Ok(bytes) = std::fs::read(git_config_path(root)) else {
        return Ok(None);
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => kernel::py::universal_newlines(&t),
        Err(e) => {
            return Err(Crash::new(format!(
                "UnicodeDecodeError: {}",
                utf8_error_text(e.as_bytes(), &e.utf8_error())
            )));
        }
    };
    for line in text.split_inclusive('\n') {
        // `^branch_prefix:\s*"?([^"#\n]*)"?`
        let Some(rest) = line.strip_prefix("branch_prefix:") else {
            continue;
        };
        let rest = rest.trim_start_matches(is_space);
        let rest = rest.strip_prefix('"').unwrap_or(rest);
        let end = rest.find(['"', '#', '\n']).unwrap_or(rest.len());
        return Ok(Some(strip(&rest[..end]).to_string()));
    }
    Ok(None)
}

/// A `re.error` raised while reading a replacement template.
fn pattern_error(template: &str, msg: &str, pos: usize) -> Crash {
    let mut msg = format!("{msg} at position {pos}");
    if template.contains('\n') {
        let before: Vec<char> = template.chars().take(pos).collect();
        let line = before.iter().filter(|c| **c == '\n').count() + 1;
        let col = before
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(pos + 1, |i| pos - i);
        msg = format!("{msg} (line {line}, column {col})");
    }
    Crash::new(format!("re.PatternError: {msg}"))
}

/// The replacement `re.sub` writes for `template` (a pattern without groups) in place of
/// `whole`, the text it matched.
fn expand_template(template: &str, whole: &str) -> Result<String, Crash> {
    let chars: Vec<char> = template.chars().collect();
    let octal = |c: Option<&char>| c.is_some_and(|c| ('0'..='7').contains(c));
    let ascii_digit = |c: Option<&char>| c.is_some_and(char::is_ascii_digit);
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\\' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        let Some(&c) = chars.get(i + 1) else {
            return Err(pattern_error(
                template,
                "bad escape (end of pattern)",
                start,
            ));
        };
        i += 2;
        match c {
            'g' => {
                if chars.get(i) != Some(&'<') {
                    return Err(pattern_error(template, "missing <", i));
                }
                let name_start = i + 1;
                let Some(len) = chars[name_start..].iter().position(|c| *c == '>') else {
                    let msg = if name_start == chars.len() {
                        "missing group name"
                    } else {
                        "missing >, unterminated name"
                    };
                    return Err(pattern_error(template, msg, name_start));
                };
                let name: String = chars[name_start..name_start + len].iter().collect();
                if name.is_empty() {
                    return Err(pattern_error(template, "missing group name", name_start));
                }
                if name.chars().all(|c| c.is_ascii_digit()) {
                    let n = name.trim_start_matches('0');
                    if n.is_empty() {
                        out.push_str(whole);
                        i = name_start + len + 1;
                        continue;
                    }
                    let msg = format!("invalid group reference {n}");
                    return Err(pattern_error(template, &msg, name_start));
                }
                let identifier = name.chars().all(is_word)
                    && !name
                        .chars()
                        .next()
                        .is_some_and(|c| decimal_value(c).is_some());
                if !identifier {
                    let msg = format!("bad character in group name {}", repr_str(&name));
                    return Err(pattern_error(template, &msg, name_start));
                }
                return Err(Crash::new(format!(
                    "IndexError: unknown group name {}",
                    repr_str(&name)
                )));
            }
            '0' => {
                let mut value = 0;
                for _ in 0..2 {
                    if octal(chars.get(i)) {
                        value = value * 8 + chars[i].to_digit(8).unwrap_or(0);
                        i += 1;
                    }
                }
                out.push(char::from_u32(value & 0xff).unwrap_or('\0'));
            }
            c if c.is_ascii_digit() => {
                let mut digits = c.to_string();
                if ascii_digit(chars.get(i)) {
                    digits.push(chars[i]);
                    i += 1;
                    if octal(Some(&c))
                        && octal(digits.chars().nth(1).as_ref())
                        && octal(chars.get(i))
                    {
                        digits.push(chars[i]);
                        i += 1;
                        let value = u32::from_str_radix(&digits, 8).unwrap_or(0);
                        if value > 0o377 {
                            let msg =
                                format!("octal escape value \\{digits} outside of range 0-0o377");
                            return Err(pattern_error(template, &msg, start));
                        }
                        out.push(char::from_u32(value).unwrap_or('\0'));
                        continue;
                    }
                }
                let n = digits.trim_start_matches('0');
                let msg = format!(
                    "invalid group reference {}",
                    if n.is_empty() { "0" } else { n }
                );
                return Err(pattern_error(template, &msg, start + 1));
            }
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'f' => out.push('\x0c'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\x0b'),
            '\\' => out.push('\\'),
            c if c.is_ascii_alphabetic() => {
                return Err(pattern_error(template, &format!("bad escape \\{c}"), start));
            }
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    Ok(out)
}

fn set_branch_prefix(root: &str, prefix: &Value) -> Result<(), Crash> {
    let path = git_config_path(root);
    let mut text = read_text(Path::new(&path))?;
    let line = format!("branch_prefix: \"{}\"", prefix.py_str());
    let found = if text.starts_with("branch_prefix:") {
        Some(0)
    } else {
        text.find("\nbranch_prefix:").map(|i| i + 1)
    };
    if let Some(start) = found {
        let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        let replacement = expand_template(&line, &text[start..end])?;
        text.replace_range(start..end, &replacement);
    } else {
        text = format!("{}\n\n{line}\n", text.trim_end_matches('\n'));
    }
    write_text(Path::new(&path), &text)
}

/// Updates the `speckit` section of the project's .dev-cycle.json (created when missing).
fn write_speckit_config(root: &str, values: &[(&str, Value)]) -> Result<String, Crash> {
    let path = find_manifest(Some(root))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| join(root, MANIFEST_NAME));
    let raw = if isfile(&path) {
        read_json(&path)
    } else {
        Value::Dict(Dict::new())
    };
    let Value::Dict(mut raw) = raw else {
        unreachable!("read_json returns an object")
    };
    let current = raw.get_or_none("speckit");
    let mut section = match current {
        Value::Dict(d) => d.clone(),
        other if !other.truthy() => Dict::new(),
        other => {
            return Err(Crash::new(format!(
                "TypeError: '{}' object is not a mapping",
                other.type_name()
            )));
        }
    };
    for (k, v) in values {
        section.insert(*k, v.clone());
    }
    raw.insert("speckit", Value::Dict(section));
    write_text(
        Path::new(&path),
        &(dumps(&Value::Dict(raw), Some(2))? + "\n"),
    )?;
    Ok(path)
}

fn preset_installed(root: &str) -> bool {
    isfile(&join(
        &join(&join(&join(root, ".specify"), "presets"), PRESET_ID),
        "preset.yml",
    ))
}

// ---------------------------------------------------------------- compatibility check

/// `speckit[.-](<command>)(?:[./]|SKILL|$)` searched in a relative path: the command.
fn command_in(rel: &str) -> Option<&'static str> {
    for (i, _) in rel.match_indices("speckit") {
        let after = &rel[i + "speckit".len()..];
        let Some(after) = after.strip_prefix(['.', '-']) else {
            continue;
        };
        for command in REQUIRED_COMMANDS {
            if let Some(rest) = after.strip_prefix(command)
                && (rest.starts_with(['.', '/'])
                    || rest.starts_with("SKILL")
                    || rest.is_empty()
                    || rest == "\n")
            {
                return Some(command);
            }
        }
    }
    None
}

/// The Spec Kit commands that have an agent file installed (skills or commands, any
/// integration).
fn find_command_files(root: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    let mut stack = vec![PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir && (name == ".git" || name == ".specify") {
                continue;
            }
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .map_or_else(|_| path.clone(), Path::to_path_buf)
                .to_string_lossy()
                .replace('\\', "/");
            if let Some(c) = command_in(&rel)
                && !found.contains(&c)
            {
                found.push(c);
            }
            if is_dir {
                stack.push(path);
            }
        }
    }
    found
}

struct Check {
    name: String,
    ok: bool,
    detail: String,
}

struct CheckResult {
    version: String,
    integration: Value,
    script: Value,
    checks: Vec<Check>,
    dir: Option<String>,
}

impl CheckResult {
    fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }

    fn to_value(&self) -> Value {
        let checks = self
            .checks
            .iter()
            .map(|c| {
                let mut d = Dict::new();
                d.insert("name", s(&c.name));
                d.insert("ok", Value::Bool(c.ok));
                d.insert("detail", s(&c.detail));
                Value::Dict(d)
            })
            .collect();
        let mut d = Dict::new();
        d.insert("version", s(&self.version));
        d.insert("integration", self.integration.clone());
        d.insert("script", self.script.clone());
        d.insert("passed", Value::Bool(self.passed()));
        d.insert("checks", Value::List(checks));
        d.insert("dir", self.dir.as_deref().map_or(Value::Null, s));
        Value::Dict(d)
    }
}

/// The value of `"BRANCH_NAME": "..."` in a script's JSON output.
fn branch_name(out: &str) -> Option<&str> {
    for (i, _) in out.match_indices("\"BRANCH_NAME\"") {
        let rest = out[i + "\"BRANCH_NAME\"".len()..].trim_start_matches(is_space);
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let rest = rest.trim_start_matches(is_space);
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"')
            && end > 0
        {
            return Some(&rest[..end]);
        }
    }
    None
}

/// `^feat/\d{3}-compat-check$`.
fn is_check_branch(branch: &str) -> bool {
    let Some(rest) = branch.strip_prefix("feat/") else {
        return false;
    };
    let mut chars = rest.chars();
    for _ in 0..3 {
        if !chars.next().is_some_and(|c| decimal_value(c).is_some()) {
            return false;
        }
    }
    matches!(chars.as_str(), "-compat-check" | "-compat-check\n")
}

/// Initializes Spec Kit `version` in a temporary repository and verifies every behavior the
/// protocol depends on.
fn compatibility_check(
    skills: &Skills,
    version: &str,
    integration: &Value,
    script: &Value,
    keep: bool,
) -> Res<CheckResult> {
    parse_str(version)?;
    let root = mkdtemp("speckit-check-")?;
    let mut checks: Vec<Check> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: String| {
        checks.push(Check {
            name: name.to_string(),
            ok,
            detail,
        });
        ok
    };
    let script_name = script.py_str();
    let r = Some(root.as_str());
    let git = |args: &[&str]| -> Vec<Value> {
        let mut cmd = strs(&[
            "git",
            "-c",
            "user.name=dev-cycle",
            "-c",
            "user.email=dev-cycle@example.invalid",
        ]);
        cmd.extend(strs(args));
        cmd
    };
    'checks: {
        let (mut code, mut out) = run(&strs(&["git", "init", "-q", "-b", "dev"]), r)?;
        if code == 0 {
            (code, out) = run(&git(&["commit", "-q", "--allow-empty", "-m", "init"]), r)?;
        }
        let detail = if code != 0 { out } else { String::new() };
        if !check("Temporary Git repository", code == 0, detail) {
            break 'checks;
        }

        let (code, out) = run(&init_cmd(version, integration, script)?, r)?;
        let name = format!("specify init ({}, {script_name})", integration.py_str());
        let detail = if code != 0 {
            tail(&out, 600).to_string()
        } else {
            String::new()
        };
        if !check(&name, code == 0, detail) {
            break 'checks;
        }

        let got = installed_version(&root);
        check(
            "Version recorded in .specify",
            got.py_eq(&s(version)),
            format!("found {}", got.py_str()),
        );

        let templates = join(&join(&root, ".specify"), "templates");
        let missing: Vec<&str> = REQUIRED_TEMPLATES
            .into_iter()
            .filter(|t| !isfile(&join(&templates, t)))
            .collect();
        let detail = if missing.is_empty() {
            String::new()
        } else {
            format!("missing: {}", missing.join(", "))
        };
        check("Core templates", missing.is_empty(), detail);

        let commands = find_command_files(&root);
        let missing: Vec<&str> = REQUIRED_COMMANDS
            .into_iter()
            .filter(|c| !commands.contains(c))
            .collect();
        let detail = if missing.is_empty() {
            String::new()
        } else {
            format!("missing: {}", missing.join(", "))
        };
        check("Agent commands", missing.is_empty(), detail);

        let has_prefix = read_branch_prefix(&root)?.is_some();
        let detail = format!("{} has no branch_prefix", git_config_path(&root));
        if check("Git extension branch_prefix setting", has_prefix, detail) {
            set_branch_prefix(&root, &s("feat"))?;
            run(&git(&["add", "-A"]), r)?;
            run(&git(&["commit", "-q", "-m", "speckit"]), r)?;
            let (code, out) = run(
                &branch_script_cmd(&script_name, "compat-check", "Compatibility check"),
                r,
            )?;
            let branch = branch_name(&out);
            let ok = code == 0 && branch.is_some_and(is_check_branch);
            let shown = branch.map_or("None".to_string(), repr_str);
            let mut detail = format!("got {shown}");
            if code != 0 {
                detail.push_str(&format!(": {}", tail(&out, 300)));
            }
            check("Branch created as feat/NNN-slug", ok, detail);
        }

        let (code, out) = run(&skills.preset_add_cmd(version)?, r)?;
        let detail = if code != 0 {
            tail(&out, 400).to_string()
        } else {
            String::new()
        };
        if check(
            "Dev Cycle preset installed",
            code == 0 && preset_installed(&root),
            detail,
        ) {
            let feature_dir = join("specs", "001-compat-check");
            makedirs(&join(&root, &feature_dir))?;
            write_text_native(
                Path::new(&join(&join(&root, &feature_dir), "spec.md")),
                "# Compatibility check\n",
            )?;
            write_text_native(
                Path::new(&specify(&root, "feature.json")),
                &format!(
                    "{{\"feature_directory\": \"{}\"}}",
                    feature_dir.replace('\\', "/")
                ),
            )?;
            let (code, out) = run(&setup_plan_cmd(&script_name), r)?;
            let plan_path = join(&join(&root, &feature_dir), "plan.md");
            let plan = if isfile(&plan_path) {
                read_text(Path::new(&plan_path))?
            } else {
                String::new()
            };
            let detail = if code != 0 {
                tail(&out, 300).to_string()
            } else {
                String::new()
            };
            check(
                "Feature resolved from .specify/feature.json",
                code == 0 && !plan.is_empty(),
                detail,
            );
            check(
                "Plan template has the Constitution Check",
                plan.contains("Constitution Check"),
                String::new(),
            );
            check(
                "Plan template has the protocol sections (preset)",
                plan.contains(PRESET_MARKER),
                String::new(),
            );
        }
    }
    if !keep {
        let _ = std::fs::remove_dir_all(&root);
    }
    Ok(CheckResult {
        version: version.to_string(),
        integration: integration.clone(),
        script: script.clone(),
        checks,
        dir: keep.then_some(root),
    })
}

fn print_check(result: &CheckResult) {
    println(&format!(
        "🔎 Spec Kit {} compatibility check ({}, {})",
        result.version,
        result.integration.py_str(),
        result.script.py_str()
    ));
    for c in &result.checks {
        let detail = if !c.detail.is_empty() && !c.ok {
            format!(" — {}", c.detail)
        } else {
            String::new()
        };
        println(&format!(
            "   {} {}{detail}",
            if c.ok { "✅" } else { "❌" },
            c.name
        ));
    }
    println(if result.passed() {
        "🟢 COMPATIBLE"
    } else {
        "🔴 NOT COMPATIBLE"
    });
    if let Some(dir) = result.dir.as_deref().filter(|d| !d.is_empty()) {
        println(&format!("   Kept at {dir}"));
    }
}

impl Skills {
    /// Drafts docs/protocol-updates/speckit-<version>.md in the skills repository. Returns the
    /// path, or None when the skills repository cannot be written (the text is printed instead).
    fn write_adaptation_draft(&self, result: &CheckResult, fallback: &Value) -> Option<String> {
        let version = &result.version;
        let mut lines = vec![
            format!("# Protocol update for Spec Kit {version}"),
            String::new(),
            format!(
                "Found on {} by the compatibility check \
                 (`dev-cycle-protocol/speckit_setup.py check --version {version}`, \
                 {}, {}). Projects keep using {} until the protocol is adapted.",
                today(),
                result.integration.py_str(),
                result.script.py_str(),
                fallback.py_str()
            ),
            String::new(),
            "## What diverged".to_string(),
            String::new(),
        ];
        for c in result.checks.iter().filter(|c| !c.ok) {
            let detail = if c.detail.is_empty() {
                "failed"
            } else {
                &c.detail
            };
            lines.push(format!("- **{}**: {detail}", c.name));
        }
        lines.extend([
            String::new(),
            "## What the protocol must change".to_string(),
            String::new(),
            "[Read the Spec Kit release notes and the failed checks; list the changes to the preset, \
             speckit_setup.py, gates and SKILL files]"
                .to_string(),
            String::new(),
            "## Done when".to_string(),
            String::new(),
            format!("`speckit_setup.py check --version {version} --record` passes."),
            String::new(),
        ]);
        let text = lines.join("\n");
        let dir = self.adaptation_dir();
        let path = join(&dir, &format!("speckit-{version}.md"));
        let written = makedirs(&dir).and_then(|()| {
            if exists(&path) {
                Ok(())
            } else {
                write_text(Path::new(&path), &text)
            }
        });
        match written {
            Ok(()) => Some(path),
            Err(_) => {
                println(
                    "⚠️ The skills repository is not writable; give this adaptation document to the user:\n",
                );
                println(&text);
                None
            }
        }
    }
}

// ---------------------------------------------------------------- project commands

fn project_root(start_dir: Option<&str>) -> Result<String, Crash> {
    let start = abspath(start_dir.unwrap_or(""))
        .to_string_lossy()
        .into_owned();
    let (code, out) = run(
        &strs(&["git", "rev-parse", "--show-toplevel"]),
        Some(&start),
    )?;
    Ok(if code == 0 && !out.is_empty() {
        abspath(&out).to_string_lossy().into_owned()
    } else {
        start
    })
}

impl Skills {
    /// The version a new project pins: the requested one, else the latest when it passes the
    /// check, else the last verified one (with an adaptation document drafted).
    fn choose_version(
        &self,
        requested: &Value,
        integration: &Value,
        script: &Value,
    ) -> Res<String> {
        if requested.truthy() {
            parse_version(requested)?;
            return Ok(requested.py_str());
        }
        let latest = latest_release()?;
        let verified = self.verified_version()?;
        if verified.truthy() && parse_str(&latest)? <= parse_version(&verified)? {
            println(&format!(
                "ℹ️ Latest Spec Kit {latest} is already verified ({}).",
                verified.py_str()
            ));
            return Ok(verified.py_str());
        }
        println(&format!(
            "🔎 Spec Kit {latest} is newer than the verified {}; running the compatibility check...",
            or(&verified, s("none")).py_str()
        ));
        let result = compatibility_check(self, &latest, integration, script, false)?;
        print_check(&result);
        if result.passed() {
            if self.record_verified(&latest, integration, script)? {
                println(&format!(
                    "✅ Verified version moved to {latest} ({}); commit it in the skills repository.",
                    compat_rel()
                ));
            }
            return Ok(latest);
        }
        if !verified.truthy() {
            return setup(
                "the latest Spec Kit release is not compatible and no verified version exists"
                    .into(),
            );
        }
        if let Some(path) = self.write_adaptation_draft(&result, &verified) {
            println(&format!("📝 Adaptation document drafted: {path}"));
            println(&format!(
                "   Capture it in the skills repository's backlog: python \"{}\" \
                 add --type improvement --title \"Adapt the protocol to Spec Kit {latest}\" \
                 --description \"See docs/protocol-updates/speckit-{latest}.md\" --dir \"{}\"",
                join(&join(&self.root, "plan-memory"), "backlog.py"),
                self.root
            ));
        }
        println(&format!(
            "↩️ Using the last verified version {}.",
            verified.py_str()
        ));
        Ok(verified.py_str())
    }
}

/// `os.path.relpath(COMPAT_FILE, SKILLS_ROOT)`.
fn compat_rel() -> String {
    join(&join("dev-cycle-protocol", "speckit-preset"), "compat.json")
}

fn execute(cmds: &[Vec<Value>], root: &str, dry_run: bool) -> Res<()> {
    for cmd in cmds {
        println(&format!(
            "$ {}",
            kernel::py::join(" ", &Value::List(cmd.clone()))?
        ));
        if dry_run {
            continue;
        }
        let (code, out) = run(cmd, Some(root))?;
        if code != 0 {
            return setup(format!("command failed ({code}):\n{}", tail(&out, 1500)));
        }
    }
    Ok(())
}

fn arg(args: &Parsed, dest: &str) -> Value {
    args.value(dest).map_or(Value::Null, s)
}

fn cmd_init(skills: &Skills, args: &Parsed) -> Res<u8> {
    let root = project_root(args.value("dir"))?;
    let config = load_project_config(Some(&root))?;
    let cfg = config.section("speckit");
    if isdir(&join(&root, ".specify")) {
        return setup(format!(
            "Spec Kit is already initialized in {root}; use 'upgrade --to <version>' to change versions"
        ));
    }
    let integration = or(
        &or(
            &arg(args, "integration"),
            cfg.get_or_none("integration").clone(),
        ),
        s("claude"),
    );
    let script = or(
        &or(&arg(args, "script"), cfg.get_or_none("script").clone()),
        s(default_script()),
    );
    let prefix = or(cfg.get_or_none("branch_prefix"), s("feat"));
    let dry_run = args.flag("dry_run");
    if !dry_run {
        require_uv()?;
    }
    let requested = or(&arg(args, "version"), cfg.get_or_none("version").clone());
    let version = skills.choose_version(&requested, &integration, &script)?;
    execute(
        &[init_cmd(&version, &integration, &script)?],
        &root,
        dry_run,
    )?;
    if !dry_run {
        set_branch_prefix(&root, &prefix)?;
    }
    execute(&[skills.preset_add_cmd(&version)?], &root, dry_run)?;
    if dry_run {
        println(&format!(
            "(dry run) would pin speckit.version = {version} in {MANIFEST_NAME}"
        ));
        return Ok(0);
    }
    let path = write_speckit_config(
        &root,
        &[
            ("enabled", Value::Bool(true)),
            ("version", s(&version)),
            ("integration", integration.clone()),
            ("script", script.clone()),
            ("branch_prefix", prefix.clone()),
        ],
    )?;
    println(&format!(
        "✅ Spec Kit {version} initialized ({}, {}), branches '{}/NNN-slug', preset '{PRESET_ID}'.",
        integration.py_str(),
        script.py_str(),
        prefix.py_str()
    ));
    println(&format!(
        "   Pinned in {path}. Next: generate the constitution (dev-cycle-protocol/constitution.py generate),"
    ));
    println("   then commit .specify/, the agent command files and .dev-cycle.json.");
    Ok(0)
}

/// (integration, script) recorded by Spec Kit in the project, or (None, None).
fn installed_settings(root: &str) -> Result<(Value, Value), Crash> {
    let integration_json = read_json(&specify(root, "integration.json"));
    let options = read_json(&specify(root, "init-options.json"));
    let integration = or(
        attr_get(&integration_json, "integration")?,
        attr_get(&options, "integration")?.clone(),
    );
    let all = or(
        attr_get(&integration_json, "integration_settings")?,
        Value::Dict(Dict::new()),
    );
    let Value::Dict(all) = &all else {
        return Err(no_get(&all));
    };
    let key = or(&integration, s(""));
    if !key.is_hashable() {
        return Err(Crash::new(format!(
            "TypeError: unhashable type: '{}'",
            key.type_name()
        )));
    }
    let settings = all
        .get_value(&key)
        .cloned()
        .unwrap_or_else(|| Value::Dict(Dict::new()));
    let script = or(
        attr_get(&settings, "script")?,
        attr_get(&options, "script")?.clone(),
    );
    Ok((integration, script))
}

fn extension_add_cmd(version: &str, extension: &str) -> Res<Vec<Value>> {
    let mut cmd = specify_cmd(version)?;
    cmd.extend(strs(&["extension", "add", extension]));
    Ok(cmd)
}

/// Brings a project that used Spec Kit without the protocol under it: pins the installed
/// version (when it passes the compatibility check), the git extension with feat/NNN-slug
/// branches, the preset.
fn cmd_adopt(skills: &Skills, args: &Parsed) -> Res<u8> {
    let root = project_root(args.value("dir"))?;
    let config = load_project_config(Some(&root))?;
    let cfg = config.section("speckit");
    if !isdir(&join(&root, ".specify")) {
        return setup(format!("Spec Kit is not initialized in {root}; use 'init'"));
    }
    if cfg.get_or_none("enabled").truthy() && cfg.get_or_none("version").truthy() {
        return setup(
            "Spec Kit is already managed by the protocol here (speckit.enabled); use 'status' or 'upgrade'"
                .into(),
        );
    }
    let version = installed_version(&root);
    let verified = skills.verified_version()?;
    if !version.truthy() {
        return setup(format!(
            "the installed Spec Kit version is unknown (no .specify/integration.json or init-options.json); \
             run 'upgrade --to {}' after enabling, or re-initialize",
            verified.py_str()
        ));
    }
    let version = version.py_str();
    let (integration, script) = installed_settings(&root)?;
    let integration = or(
        &or(&integration, cfg.get_or_none("integration").clone()),
        s("claude"),
    );
    let script = or(
        &or(&script, cfg.get_or_none("script").clone()),
        s(default_script()),
    );
    let prefix = or(cfg.get_or_none("branch_prefix"), s("feat"));
    let dry_run = args.flag("dry_run");
    if !dry_run {
        require_uv()?;
        if !verified.py_eq(&s(&version)) {
            let result = compatibility_check(skills, &version, &integration, &script, false)?;
            print_check(&result);
            if !result.passed() {
                let v = verified.py_str();
                return setup(format!(
                    "the installed Spec Kit {version} is not compatible with the protocol; upgrade it to the \
                     verified {v} first (speckit_setup.py upgrade --to {v}), then adopt again"
                ));
            }
        }
    }
    let mut cmds = Vec::new();
    if !isfile(&git_config_path(&root)) {
        cmds.push(extension_add_cmd(&version, "git")?);
    }
    cmds.push(if preset_installed(&root) {
        skills.preset_update_cmd(&version)?
    } else {
        skills.preset_add_cmd(&version)?
    });
    execute(&cmds, &root, dry_run)?;
    if dry_run {
        println(&format!(
            "(dry run) would pin speckit.version = {version} and set branches {}/NNN-slug",
            prefix.py_str()
        ));
        return Ok(0);
    }
    set_branch_prefix(&root, &prefix)?;
    write_speckit_config(
        &root,
        &[
            ("enabled", Value::Bool(true)),
            ("version", s(&version)),
            ("integration", integration.clone()),
            ("script", script.clone()),
            ("branch_prefix", prefix.clone()),
        ],
    )?;
    println(&format!(
        "✅ Spec Kit {version} adopted ({}, {}), branches '{}/NNN-slug', preset '{PRESET_ID}'. \
         Existing specs/ are kept.",
        integration.py_str(),
        script.py_str(),
        prefix.py_str()
    ));
    println(
        "   Next: the constitution. If it was written by hand, move its principles into the sources (TECH_SPEC.md, ADRs) \
         with the user, then run constitution.py generate --replace-hand-written (a copy is kept).",
    );
    Ok(0)
}

fn cmd_upgrade(skills: &Skills, args: &Parsed) -> Res<u8> {
    let root = project_root(args.value("dir"))?;
    let config = load_project_config(Some(&root))?;
    let cfg = config.section("speckit");
    if !isdir(&join(&root, ".specify")) {
        return setup(format!("Spec Kit is not initialized in {root}; use 'init'"));
    }
    let current = or(cfg.get_or_none("version"), installed_version(&root));
    let target = args.value("to").unwrap_or_default().to_string();
    if current.truthy() && parse_str(&target)? <= parse_version(&current)? {
        return setup(format!(
            "{target} is not newer than the pinned {}",
            current.py_str()
        ));
    }
    let integration = or(cfg.get_or_none("integration"), s("claude"));
    let script = or(cfg.get_or_none("script"), s(default_script()));
    let dry_run = args.flag("dry_run");
    if !dry_run {
        require_uv()?;
        let verified = skills.verified_version()?;
        if !verified.truthy() || parse_str(&target)? > parse_version(&verified)? {
            let result = compatibility_check(skills, &target, &integration, &script, false)?;
            print_check(&result);
            if !result.passed() {
                let path = skills.write_adaptation_draft(&result, &or(&current, verified.clone()));
                let mut msg = format!(
                    "Spec Kit {target} is not compatible with the protocol; the project stays on {}.",
                    current.py_str()
                );
                if let Some(p) = path {
                    msg.push_str(&format!(" Adaptation document: {p}"));
                }
                return setup(msg);
            }
            skills.record_verified(&target, &integration, &script)?;
        }
    }
    execute(
        &skills.upgrade_cmds(&target, &integration, &script)?,
        &root,
        dry_run,
    )?;
    if dry_run {
        return Ok(0);
    }
    let prefix = or(cfg.get_or_none("branch_prefix"), s("feat"));
    let now = read_branch_prefix(&root)?;
    if !now.map_or(Value::Null, |p| s(&p)).py_eq(&prefix) {
        set_branch_prefix(&root, &prefix)?;
    }
    write_speckit_config(&root, &[("version", s(&target))])?;
    println(&format!(
        "✅ Spec Kit upgraded {} -> {target}. Review the diff (locally edited files are kept), \
         regenerate the constitution if templates changed, and commit.",
        current.py_str()
    ));
    Ok(0)
}

fn prefix_value(prefix: Option<String>) -> Value {
    prefix.map_or(Value::Null, Value::Str)
}

fn status_data(skills: &Skills, root: &str) -> Res<Dict> {
    let config = load_project_config(Some(root))?;
    let cfg = config.section("speckit");
    let initialized = isdir(&join(root, ".specify"));
    let pinned = cfg.get_or_none("version").clone();
    let installed = if initialized {
        installed_version(root)
    } else {
        Value::Null
    };
    let mut problems: Vec<Value> = Vec::new();
    let mut problem = |p: String| problems.push(Value::Str(p));
    if cfg.get_or_none("enabled").truthy() {
        if !initialized {
            problem(
                "speckit.enabled is true but Spec Kit is not initialized (run speckit_setup.py init)"
                    .into(),
            );
        }
        if !pinned.truthy() {
            problem("speckit.version is not set".into());
        } else if installed.truthy() && !installed.py_eq(&pinned) {
            problem(format!(
                "pinned {} but the project was set up with {}",
                pinned.py_str(),
                installed.py_str()
            ));
        }
        if initialized && !preset_installed(root) {
            problem(format!("preset '{PRESET_ID}' is not installed"));
        }
        let prefix = or(cfg.get_or_none("branch_prefix"), s("feat"));
        if initialized && !prefix_value(read_branch_prefix(root)?).py_eq(&prefix) {
            let found = read_branch_prefix(root)?.map_or("None".to_string(), |p| repr_str(&p));
            problem(format!(
                "git extension branch_prefix is {found}, expected '{}'",
                prefix.py_str()
            ));
        }
    }
    let mut d = Dict::new();
    d.insert("enabled", Value::Bool(cfg.get_or_none("enabled").truthy()));
    d.insert("pinned", pinned);
    d.insert("installed", installed);
    d.insert("integration", cfg.get_or_none("integration").clone());
    d.insert("script", cfg.get_or_none("script").clone());
    d.insert("initialized", Value::Bool(initialized));
    d.insert("preset", Value::Bool(initialized && preset_installed(root)));
    let prefix = if initialized {
        prefix_value(read_branch_prefix(root)?)
    } else {
        Value::Null
    };
    d.insert("branch_prefix", prefix);
    d.insert("verified", skills.verified_version()?);
    d.insert("uv", Value::Bool(which("uvx").is_some()));
    d.insert("problems", Value::List(problems));
    Ok(d)
}

fn cmd_status(skills: &Skills, args: &Parsed) -> Res<u8> {
    let data = status_data(skills, &project_root(args.value("dir"))?)?;
    let problems = data
        .get_or_none("problems")
        .as_list()
        .cloned()
        .unwrap_or_default();
    let code = u8::from(!problems.is_empty());
    if args.flag("json") {
        println(&dumps_ascii(&Value::Dict(data), Some(2))?);
        return Ok(code);
    }
    let field = |key: &str| or(data.get_or_none(key), s("-")).py_str();
    let enabled = data.get_or_none("enabled").truthy();
    println(&format!(
        "🧩 Spec Kit: {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    println(&format!(
        "   Pinned / installed : {} / {} (verified by the protocol: {})",
        field("pinned"),
        field("installed"),
        field("verified")
    ));
    println(&format!(
        "   Integration, script: {}, {}",
        field("integration"),
        field("script")
    ));
    let preset = if data.get_or_none("preset").truthy() {
        "installed"
    } else {
        "missing"
    };
    println(&format!(
        "   Preset, branches   : {preset}, {}/NNN-slug",
        field("branch_prefix")
    ));
    let uv = if data.get_or_none("uv").truthy() {
        "available"
    } else {
        "missing"
    };
    println(&format!("   uv                 : {uv}"));
    for p in &problems {
        println(&format!("⚠️  {}", p.py_str()));
    }
    Ok(code)
}

fn cmd_check(skills: &Skills, args: &Parsed) -> Res<u8> {
    require_uv()?;
    let version = match args.value("version").filter(|v| !v.is_empty()) {
        Some(v) => v.to_string(),
        None => latest_release()?,
    };
    let integration = s(args.value("integration").unwrap_or("claude"));
    let script = s(args.value("script").unwrap_or(default_script()));
    let result = compatibility_check(skills, &version, &integration, &script, args.flag("keep"))?;
    if args.flag("json") {
        println(&dumps_ascii(&result.to_value(), Some(2))?);
    } else {
        print_check(&result);
    }
    if result.passed()
        && args.flag("record")
        && skills.record_verified(&version, &integration, &script)?
    {
        println(&format!(
            "✅ Verified version moved to {version}; commit {}.",
            compat_rel()
        ));
    }
    Ok(u8::from(!result.passed()))
}

fn command_parser(name: &str) -> Parser {
    let p = Parser::new(&format!("speckit_setup.py {name}"), "");
    match name {
        "status" => p.option("--dir", "").flag("--json", ""),
        "check" => p
            .option("--version", "Release tag (default: the latest release)")
            .option("--integration", "")
            .choice_option("--script", &SCRIPT_TYPES, "")
            .flag(
                "--record",
                "When it passes, move the verified version forward",
            )
            .flag("--keep", "Keep the temporary repository")
            .flag("--json", ""),
        "init" => p
            .option("--dir", "")
            .option(
                "--version",
                "Release tag (default: latest if compatible, else the verified one)",
            )
            .option(
                "--integration",
                "Agent integration (default: speckit.integration or claude)",
            )
            .choice_option("--script", &SCRIPT_TYPES, "")
            .flag("--dry-run", ""),
        "adopt" => p.option("--dir", "").flag("--dry-run", ""),
        "upgrade" => p
            .option("--dir", "")
            .required_option("--to", "Release tag")
            .flag("--dry-run", ""),
        _ => p,
    }
}

pub fn main(argv: &[String]) -> u8 {
    match run_main(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run_main(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "speckit_setup.py",
        "Install, pin, check and upgrade Spec Kit for dev-cycle-protocol",
    )
    .commands(
        "command",
        &[
            ("status", "Spec Kit state of the project"),
            ("latest", "Latest Spec Kit release and the verified version"),
            (
                "check",
                "Compatibility check of a Spec Kit version in a temporary repository",
            ),
            (
                "init",
                "Initialize Spec Kit in the project at a pinned version",
            ),
            (
                "adopt",
                "Bring a project that used Spec Kit without the protocol under it",
            ),
            (
                "upgrade",
                "Upgrade the project's Spec Kit after a compatibility check",
            ),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let Some(root) = kernel::skills::root() else {
        err(&format!(
            "dev-cycle: cannot find the skills folder; set {} to it\n",
            kernel::skills::ENV
        ));
        return Ok(2);
    };
    let skills = Skills {
        root: abspath(&root.to_string_lossy())
            .to_string_lossy()
            .into_owned(),
    };
    let result = match args.value("command").unwrap_or_default() {
        "status" => cmd_status(&skills, &args),
        "latest" => latest_release().and_then(|latest| {
            let verified = or(&skills.verified_version()?, s("-"));
            println(&format!(
                "Latest Spec Kit release: {latest} (verified by the protocol: {})",
                verified.py_str()
            ));
            Ok(0)
        }),
        "check" => cmd_check(&skills, &args),
        "init" => cmd_init(&skills, &args),
        "adopt" => cmd_adopt(&skills, &args),
        _ => cmd_upgrade(&skills, &args),
    };
    match result {
        Ok(code) => Ok(code),
        Err(Error::Setup(e)) => {
            println(&format!("❌ {e}"));
            Ok(1)
        }
        Err(Error::Crash(c)) => Err(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_exact_release_tags() {
        assert!(version_match("v1.2.3").is_some());
        assert!(version_match("v1.2.3\n").is_some());
        for bad in ["1.2.3", "latest", "main", "v1.2", "v1.2.3-rc.1", "v1..3"] {
            assert!(version_match(bad).is_none(), "{bad}");
        }
        let tags: Vec<String> = ["v1.2.3", "v1.10.0", "v1.9.9", "v2.0.0-rc.1", "main"]
            .map(String::from)
            .to_vec();
        assert_eq!(newest(&tags).as_deref(), Some("v1.10.0"));
    }

    #[test]
    fn templates_expand_as_re_sub() {
        assert_eq!(expand_template(r"a\n\t\\\-", "b").unwrap(), "a\n\t\\\\-");
        assert_eq!(expand_template(r"a\123", "b").unwrap(), "aS");
        assert_eq!(expand_template(r"a\0123", "b").unwrap(), "a\n3");
        let e = |t: &str| expand_template(t, "b").unwrap_err().0;
        assert_eq!(e(r"a\q"), r"re.PatternError: bad escape \q at position 1");
        assert_eq!(
            e(r"a\12"),
            "re.PatternError: invalid group reference 12 at position 2"
        );
        assert_eq!(e(r"a\g<x>"), "IndexError: unknown group name 'x'");
        assert_eq!(
            e(r"a\g<1x>"),
            "re.PatternError: bad character in group name '1x' at position 4"
        );
        assert_eq!(
            e("a\\400"),
            r"re.PatternError: octal escape value \400 outside of range 0-0o377 at position 1"
        );
    }

    #[test]
    fn command_files_are_found_by_name() {
        assert_eq!(
            command_in(".claude/skills/speckit-plan/SKILL.md"),
            Some("plan")
        );
        assert_eq!(
            command_in(".claude/commands/speckit.tasks.md"),
            Some("tasks")
        );
        assert_eq!(command_in(".claude/skills/speckit-planner"), None);
        assert_eq!(command_in("x/speckit.specifySKILL"), Some("specify"));
    }
}
