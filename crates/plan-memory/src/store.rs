//! `_store.py` and `_common.py`: plans live as Markdown files with YAML frontmatter in the
//! project repository (default: `<repo>/docs/plans/<plan_id>.md`). The files are the source
//! of truth. The values stay as the files hold them, so a field of an unexpected type fails
//! where the scripts failed.

use architecture_docs::docs::{
    get, join_value, listdir, no_attribute, repo_root, split_frontmatter,
};
use kernel::out::err;
use kernel::py::json::{self, LoadError};
use kernel::py::path::{basename, isdir, isfile, join, makedirs, relpath};
use kernel::py::unicode::{is_word, lower};
use kernel::py::yaml::{self, DumpOptions};
use kernel::py::{Crash, Dict, Value, iterate, read_text, write_text};

pub const KINDS: [&str; 2] = ["feature", "bug"];
const FIELD_ORDER: [&str; 18] = [
    "plan_id",
    "project",
    "title",
    "kind",
    "version",
    "status",
    "author",
    "created_at",
    "updated_at",
    "session_id",
    "tags",
    "supersedes",
    "superseded_by",
    "parent_plan_id",
    "backlog_items",
    "spec_dir",
    "milestones",
    "status_log",
];
/// Spec Kit artifacts of a plan's spec folder (`spec_dir`), in reading order.
pub const SPEC_FILES: [&str; 6] = [
    "spec.md",
    "plan.md",
    "research.md",
    "data-model.md",
    "quickstart.md",
    "tasks.md",
];

/// `normalize_project(name)`: kebab-case, `AnyContext` -> `any-context`.
pub fn normalize_project(name: &Value) -> Result<String, Crash> {
    let Value::Str(name) = name else {
        return Err(no_attribute(name, "strip"));
    };
    // `re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "-", name.strip())`
    let mut split = String::new();
    let mut prev: Option<char> = None;
    for c in kernel::py::num::strip(name).chars() {
        if prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
            && c.is_ascii_uppercase()
        {
            split.push('-');
        }
        split.push(c);
        prev = Some(c);
    }
    let is_space = |c: char| kernel::py::num::is_space(c);
    // `re.sub(r"[^\w\s-]", "", text)`, then `re.sub(r"[\s_-]+", "-", text)`.
    let kept: String = lower(&split)
        .chars()
        .filter(|&c| is_word(c) || is_space(c) || c == '-')
        .collect();
    let mut out = String::new();
    let mut in_run = false;
    for c in kept.chars() {
        if is_space(c) || c == '_' || c == '-' {
            if !in_run {
                out.push('-');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    Ok(out.trim_matches('-').to_string())
}

/// `load_config(root)`: the `.dev-cycle.json` as it is, or `{}` with a warning when it
/// cannot be read.
fn load_config(root: &str) -> Value {
    let path = join(root, ".dev-cycle.json");
    if !isfile(&path) {
        return Value::Dict(Dict::new());
    }
    let error = match json::load_file(std::path::Path::new(&path)) {
        Ok(v) => return v,
        Err(LoadError::Decode(e)) => e.to_string(),
        // `str(e)`: the message without the exception's name.
        Err(LoadError::Crash(c)) => {
            c.0.split_once(": ")
                .map_or(c.0.clone(), |(_, m)| m.to_string())
        }
    };
    err(&format!("⚠️ Could not read '{path}': {error}\n"));
    Value::Dict(Dict::new())
}

/// `x or default`.
fn or<'a>(v: &'a Value, default: &'a Value) -> &'a Value {
    if v.truthy() { v } else { default }
}

/// Plans of one repository.
pub struct PlanStore {
    pub root: String,
    pub config: Value,
    pub plans_dir: String,
    pub project: String,
}

impl PlanStore {
    pub fn new(start_dir: Option<&str>, project: Option<&str>) -> Result<Self, Crash> {
        let root = repo_root(start_dir);
        let config = load_config(&root);
        let empty = Value::Dict(Dict::new());
        let plans = or(get(&config, "plans")?, &empty);
        let default_dir = Value::Str(join("docs", "plans"));
        let plans_rel = or(
            or(get(plans, "dir")?, get(&config, "plans_dir")?),
            &default_dir,
        );
        let plans_dir = join_value(&root, plans_rel)?;
        let base = Value::Str(basename(std::path::Path::new(&root)));
        let project = match project.filter(|p| !p.is_empty()) {
            Some(p) => normalize_project(&Value::str(p))?,
            None => normalize_project(or(get(&config, "project_name")?, &base))?,
        };
        Ok(Self {
            root,
            config,
            plans_dir,
            project,
        })
    }

    pub fn path_for(&self, plan_id: &str) -> String {
        join(&self.plans_dir, &format!("{plan_id}.md"))
    }

    /// `get(plan_id)`: the plan, or `None` when it has no file.
    pub fn get(&self, plan_id: &str) -> Result<Option<Plan>, Crash> {
        let path = self.path_for(plan_id);
        if !isfile(&path) {
            return Ok(None);
        }
        let name = basename(std::path::Path::new(&path));
        read_plan(&path, &name).map(Some)
    }

    /// `latest()`: active and in-progress plans first, then the most recently created.
    pub fn latest(&self) -> Result<Option<Plan>, Crash> {
        Ok(Self::latest_of(self.load_all()?))
    }

    /// `latest(plans)`.
    pub fn latest_of(plans: Vec<Plan>) -> Option<Plan> {
        let active = [Value::str("active"), Value::str("in_progress")];
        let key = |p: &Plan| {
            let status = p.meta.get_or_none("status");
            let created = p.meta.get_or_none("created_at");
            (
                active.iter().any(|a| status.py_eq(a)),
                if created.truthy() {
                    created.py_str()
                } else {
                    String::new()
                },
            )
        };
        // `sorted(..., reverse=True)[0]`: the first plan with the greatest key.
        let mut best: Option<(Plan, (bool, String))> = None;
        for p in plans {
            let k = key(&p);
            if best.as_ref().is_none_or(|(_, b)| k > *b) {
                best = Some((p, k));
            }
        }
        best.map(|(p, _)| p)
    }

    /// `spec_files(plan)`: (relative path, text) of the Spec Kit artifacts in the plan's
    /// `spec_dir`, in reading order.
    pub fn spec_files(&self, plan: &Plan) -> Result<Vec<(String, String)>, Crash> {
        let Some(spec_dir) = plan.spec_dir().filter(|d| !d.is_empty()) else {
            return Ok(Vec::new());
        };
        let folder = join(&self.root, &spec_dir);
        if !isdir(&folder) {
            return Ok(Vec::new());
        }
        let mut paths: Vec<String> = SPEC_FILES.iter().map(|n| join(&folder, n)).collect();
        let contracts = join(&folder, "contracts");
        if isdir(&contracts) {
            for name in listdir(&contracts)? {
                if name.ends_with(".md") {
                    paths.push(join(&contracts, &name));
                }
            }
        }
        let mut out = Vec::new();
        for path in paths {
            if isfile(&path) {
                let text = read_text(std::path::Path::new(&path))?;
                out.push((relpath(&path, &self.root)?.replace('\\', "/"), text));
            }
        }
        Ok(out)
    }

    /// `full_text(plan)`: the plan body followed by its Spec Kit artifacts.
    pub fn full_text(&self, plan: &Plan) -> Result<String, Crash> {
        let mut text = plan.body.clone();
        for (rel, artifact) in self.spec_files(plan)? {
            text.push_str(&format!(
                "\n\n<!-- Spec Kit artifact: {rel} -->\n\n{artifact}"
            ));
        }
        Ok(text)
    }

    /// `save(plan)`: writes the plan to the file of its `plan_id`.
    pub fn save(&self, plan: &mut Plan) -> Result<(), Crash> {
        makedirs(&self.plans_dir)?;
        let id = plan.meta.get_or_none("plan_id").py_str();
        plan.path = self.path_for(&id);
        write_plan(plan)
    }

    /// `author()`: the repository's `git config user.name`, or `None`.
    pub fn author(&self) -> Value {
        let out = std::process::Command::new("git")
            .args(["config", "user.name"])
            .current_dir(&self.root)
            .output();
        match out {
            Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
                Ok(text) => {
                    let name =
                        kernel::py::num::strip(&kernel::py::universal_newlines(&text)).to_string();
                    if name.is_empty() {
                        Value::Null
                    } else {
                        Value::Str(name)
                    }
                }
                Err(_) => Value::Null,
            },
            _ => Value::Null,
        }
    }

    /// `load_all()`: the `.md` files of the plans folder, by name.
    pub fn load_all(&self) -> Result<Vec<Plan>, Crash> {
        if !isdir(&self.plans_dir) {
            return Ok(Vec::new());
        }
        let mut plans = Vec::new();
        for name in listdir(&self.plans_dir)? {
            if name.ends_with(".md") {
                plans.push(read_plan(&join(&self.plans_dir, &name), &name)?);
            }
        }
        Ok(plans)
    }
}

pub struct Plan {
    pub meta: Dict,
    pub body: String,
    pub path: String,
}

impl Plan {
    /// Every milestone is a dict, as `read_plan` leaves them.
    pub fn milestones(&self) -> Vec<Value> {
        match self.meta.get("milestones") {
            Some(Value::List(items)) => items.clone(),
            _ => Vec::new(),
        }
    }

    pub fn kind(&self) -> Value {
        or(self.meta.get_or_none("kind"), &Value::str("feature")).clone()
    }

    pub fn backlog_items(&self) -> Result<Vec<String>, Crash> {
        let items = self.meta.get_or_none("backlog_items");
        if !items.truthy() {
            return Ok(Vec::new());
        }
        Ok(iterate(items)?.iter().map(Value::py_str).collect())
    }

    /// `spec_dir`: `str(value).strip().rstrip("/")`, or `None` when unset.
    pub fn spec_dir(&self) -> Option<String> {
        let value = self.meta.get_or_none("spec_dir");
        value.truthy().then(|| {
            kernel::py::num::strip(&value.py_str())
                .trim_end_matches('/')
                .to_string()
        })
    }

    /// `done_milestones`: the names of the milestones marked done.
    pub fn done_milestones(&self) -> Result<Vec<Value>, Crash> {
        let mut out = Vec::new();
        for m in self.milestones() {
            let Value::Dict(d) = &m else { continue };
            if d.get_or_none("done").truthy() {
                out.push(name_of(d)?);
            }
        }
        Ok(out)
    }
}

impl Plan {
    /// `add_log(by, note)`: appends a `status_log` entry stamped with the local time.
    pub fn add_log(&mut self, by: Value, note: &str) -> Result<(), Crash> {
        let mut entry = Dict::new();
        entry.insert("at", Value::Str(kernel::py::now_iso()));
        entry.insert("by", by);
        entry.insert("note", Value::str(note));
        let log = match self.meta.get("status_log") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::List(items)) => items.clone(),
            Some(other) => return Err(no_attribute(other, "append")),
        };
        let mut log = log;
        log.push(Value::Dict(entry));
        self.meta.insert("status_log", Value::List(log));
        Ok(())
    }
}

/// `write_plan(plan)`: the frontmatter in `FIELD_ORDER`, then the body.
fn write_plan(plan: &Plan) -> Result<(), Crash> {
    let mut ordered = Dict::new();
    for k in FIELD_ORDER {
        if let Some(v) = plan.meta.get(k) {
            ordered.insert(k, v.clone());
        }
    }
    for (k, v) in plan.meta.iter() {
        if !k.as_str().is_some_and(|k| FIELD_ORDER.contains(&k)) {
            ordered.push_new(k.clone(), v.clone());
        }
    }
    let options = DumpOptions {
        default_flow_style: Some(false),
        allow_unicode: true,
        width: Some(4096),
    };
    let front = yaml::safe_dump(&Value::Dict(ordered), &options)?;
    let text = format!(
        "---\n{front}---\n\n{}\n",
        kernel::py::num::strip(&plan.body)
    );
    write_text(std::path::Path::new(&plan.path), &text)
}

/// `m["name"]`.
pub fn name_of(d: &Dict) -> Result<Value, Crash> {
    d.get("name")
        .cloned()
        .ok_or_else(|| Crash::new("KeyError: 'name'"))
}

/// `os.path.splitext(name)[0]` for a name ending in `.md`.
fn plan_id_of(name: &str) -> String {
    let stem = &name[..name.len() - 3];
    if stem.chars().all(|c| c == '.') {
        name.to_string()
    } else {
        stem.to_string()
    }
}

/// `read_plan(path)`.
pub fn read_plan(path: &str, name: &str) -> Result<Plan, Crash> {
    let raw = read_text(std::path::Path::new(path))?;
    let (meta, body, _) = split_frontmatter(&raw)?;
    let Value::Dict(mut meta) = meta else {
        return Err(no_attribute(&meta, "setdefault"));
    };
    if !meta.contains_key("plan_id") {
        meta.insert("plan_id", Value::str(plan_id_of(name)));
    }
    if !meta.contains_key("status") {
        meta.insert("status", Value::str("active"));
    }
    let listed = meta.get_or_none("milestones");
    let mut milestones = Vec::new();
    if listed.truthy() {
        for m in iterate(listed)? {
            milestones.push(match m {
                Value::Dict(_) => m,
                other => {
                    let mut d = Dict::new();
                    d.insert("name", Value::str(other.py_str()));
                    d.insert("done", Value::Bool(false));
                    Value::Dict(d)
                }
            });
        }
    }
    meta.insert("milestones", Value::List(milestones));
    Ok(Plan {
        meta,
        body,
        path: path.to_string(),
    })
}

/// `progress_bar(done, total)`.
pub fn progress_bar(done: usize, total: usize) -> String {
    let width = 16;
    let ratio = if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    };
    let filled = (width as f64 * ratio).round_ties_even() as usize;
    format!(
        "[{}{}] {:.1}% ({done}/{total} Milestones)",
        "█".repeat(filled),
        "░".repeat(width - filled),
        ratio * 100.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_names_like_python() {
        let n = |s: &str| normalize_project(&Value::str(s)).unwrap();
        assert_eq!(n("AnyContext"), "any-context");
        assert_eq!(n("  any_context  "), "any-context");
        assert_eq!(n("AI Code--Agent.Skills"), "ai-code-agentskills");
        assert_eq!(n("Ünïcode2Go"), "ünïcode2-go");
        assert!(normalize_project(&Value::int(3)).is_err());
    }

    #[test]
    fn progress_like_python() {
        assert_eq!(
            progress_bar(1, 32),
            "[░░░░░░░░░░░░░░░░] 3.1% (1/32 Milestones)"
        );
        assert_eq!(
            progress_bar(3, 4),
            "[████████████░░░░] 75.0% (3/4 Milestones)"
        );
        assert_eq!(plan_id_of(".md"), ".md");
        assert_eq!(plan_id_of("a.b.md"), "a.b");
    }
}
