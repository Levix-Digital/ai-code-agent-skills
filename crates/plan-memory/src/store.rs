//! `_store.py` and `_common.py`: plans live as Markdown files with YAML frontmatter in the
//! project repository (default: `<repo>/docs/plans/<plan_id>.md`). The files are the source
//! of truth. The values stay as the files hold them, so a field of an unexpected type fails
//! where the scripts failed.

use architecture_docs::docs::{
    get, join_value, listdir, no_attribute, repo_root, split_frontmatter,
};
use kernel::out::err;
use kernel::py::json::{self, LoadError};
use kernel::py::path::{basename, isdir, isfile, join};
use kernel::py::unicode::{is_word, lower};
use kernel::py::{Crash, Dict, Value, iterate, read_text};

pub const KINDS: [&str; 2] = ["feature", "bug"];

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
