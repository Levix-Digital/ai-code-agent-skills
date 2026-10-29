//! Project Configuration - dev-cycle-protocol Skill
//! Single loader for `.dev-cycle.json`, the project configuration shared by every skill.
//!
//! The file sits at the repository root. Each skill owns one section; this module fills
//! defaults, maps the legacy flat keys of older manifests into their sections, and reports
//! problems. Translated from `dev-cycle-protocol/project_config.py`.
//!
//! CLI: `dev-cycle dev-cycle-protocol project_config [--dir <path>] [--json] [--validate] [--migrate [--write]]`

use crate::out::println;
use crate::py::argparse::Parser;
use crate::py::json::{self, LoadError};
use crate::py::path::{abspath, basename};
use crate::py::{Crash, Dict, EMPTY, Value, is_word, join, matches_to_end};
use std::path::{Path, PathBuf};

pub const MANIFEST_NAME: &str = ".dev-cycle.json";

pub const ARCHITECTURE_STYLES: [&str; 6] = [
    "hexagonal",
    "layered",
    "mvc",
    "pipeline",
    "modular-monolith",
    "other",
];
pub const BUG_SEVERITIES: [&str; 4] = ["critical", "high", "medium", "low"];
pub const BUG_DEADLINES: [&str; 2] = ["next_release", "off"];
pub const OBSERVABILITY_PROFILES: [&str; 5] = ["cli", "service", "library", "desktop", "none"];
pub const SPECKIT_SCRIPTS: [&str; 3] = ["sh", "ps", "py"];
pub const REVIEW_PROVIDERS: [&str; 4] = ["auto", "github", "gitlab", "none"];
pub const BRANCHING_MODELS: [&str; 3] = ["integration", "trunk", "release-branches"];

fn section(entries: &[(&str, Value)]) -> Value {
    Value::Dict(
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}

fn s(v: &str) -> Value {
    Value::str(v)
}

fn int(v: i64) -> Value {
    Value::Int(v.to_string())
}

/// Sections and their defaults. `None` means "not set": skills fall back to auto-detection.
pub fn defaults() -> Dict {
    let mut d = Dict::new();
    d.insert("project_name", Value::Null);
    d.insert(
        "architecture",
        section(&[
            ("style", Value::Null),
            ("core_path", Value::Null),
            ("surfaces", Value::List(vec![])),
            ("canonical_datastore", Value::Null),
            ("forbidden_core_imports", Value::List(vec![])),
        ]),
    );
    d.insert("test_commands", Value::List(vec![]));
    d.insert(
        "docs",
        section(&[
            ("user_doc", s("README.md")),
            ("tec_doc", s("TECDOC.md")),
            ("readme_max_lines", int(400)),
        ]),
    );
    d.insert(
        "tests",
        section(&[("manual_tests", s("tests/MANUAL_TESTS.md"))]),
    );
    d.insert(
        "devops",
        section(&[
            ("branching", s("integration")),
            ("integration_branch", s("dev")),
            ("production_branch", s("main")),
            ("releases_repo", Value::Null),
            ("release_candidates", Value::Bool(true)),
            ("release_branch_prefix", s("release")),
            ("version_files", Value::List(vec![])),
            ("observability", Value::Null),
        ]),
    );
    d.insert("plans", section(&[("dir", s("docs/plans"))]));
    d.insert(
        "backlog",
        section(&[
            ("file", s("docs/BACKLOG.md")),
            (
                "blocking_severities",
                Value::list_of_str(&["critical", "high", "medium"]),
            ),
            ("bug_deadline", s("next_release")),
        ]),
    );
    d.insert("roadmap", section(&[("file", s("docs/ROADMAP.md"))]));
    d.insert(
        "product",
        section(&[
            ("vision", s("docs/product/VISION.md")),
            ("tech_spec", s("docs/product/TECH_SPEC.md")),
            ("max_lines", int(400)),
        ]),
    );
    d.insert(
        "review",
        section(&[
            ("provider", s("auto")),
            ("host", Value::Null),
            ("marker", s("[review]")),
            ("reply_marker", s("[reply]")),
            ("auto_review", Value::Bool(true)),
            ("allowed_terms", Value::List(vec![])),
        ]),
    );
    d.insert(
        "speckit",
        section(&[
            ("enabled", Value::Bool(false)),
            ("version", Value::Null),
            ("integration", s("claude")),
            ("script", Value::Null),
            ("branch_prefix", s("feat")),
        ]),
    );
    d.insert(
        "gedo",
        section(&[
            ("enabled", Value::Bool(false)),
            ("feedback", Value::Bool(true)),
            ("feedback_dir", s("docs/gedo-feedback")),
            ("repo", s("Levix-Digital/gedo")),
        ]),
    );
    // Owned by other skills; passed through untouched.
    d.insert("architecture_docs", Value::Dict(Dict::new()));
    d.insert("drift_rules", Value::List(vec![]));
    d
}

/// Legacy flat keys -> (section, key). The new location wins when both are present.
pub const LEGACY_KEYS: [(&str, &str, &str); 5] = [
    ("core_path", "architecture", "core_path"),
    ("surfaces", "architecture", "surfaces"),
    ("canonical_datastore", "architecture", "canonical_datastore"),
    ("releases_repo", "devops", "releases_repo"),
    ("plans_dir", "plans", "dir"),
];
pub const LEGACY_DOC_FILES: [(&str, &str, &str); 3] = [
    ("user_doc", "docs", "user_doc"),
    ("tec_doc", "docs", "tec_doc"),
    ("manual_tests", "tests", "manual_tests"),
];

fn is_legacy_key(key: &str) -> bool {
    LEGACY_KEYS.iter().any(|(k, _, _)| *k == key)
}

/// `key in raw.get("doc_files", {})`, then `raw["doc_files"][key]`, failing where Python fails.
fn legacy_doc_file(raw: &Dict, key: &str) -> Result<Option<Value>, Crash> {
    match raw.get("doc_files") {
        None => Ok(None),
        Some(Value::Dict(d)) => Ok(d.get(key).cloned()),
        Some(Value::List(items)) => {
            if items.iter().any(|v| v.as_str() == Some(key)) {
                Err(Crash::new(
                    "TypeError: list indices must be integers or slices, not str",
                ))
            } else {
                Ok(None)
            }
        }
        Some(Value::Str(text)) => {
            if text.contains(key) {
                Err(Crash::new(
                    "TypeError: string indices must be integers, not 'str'",
                ))
            } else {
                Ok(None)
            }
        }
        Some(other) => Err(Crash::new(format!(
            "TypeError: argument of type '{}' is not iterable",
            other.type_name()
        ))),
    }
}

/// Nearest `.dev-cycle.json`, walking up from `start_dir` (default: current directory).
pub fn find_manifest(start_dir: Option<&str>) -> Option<PathBuf> {
    let mut current = abspath(start_dir.unwrap_or(""));
    loop {
        let candidate = current.join(MANIFEST_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        current = current.parent()?.to_path_buf();
    }
}

/// Effective configuration: defaults, legacy keys mapped, then the file's own sections.
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub raw: Dict,
    pub manifest_path: Option<PathBuf>,
    pub legacy_keys_used: Vec<String>,
    pub data: Dict,
}

impl ProjectConfig {
    pub fn new(root: PathBuf, raw: Dict, manifest_path: Option<PathBuf>) -> Result<Self, Crash> {
        let mut cfg = ProjectConfig {
            root,
            raw,
            manifest_path,
            legacy_keys_used: Vec::new(),
            data: Dict::new(),
        };
        cfg.data = cfg.resolve()?;
        Ok(cfg)
    }

    fn resolve(&mut self) -> Result<Dict, Crash> {
        let mut data = defaults();
        data.insert("project_name", Value::str(basename(&self.root)));

        for (key, sec, sub) in LEGACY_KEYS {
            if let Some(v) = self.raw.get(key) {
                if let Some(Value::Dict(d)) = data.get_mut(sec) {
                    d.insert(sub, v.clone());
                }
                self.legacy_keys_used.push(key.to_string());
            }
        }
        for (key, sec, sub) in LEGACY_DOC_FILES {
            if let Some(v) = legacy_doc_file(&self.raw, key)? {
                if let Some(Value::Dict(d)) = data.get_mut(sec) {
                    d.insert(sub, v);
                }
                self.legacy_keys_used.push(format!("doc_files.{key}"));
            }
        }

        for (key, value) in self.raw.str_iter() {
            if is_legacy_key(key) || key == "doc_files" {
                continue;
            }
            match (data.get(key), value) {
                (Some(Value::Dict(base)), Value::Dict(over)) => {
                    let mut merged = base.clone();
                    for (k, v) in over.iter() {
                        merged.insert_value(k.clone(), v.clone());
                    }
                    data.insert(key, Value::Dict(merged));
                }
                _ => data.insert(key, value.clone()),
            }
        }
        Ok(data)
    }

    pub fn get(&self, key: &str) -> &Value {
        self.data.get_or_none(key)
    }

    pub fn section(&self, name: &str) -> &Dict {
        self.data
            .get(name)
            .and_then(Value::as_dict)
            .unwrap_or(&EMPTY)
    }

    pub fn project_name(&self) -> String {
        let v = self.get("project_name");
        if v.truthy() {
            v.py_str()
        } else {
            basename(&self.root)
        }
    }

    /// True when the file itself sets `section.key` (a default does not count): tells an
    /// explicit `false` from a decision never taken.
    pub fn is_set(&self, section: &str, key: &str) -> bool {
        self.raw
            .get(section)
            .and_then(Value::as_dict)
            .is_some_and(|d| d.contains_key(key))
    }

    /// Absolute path of a path-valued setting, or None when unset.
    pub fn path(&self, section: &str, key: &str) -> Option<PathBuf> {
        let v = self.section(section).get_or_none(key);
        if v.truthy() {
            Some(self.root.join(v.py_str()))
        } else {
            None
        }
    }

    /// Human-readable problems. An empty list means the configuration is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let one_of = |v: &[&str]| v.join(", ");
        let arch = self.section("architecture");
        let devops = self.section("devops");
        let style = arch.get_or_none("style");
        if !style.is_none() && !style.is_one_of(&ARCHITECTURE_STYLES) {
            problems.push(format!(
                "architecture.style '{}' is not one of: {}",
                style.py_str(),
                one_of(&ARCHITECTURE_STYLES)
            ));
        }
        let obs = devops.get_or_none("observability");
        if !obs.is_none() && !obs.is_one_of(&OBSERVABILITY_PROFILES) {
            problems.push(format!(
                "devops.observability '{}' is not one of: {}",
                obs.py_str(),
                one_of(&OBSERVABILITY_PROFILES)
            ));
        }
        if !self.get("test_commands").is_list() {
            problems.push("test_commands must be a list of shell commands".into());
        }
        let branching = devops.get_or_none("branching");
        if !branching.is_one_of(&BRANCHING_MODELS) {
            problems.push(format!(
                "devops.branching '{}' is not one of: {}",
                branching.py_str(),
                one_of(&BRANCHING_MODELS)
            ));
        }
        let prefix = devops.get_or_none("release_branch_prefix");
        let prefix_ok = prefix.as_str().is_some_and(|p| {
            matches_to_end(p, |t| {
                !t.is_empty()
                    && t.split('/').all(|seg| {
                        !seg.is_empty()
                            && seg
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
                    })
            })
        });
        if !prefix_ok {
            problems.push(format!(
                "devops.release_branch_prefix '{}' must be a branch name prefix like 'release'",
                prefix.py_str()
            ));
        }
        if !devops.get_or_none("release_candidates").is_bool() {
            problems.push("devops.release_candidates must be true or false".into());
        }
        for key in ["surfaces", "forbidden_core_imports"] {
            if !arch.get_or_none(key).is_list() {
                problems.push(format!("architecture.{key} must be a list"));
            }
        }
        if !devops.get_or_none("version_files").is_list() {
            problems.push("devops.version_files must be a list of paths".into());
        }
        let backlog = self.section("backlog");
        let blocking_ok = match backlog.get_or_none("blocking_severities") {
            Value::List(items) => items.iter().all(|s| s.is_one_of(&BUG_SEVERITIES)),
            _ => false,
        };
        if !blocking_ok {
            problems.push(format!(
                "backlog.blocking_severities must be a list of: {}",
                one_of(&BUG_SEVERITIES)
            ));
        }
        if !backlog
            .get_or_none("bug_deadline")
            .is_one_of(&BUG_DEADLINES)
        {
            problems.push(format!(
                "backlog.bug_deadline must be one of: {}",
                one_of(&BUG_DEADLINES)
            ));
        }
        let speckit = self.section("speckit");
        if !speckit.get_or_none("enabled").is_bool() {
            problems.push("speckit.enabled must be true or false".into());
        }
        let version = speckit.get_or_none("version");
        if !version.is_none() && !version.as_str().is_some_and(is_release_tag) {
            problems.push(format!(
                "speckit.version '{}' must be an exact release tag like v1.2.3",
                version.py_str()
            ));
        }
        if speckit.get_or_none("enabled").truthy() && !version.truthy() {
            problems.push(
                "speckit.version must be set when speckit.enabled is true (run speckit_setup.py init)"
                    .into(),
            );
        }
        let script = speckit.get_or_none("script");
        if !script.is_none() && !script.is_one_of(&SPECKIT_SCRIPTS) {
            problems.push(format!(
                "speckit.script '{}' is not one of: {}",
                script.py_str(),
                one_of(&SPECKIT_SCRIPTS)
            ));
        }
        let review = self.section("review");
        let provider = review.get_or_none("provider");
        if !provider.is_one_of(&REVIEW_PROVIDERS) {
            problems.push(format!(
                "review.provider '{}' is not one of: {}",
                provider.py_str(),
                one_of(&REVIEW_PROVIDERS)
            ));
        }
        if !review.get_or_none("auto_review").is_bool() {
            problems.push("review.auto_review must be true or false".into());
        }
        if !review.get_or_none("allowed_terms").is_list() {
            problems.push("review.allowed_terms must be a list of phrases".into());
        }
        let gedo = self.section("gedo");
        for key in ["enabled", "feedback"] {
            if !gedo.get_or_none(key).is_bool() {
                problems.push(format!("gedo.{key} must be true or false"));
            }
        }
        let feedback_dir = gedo.get_or_none("feedback_dir");
        if !(feedback_dir.as_str().is_some() && feedback_dir.truthy()) {
            problems.push("gedo.feedback_dir must be a folder path".into());
        }
        let repo = gedo.get_or_none("repo");
        if !repo.as_str().is_some_and(is_repo_name) {
            problems.push(format!(
                "gedo.repo '{}' must be an owner/name repository",
                repo.py_str()
            ));
        }
        let defaults = defaults();
        for (key, _) in self.raw.str_iter() {
            if !(defaults.contains_key(key) || is_legacy_key(key) || key == "doc_files") {
                problems.push(format!("unknown top-level key '{key}' (kept as is)"));
            }
        }
        problems
    }

    /// The raw file rewritten with sections instead of legacy flat keys.
    pub fn migrated(&self) -> Result<Dict, Crash> {
        let mut out = Dict::new();
        for (key, value) in self.raw.str_iter() {
            if is_legacy_key(key) || key == "doc_files" {
                continue;
            }
            out.insert(key, value.clone());
        }
        let setdefault =
            |out: &mut Dict, sec: &str, sub: &str, value: Value| -> Result<(), Crash> {
                if !out.contains_key(sec) {
                    out.insert(sec, Value::Dict(Dict::new()));
                }
                match out.get_mut(sec) {
                    Some(Value::Dict(d)) => {
                        if !d.contains_key(sub) {
                            d.insert(sub, value);
                        }
                        Ok(())
                    }
                    Some(other) => Err(Crash::new(format!(
                        "AttributeError: '{}' object has no attribute 'setdefault'",
                        other.type_name()
                    ))),
                    None => Ok(()),
                }
            };
        for (key, sec, sub) in LEGACY_KEYS {
            if let Some(v) = self.raw.get(key) {
                setdefault(&mut out, sec, sub, v.clone())?;
            }
        }
        for (key, sec, sub) in LEGACY_DOC_FILES {
            if let Some(v) = legacy_doc_file(&self.raw, key)? {
                setdefault(&mut out, sec, sub, v)?;
            }
        }
        let order: Vec<String> = defaults().str_iter().map(|(k, _)| k.to_string()).collect();
        let rank = |k: &str| order.iter().position(|o| o == k).unwrap_or(order.len());
        let mut entries: Vec<(String, Value)> = out
            .str_iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        entries.sort_by_key(|(k, _)| rank(k));
        Ok(entries.into_iter().collect())
    }
}

/// `^v\d+\.\d+\.\d+$`.
fn is_release_tag(s: &str) -> bool {
    matches_to_end(s, |t| {
        let Some(rest) = t.strip_prefix('v') else {
            return false;
        };
        let parts: Vec<&str> = rest.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(is_decimal))
    })
}

/// `\d` of Python's `re`: Unicode decimal digits.
fn is_decimal(c: char) -> bool {
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    c.is_ascii_digit()
        || RE
            .get_or_init(|| Regex::new(r"^\d$").expect("valid"))
            .is_match(&c.to_string())
}

/// `^[\w.-]+/[\w.-]+$`.
fn is_repo_name(s: &str) -> bool {
    matches_to_end(s, |t| {
        let Some((owner, name)) = t.split_once('/') else {
            return false;
        };
        let ok = |p: &str| !p.is_empty() && p.chars().all(|c| is_word(c) || c == '.' || c == '-');
        ok(owner) && ok(name)
    })
}

/// One line for the branching model, e.g. 'integration: dev -> main' or 'trunk: main'.
pub fn branch_summary(devops: &Dict) -> String {
    let or = |key: &str, default: &str| {
        let v = devops.get_or_none(key);
        if v.truthy() {
            v.py_str()
        } else {
            default.to_string()
        }
    };
    let model = devops.get_or_none("branching");
    let model_name = or("branching", "integration");
    let production = or("production_branch", "main");
    if !model.truthy() || model.as_str() == Some("integration") {
        return format!(
            "integration: {} -> {production}",
            or("integration_branch", "dev")
        );
    }
    if model.as_str() == Some("release-branches") {
        return format!(
            "release-branches: {production} -> {}/X.Y",
            or("release_branch_prefix", "release")
        );
    }
    format!("{model_name}: {production}")
}

#[derive(Debug)]
pub enum ConfigError {
    /// `ValueError`: the scripts that catch it print it and stop.
    Invalid(String),
    /// `json.JSONDecodeError`, a `ValueError` too, for a manifest that is not JSON.
    Decode(String),
    Crash(Crash),
}

impl ConfigError {
    /// The error as the traceback of a script that does not catch it.
    pub fn into_crash(self) -> Crash {
        match self {
            ConfigError::Invalid(e) => Crash::new(format!("ValueError: {e}")),
            ConfigError::Decode(e) => Crash::new(format!("json.decoder.JSONDecodeError: {e}")),
            ConfigError::Crash(c) => c,
        }
    }
}

impl From<Crash> for ConfigError {
    fn from(c: Crash) -> Self {
        ConfigError::Crash(c)
    }
}

/// Loads the nearest `.dev-cycle.json`; returns defaults when there is none.
pub fn load_project_config(start_dir: Option<&str>) -> Result<ProjectConfig, ConfigError> {
    let Some(manifest) = find_manifest(start_dir) else {
        return Ok(ProjectConfig::new(
            abspath(start_dir.unwrap_or("")),
            Dict::new(),
            None,
        )?);
    };
    let raw = match json::load_file(&manifest) {
        Ok(v) => v,
        Err(LoadError::Decode(e)) => return Err(ConfigError::Decode(e.to_string())),
        Err(LoadError::Crash(c)) => return Err(ConfigError::Crash(c)),
    };
    let Value::Dict(raw) = raw else {
        return Err(ConfigError::Invalid(format!(
            "{} must contain a JSON object",
            manifest.display()
        )));
    };
    let root = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    Ok(ProjectConfig::new(root, raw, Some(manifest))?)
}

fn on_off(v: &Value) -> &'static str {
    if v.truthy() { "on" } else { "off" }
}

fn or_text(v: &Value, default: &str) -> String {
    if v.truthy() {
        v.py_str()
    } else {
        default.to_string()
    }
}

/// `python project_config.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "project_config.py",
        "Show, validate or migrate the project's .dev-cycle.json",
    )
    .option("--dir", "Project directory (default: current directory)")
    .flag("--json", "Print the effective configuration as JSON")
    .flag(
        "--validate",
        "Exit with code 1 when the configuration has problems",
    )
    .flag(
        "--migrate",
        "Print the file with legacy flat keys moved into sections",
    )
    .flag(
        "--write",
        "With --migrate: write the migrated file in place",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };

    let cfg = match load_project_config(args.value("dir")) {
        Ok(c) => c,
        Err(ConfigError::Invalid(e) | ConfigError::Decode(e)) => {
            println(&format!("❌ {e}"));
            return Ok(1);
        }
        Err(ConfigError::Crash(c)) => return Err(c),
    };

    if args.flag("migrate") {
        let Some(manifest) = &cfg.manifest_path else {
            println(&format!("❌ No {MANIFEST_NAME} found."));
            return Ok(1);
        };
        let text = json::dumps(&Value::Dict(cfg.migrated()?), Some(2))? + "\n";
        if args.flag("write") {
            crate::py::write_text(manifest, &text)?;
            println(&format!("✅ Migrated {}", manifest.display()));
        } else {
            crate::out::out(&text);
        }
        return Ok(0);
    }

    if args.flag("json") {
        println(&json::dumps(&Value::Dict(cfg.data.clone()), Some(2))?);
    } else {
        let source = match &cfg.manifest_path {
            Some(p) => p.display().to_string(),
            None => format!("defaults (no {MANIFEST_NAME})"),
        };
        println(&format!(
            "⚙️  Project configuration: {}",
            cfg.project_name()
        ));
        println(&format!("   Source: {source}"));
        let arch = cfg.section("architecture");
        let devops = cfg.section("devops");
        println(&format!(
            "   Architecture style : {}",
            or_text(arch.get_or_none("style"), "not set")
        ));
        println(&format!(
            "   Core path          : {}",
            or_text(arch.get_or_none("core_path"), "not set")
        ));
        let commands = join(", ", cfg.get("test_commands"))?;
        println(&format!(
            "   Test commands      : {}",
            if commands.is_empty() {
                "auto-detect"
            } else {
                &commands
            }
        ));
        println(&format!(
            "   Branching          : {} (release candidates: {})",
            branch_summary(devops),
            on_off(devops.get_or_none("release_candidates"))
        ));
        println(&format!(
            "   Releases repo      : {}",
            or_text(devops.get_or_none("releases_repo"), "not set")
        ));
        let backlog = cfg.section("backlog");
        let blocking = backlog.get_or_none("blocking_severities");
        let blocking = if blocking.truthy() {
            join(", ", blocking)?
        } else {
            String::new()
        };
        println(&format!(
            "   Backlog            : {} (bugs first: {}; other bugs by: {})",
            backlog.get_or_none("file").py_str(),
            if blocking.is_empty() {
                "off"
            } else {
                &blocking
            },
            backlog.get_or_none("bug_deadline").py_str()
        ));
        let speckit = cfg.section("speckit");
        let speckit_line = if speckit.get_or_none("enabled").truthy() {
            format!(
                "{} ({}, branches {}/NNN-slug)",
                or_text(speckit.get_or_none("version"), "version not set"),
                speckit.get_or_none("integration").py_str(),
                speckit.get_or_none("branch_prefix").py_str()
            )
        } else {
            "off".into()
        };
        println(&format!("   Spec Kit           : {speckit_line}"));
        let gedo = cfg.section("gedo");
        let gedo_line = if gedo.get_or_none("enabled").truthy() {
            format!(
                "on (feedback {}: {})",
                on_off(gedo.get_or_none("feedback")),
                gedo.get_or_none("feedback_dir").py_str()
            )
        } else {
            "off".into()
        };
        println(&format!("   Gedo               : {gedo_line}"));
        if !cfg.legacy_keys_used.is_empty() {
            println(&format!(
                "   ℹ️  Legacy keys in use: {} (run with --migrate --write)",
                cfg.legacy_keys_used.join(", ")
            ));
        }
    }

    let problems = cfg.validate();
    for p in &problems {
        println(&format!("⚠️  {p}"));
    }
    if args.flag("validate")
        && problems
            .iter()
            .any(|p| !p.starts_with("unknown top-level key"))
    {
        return Ok(1);
    }
    Ok(0)
}
