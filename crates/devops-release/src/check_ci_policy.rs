//! `check_ci_policy.py`: checks a repository's GitHub Actions workflows against the
//! devops-release policy, for the project's branching model (`devops.branching`): tests on pull
//! requests into every branch that receives them, a linter or static analysis on the same pull
//! requests, pre-releases for -rc tags, the release gate (a tag-on-merge job, or a promote
//! workflow) able to actually trigger the release, release permissions, and existing version
//! files. Exit code 1 on errors.

use std::path::Path;
use std::sync::LazyLock;

use kernel::out::println;
use kernel::project_config::load_project_config;
use kernel::py::argparse::Parser;
use kernel::py::fnmatch::{fnmatch, glob_in};
use kernel::py::path::{basename, isfile, join};
use kernel::py::unicode::{SPACE, word};
use kernel::py::{Crash, Dict, Value, iterate, read_text, yaml};
use regex::Regex;

use crate::branching::BranchingModel;

/// A pattern with Python's `\s`, `\S` and `\w`; `{NW}` is a character that is not `\w`.
/// Every check only asks whether the pattern is found anywhere, so `\b` is written as the
/// non-word character (or the end) on its side.
fn pattern(p: &str) -> Regex {
    let p = p
        .replace(r"\S", &format!("[^{SPACE}]"))
        .replace(r"\s", &format!("[{SPACE}]"))
        .replace(r"\w", &format!("[{}]", word()))
        .replace("{NW}", &format!("[^{}]", word()))
        .replace("{NWNL}", &format!("[^{}\\n]", word()));
    Regex::new(&p).expect("valid pattern")
}

static RELEASE_ACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(r"softprops/action-gh-release|ncipollo/release-action|gh\s+release\s+create")
});
static TEST_STEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(concat!(
        r"(?:^|{NW})(cargo\s+test|pytest|npm\s+(run\s+)?test|pnpm\s+(run\s+)?test|yarn\s+test|",
        r"go\s+test|dotnet\s+test|mvn\s+test|gradle\w*\s+test|make\s+test|tox)(?:{NW}|$)"
    ))
});
static LINT_STEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    // `cargo\s+fmt\b[^\n]*--check`: after `fmt`, either `--check` at once or a non-word
    // character that `[^\n]*` starts with.
    pattern(concat!(
        r"(?:^|{NW})(clippy|cargo\s+fmt(?:{NWNL}[^\n]*)?--check|ruff|flake8|pylint|mypy|",
        r"black\s+--check|eslint|biome|prettier\s+--check|golangci-lint|go\s+vet|gofmt|",
        r"staticcheck|rubocop|ktlint|detekt|dotnet\s+format|shellcheck|codeql|",
        r"(npm|pnpm|yarn)\s+(run\s+)?lint|make\s+lint)(?:{NW}|$)"
    ))
});
static TAG_PUSH_RE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(concat!(
        r"git\s+tag(?:{NW}|$)[\s\S]*?git\s+push(?:{NW}|$)|git\s+push\s+origin\s+\S*v\$|",
        r"gh\s+release\s+create"
    ))
});
static REUSABLE_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(?m)^\s*uses:\s*\./\.github/workflows/"));
static PRERELEASE_RE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(?im)^\s*prerelease\s*:|--prerelease"));
static RC_TEST_RE: LazyLock<Regex> = LazyLock::new(|| pattern(r"-rc(?:{NW}|$)|(?:^|{NW})rc\."));
static CONTENTS_WRITE_RE: LazyLock<Regex> = LazyLock::new(|| pattern(r"contents\s*:\s*write"));
static ENVIRONMENT_RE: LazyLock<Regex> = LazyLock::new(|| pattern(r"(?m)^\s*environment\s*:"));

/// `CUSTOM_SECRET_RE = secrets\.(?!GITHUB_TOKEN\b)[A-Z0-9_]+` is found in `text`.
fn uses_own_token(text: &str) -> bool {
    let secret_char = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_';
    text.match_indices("secrets.").any(|(i, m)| {
        let rest = &text[i + m.len()..];
        if !rest.starts_with(secret_char) {
            return false;
        }
        match rest.strip_prefix("GITHUB_TOKEN") {
            Some(after) => after.chars().next().is_some_and(kernel::py::is_word),
            None => true,
        }
    })
}

/// `key in container`.
fn contains(container: &Value, key: &str) -> Result<bool, Crash> {
    let key = Value::str(key);
    match container {
        Value::Dict(d) => Ok(d.get_value(&key).is_some()),
        Value::Set(items) | Value::List(items) | Value::Tuple(items) => {
            Ok(items.iter().any(|i| i.py_eq(&key)))
        }
        Value::Str(s) => Ok(s.contains(key.as_str().unwrap_or_default())),
        Value::Bytes(_) => Err(Crash::new(
            "TypeError: a bytes-like object is required, not 'str'",
        )),
        other => Err(Crash::new(format!(
            "TypeError: argument of type '{}' is not iterable",
            other.tp_name()
        ))),
    }
}

fn no_attribute(value: &Value, name: &str) -> Crash {
    Crash::new(format!(
        "AttributeError: '{}' object has no attribute '{name}'",
        value.tp_name()
    ))
}

pub struct Workflow {
    pub name: String,
    pub text: String,
    /// What the steps run, without the comments (a template names linters in its comments).
    pub commands: String,
    pub triggers: Value,
}

/// A workflow file that is not valid YAML: `str(yaml.YAMLError)`.
pub struct BadYaml(pub String);

impl Workflow {
    fn load(path: &str) -> Result<Result<Self, BadYaml>, Crash> {
        let text = read_text(Path::new(path))?;
        let data = match yaml::safe_load(&text) {
            Ok(v) => v,
            Err(Crash(msg)) => {
                return match msg.strip_prefix("yaml.").and_then(|m| m.split_once(": ")) {
                    Some((_, e)) => Ok(Err(BadYaml(e.to_string()))),
                    None => Err(Crash(msg)),
                };
            }
        };
        let data = if data.truthy() {
            data
        } else {
            Value::Dict(Dict::new())
        };
        let jobs = match &data {
            Value::Dict(d) => d.get_or_none("jobs").clone(),
            _ => Value::Null,
        };
        let mut steps: Vec<Dict> = Vec::new();
        if jobs.truthy() {
            let Value::Dict(jobs) = &jobs else {
                return Err(no_attribute(&jobs, "values"));
            };
            for (_, job) in jobs.iter() {
                let Value::Dict(job) = job else { continue };
                let job_steps = job.get_or_none("steps");
                if !job_steps.truthy() {
                    continue;
                }
                for step in iterate(job_steps)? {
                    if let Value::Dict(step) = step {
                        steps.push(step);
                    }
                }
            }
        }
        let mut commands: Vec<String> = Vec::new();
        for step in &steps {
            for k in ["run", "uses"] {
                let v = step.get_or_none(k);
                commands.push(if v.truthy() {
                    v.py_str()
                } else {
                    String::new()
                });
            }
        }

        // PyYAML reads the bare key `on` as boolean True.
        let raw = match &data {
            Value::Dict(d) => match d.get("on") {
                Some(v) => v.clone(),
                None => d
                    .get_value(&Value::Bool(true))
                    .cloned()
                    .unwrap_or(Value::Dict(Dict::new())),
            },
            _ => Value::Dict(Dict::new()),
        };
        let raw = match raw {
            Value::Str(s) => {
                let mut d = Dict::new();
                d.insert(s, Value::Null);
                Value::Dict(d)
            }
            Value::List(items) => {
                let mut d = Dict::new();
                for k in items {
                    if !k.is_hashable() {
                        return Err(Crash::new(format!(
                            "TypeError: unhashable type: '{}'",
                            k.type_name()
                        )));
                    }
                    d.insert_value(k, Value::Null);
                }
                Value::Dict(d)
            }
            other => other,
        };
        let triggers = if raw.truthy() {
            raw
        } else {
            Value::Dict(Dict::new())
        };
        Ok(Ok(Self {
            name: basename(Path::new(path)),
            text,
            commands: commands.join("\n"),
            triggers,
        }))
    }

    /// `None` when the event is absent; `[]` when present without a filter (all); else the
    /// list.
    fn filter(&self, event: &str, key: &str) -> Result<Option<Vec<Value>>, Crash> {
        if !contains(&self.triggers, event)? {
            return Ok(None);
        }
        let Value::Dict(triggers) = &self.triggers else {
            return Err(no_attribute(&self.triggers, "get"));
        };
        let conf = triggers.get(event).cloned().unwrap_or(Value::Null);
        let values = match &conf {
            Value::Dict(c) if conf.truthy() => c.get_or_none(key).clone(),
            _ => Value::Null,
        };
        Ok(Some(match values {
            Value::Null => Vec::new(),
            Value::Str(_) => vec![values],
            other => iterate(&other)?,
        }))
    }

    fn any_match(branch: &str, patterns: &[Value]) -> Result<bool, Crash> {
        for p in patterns {
            if fnmatch(branch, p)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn pr_covers(&self, branch: &str) -> Result<bool, Crash> {
        for event in ["pull_request", "pull_request_target"] {
            if let Some(branches) = self.filter(event, "branches")?
                && (branches.is_empty() || Self::any_match(branch, &branches)?)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn push_covers_branch(&self, branch: &str) -> Result<bool, Crash> {
        match self.filter("push", "branches")? {
            Some(branches) if !branches.is_empty() => Self::any_match(branch, &branches),
            _ => Ok(false),
        }
    }

    pub fn push_tags(&self) -> Result<bool, Crash> {
        Ok(self
            .filter("push", "tags")?
            .is_some_and(|tags| !tags.is_empty()))
    }

    pub fn runs_tests(&self) -> bool {
        TEST_STEP_RE.is_match(&self.text)
    }

    pub fn runs_lint(&self) -> bool {
        LINT_STEP_RE.is_match(&self.commands)
    }

    pub fn publishes_release(&self) -> bool {
        RELEASE_ACTION_RE.is_match(&self.text)
    }
}

/// The workflow files, sorted: each one loaded, or its YAML error.
fn load_workflows(root: &str) -> Result<Vec<Result<Workflow, BadYaml>>, Crash> {
    let dir = join(&join(root, ".github"), "workflows");
    let mut paths = glob_in(&dir, "*.yml");
    paths.extend(glob_in(&dir, "*.yaml"));
    paths.sort();
    paths.iter().map(|p| Workflow::load(p)).collect()
}

/// `target.replace("*", "0.0")`.
fn sample(target: &Value) -> Result<String, Crash> {
    match target {
        Value::Str(s) => Ok(s.replace('*', "0.0")),
        other => Err(no_attribute(other, "replace")),
    }
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

/// Returns (errors, warnings).
pub fn run_checks(project_dir: Option<&str>) -> Result<(Vec<String>, Vec<String>), Crash> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let cfg = match load_project_config(project_dir) {
        Ok(c) => c,
        Err(e) => return Err(e.into_crash()),
    };
    let devops = cfg.section("devops");
    let model = match BranchingModel::from_devops(devops) {
        Ok(m) => m,
        Err(e) => return Ok((vec![e], warnings)),
    };
    let production = &model.production;
    let use_rc = devops.get("release_candidates").is_none_or(Value::truthy);
    let root = cfg.root.to_string_lossy().into_owned();

    let version_files = devops.get_or_none("version_files");
    if version_files.truthy() {
        for rel in iterate(version_files)? {
            if !isfile(&join_root(&root, &rel)?) {
                errors.push(format!(
                    "devops.version_files: '{}' does not exist",
                    rel.py_str()
                ));
            }
        }
    } else {
        warnings.push(
            "devops.version_files is empty; list the files that hold the version".to_string(),
        );
    }

    let loaded = load_workflows(&root)?;
    let mut workflows: Vec<&Workflow> = Vec::new();
    for item in &loaded {
        match item {
            Ok(w) => workflows.push(w),
            Err(BadYaml(e)) => errors.push(format!("workflow YAML could not be parsed: {e}")),
        }
    }
    if workflows.is_empty() {
        errors.push("no GitHub Actions workflows in .github/workflows".to_string());
        return Ok((errors, warnings));
    }

    let tests: Vec<&Workflow> = workflows
        .iter()
        .copied()
        .filter(|w| w.runs_tests())
        .collect();
    let targets = model.pr_targets();
    for target in &targets {
        // release/* is checked with a branch it must cover.
        let sample = sample(target)?;
        let mut tested = false;
        for w in &tests {
            if w.pr_covers(&sample)? {
                tested = true;
                break;
            }
        }
        let target = target.py_str();
        if !tested {
            errors.push(format!(
                "no test workflow runs on pull requests into '{target}' (see templates/ci.yml)"
            ));
        }
        let mut linted = false;
        for w in &workflows {
            if w.runs_lint() && w.pr_covers(&sample)? {
                linted = true;
                break;
            }
        }
        if !linted {
            warnings.push(format!(
                "no workflow runs a linter or static analysis on pull requests into '{target}' \
                 (see the lint job of templates/ci.yml)"
            ));
        }
    }

    let releases: Vec<&Workflow> = workflows
        .iter()
        .copied()
        .filter(|w| w.publishes_release())
        .collect();
    if releases.is_empty() {
        warnings.push("no workflow publishes releases (see templates/release.yml)".to_string());
    }
    for w in &releases {
        if !CONTENTS_WRITE_RE.is_match(&w.text) {
            errors.push(format!(
                "{}: publishing a release needs 'permissions: contents: write'",
                w.name
            ));
        }
        if use_rc
            && (w.push_tags()? || contains(&w.triggers, "workflow_call")?)
            && !(PRERELEASE_RE.is_match(&w.text) && RC_TEST_RE.is_match(&w.text))
        {
            errors.push(format!(
                "{}: '-rc' tags must publish pre-releases (prerelease: ${{{{ contains(<tag>, '-rc') }}}})",
                w.name
            ));
        }
    }

    let merged: Vec<String> = targets.iter().map(sample).collect::<Result<_, _>>()?;
    let mut taggers: Vec<&Workflow> = Vec::new();
    for w in &workflows {
        if !TAG_PUSH_RE.is_match(&w.text) {
            continue;
        }
        for b in &merged {
            if w.push_covers_branch(b)? {
                taggers.push(w);
                break;
            }
        }
    }
    let production = production.as_str().unwrap_or_default();
    let mut gate: Vec<&Workflow> = Vec::new();
    if model.gate() == "merge" {
        for w in &taggers {
            if w.push_covers_branch(production)? {
                gate.push(w);
            }
        }
        if gate.is_empty() {
            warnings.push(format!(
                "no workflow creates the final tag when '{production}' receives the merge (see templates/tag-on-merge.yml)"
            ));
        }
    } else {
        for w in &taggers {
            errors.push(format!(
                "{}: creates a tag on every merge, but under '{}' the release gate is the user's \
                 promotion of an accepted candidate (see templates/promote.yml)",
                w.name, model.name
            ));
        }
        for w in &workflows {
            if contains(&w.triggers, "workflow_dispatch")? && TAG_PUSH_RE.is_match(&w.text) {
                gate.push(w);
            }
        }
        if gate.is_empty() {
            warnings.push(format!(
                "no promote workflow releases an accepted candidate under '{}' (see templates/promote.yml)",
                model.name
            ));
        }
        for w in &gate {
            if !ENVIRONMENT_RE.is_match(&w.text) {
                warnings.push(format!(
                    "{}: no 'environment:' with required reviewers holds the promotion for the user's approval",
                    w.name
                ));
            }
        }
    }
    for w in &gate {
        let calls_release = REUSABLE_CALL_RE.is_match(&w.text) || w.publishes_release();
        if !calls_release && !uses_own_token(&w.text) {
            errors.push(format!(
                "{}: pushes a tag with GITHUB_TOKEN, which does not trigger the release workflow; \
                 call the release as a reusable workflow or push with a dedicated token",
                w.name
            ));
        }
    }
    Ok((errors, warnings))
}

/// `python check_ci_policy.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "check_ci_policy.py",
        "Check GitHub Actions workflows against the devops-release policy",
    )
    .option("--dir", "Project directory (default: current directory)");
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
    println(&format!(
        "\n{} devops-release CI policy: {} error(s), {} warning(s).",
        if errors.is_empty() { "✅" } else { "❌" },
        errors.len(),
        warnings.len()
    ));
    Ok(u8::from(!errors.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_like_python() {
        assert!(TEST_STEP_RE.is_match("run: cargo test --all"));
        assert!(!TEST_STEP_RE.is_match("run: xpytest"));
        assert!(TEST_STEP_RE.is_match("gradlew test"));
        assert!(LINT_STEP_RE.is_match("cargo fmt --all -- --check"));
        assert!(LINT_STEP_RE.is_match("cargo fmt--check"));
        assert!(!LINT_STEP_RE.is_match("cargo fmtx --check"));
        assert!(!LINT_STEP_RE.is_match("cargo fmt\n--check"));
        assert!(TAG_PUSH_RE.is_match("git tag v1\n git push origin v1"));
        assert!(!TAG_PUSH_RE.is_match("git tagger; git push"));
        assert!(RC_TEST_RE.is_match("contains(tag, '-rc')"));
        assert!(!RC_TEST_RE.is_match("-rcx"));
        assert!(PRERELEASE_RE.is_match("  PreRelease: true"));
        assert!(uses_own_token("${{ secrets.RELEASE_TOKEN }}"));
        assert!(!uses_own_token("${{ secrets.GITHUB_TOKEN }}"));
        assert!(uses_own_token("${{ secrets.GITHUB_TOKENS }}"));
        assert!(!uses_own_token("${{ secrets.lower }}"));
    }
}
