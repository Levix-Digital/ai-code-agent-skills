//! `branching.py`: the branching model a project chose (`devops.branching`) and what it means:
//! where work branches start, where pull requests go, the branch a release candidate is tagged
//! on, how a hotfix travels and which action of the user releases. The values stay as
//! configured, as Python keeps them, so a non-string branch fails where the script failed.

use std::sync::LazyLock;

use kernel::out::println;
use kernel::project_config::{BRANCHING_MODELS, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::unicode::digits;
use kernel::py::{Crash, Dict, Value, join, json};
use regex::Regex;

use crate::release_status::project_version;

static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    let d = format!("[{}]", digits());
    Regex::new(&format!(r"^v?({d}+)\.({d}+)\.({d}+)")).expect("valid pattern")
});

pub struct BranchingModel {
    pub name: String,
    pub production: Value,
    /// Where work branches start and pull requests go.
    pub integration: Value,
    pub release_prefix: Value,
}

/// `d.get(key) or default`.
fn or(devops: &Dict, key: &str, default: &str) -> Value {
    match devops.get(key) {
        Some(v) if v.truthy() => v.clone(),
        _ => Value::str(default),
    }
}

impl BranchingModel {
    /// `BranchingModel.from_devops(devops)`: `Err` holds the `ValueError` message.
    pub fn from_devops(devops: &Dict) -> Result<Self, String> {
        let name = or(devops, "branching", "integration");
        if !name.is_one_of(&BRANCHING_MODELS) {
            return Err(format!(
                "devops.branching '{}' is not one of: {}",
                name.py_str(),
                BRANCHING_MODELS.join(", ")
            ));
        }
        let name = name.py_str();
        let production = or(devops, "production_branch", "main");
        let integration = if name == "integration" {
            or(devops, "integration_branch", "dev")
        } else {
            production.clone()
        };
        Ok(Self {
            name,
            production,
            integration,
            release_prefix: or(devops, "release_branch_prefix", "release"),
        })
    }

    /// The user's release action: merging into the production branch, or promoting an
    /// accepted candidate.
    pub fn gate(&self) -> &'static str {
        if self.name == "integration" {
            "merge"
        } else {
            "promote"
        }
    }

    /// Branches (or patterns) that receive pull requests, so CI must run on them.
    pub fn pr_targets(&self) -> Vec<Value> {
        match self.name.as_str() {
            "integration" => vec![self.integration.clone(), self.production.clone()],
            "release-branches" => vec![
                self.production.clone(),
                Value::str(format!("{}/*", self.release_prefix.py_str())),
            ],
            _ => vec![self.production.clone()],
        }
    }

    pub fn long_lived(&self) -> Vec<Value> {
        match self.name.as_str() {
            "integration" => vec![self.production.clone(), self.integration.clone()],
            "release-branches" => vec![
                self.production.clone(),
                Value::str(format!("{}/X.Y", self.release_prefix.py_str())),
            ],
            _ => vec![self.production.clone()],
        }
    }

    /// `Err` holds the `ValueError` message.
    pub fn release_branch(&self, version: &str) -> Result<String, String> {
        let Some(m) = VERSION_RE.captures(version) else {
            return Err(format!("'{version}' is not a version like 1.4.0"));
        };
        Ok(format!(
            "{}/{}.{}",
            self.release_prefix.py_str(),
            &m[1],
            &m[2]
        ))
    }

    /// The branch a candidate of `version` is tagged on.
    pub fn rc_branch(&self, version: Option<&str>) -> Result<String, String> {
        if self.name == "release-branches" {
            return self.release_branch(version.unwrap_or_default());
        }
        Ok(self.integration.py_str())
    }

    pub fn describe(&self) -> Dict {
        let (p, i, r) = (
            self.production.py_str(),
            self.integration.py_str(),
            self.release_prefix.py_str(),
        );
        let mut work = format!("feat/, fix/, docs/, ... from '{i}', pull requests into '{i}'");
        let promote = "the user runs the promote workflow for the accepted candidate; it tags that commit vX.Y.Z and releases it";
        let (rc, gate, hotfix) = match self.name.as_str() {
            "integration" => (
                format!("vX.Y.Z-rc.N tagged on '{i}' after the version bump"),
                format!(
                    "the user merges the pull request '{i}' -> '{p}'; tag-on-merge creates vX.Y.Z and the release"
                ),
                format!(
                    "hotfix/<slug> from '{p}', pull request into '{p}', then '{p}' merged back into '{i}'"
                ),
            ),
            "trunk" => (
                format!(
                    "vX.Y.Z-rc.N tagged on '{p}' after the version bump; fixes found in acceptance go into '{p}' and a new candidate"
                ),
                promote.to_string(),
                format!(
                    "fix/<slug> into '{p}', patch bump, candidate, promote; when '{p}' holds unreleased work, \
                     hotfix/X.Y.Z from the release tag, the fix by pull request into it, candidate tagged there, then the fix \
                     into '{p}' by pull request"
                ),
            ),
            _ => {
                work += &format!(
                    "; fixes of a release in acceptance or maintenance from '{r}/X.Y', pull requests into it"
                );
                (
                    format!(
                        "cut '{r}/X.Y' from '{p}' after the version bump; vX.Y.Z-rc.N tagged on '{r}/X.Y'; \
                         fixes go into '{r}/X.Y' and forward into '{p}' by pull request"
                    ),
                    promote.to_string(),
                    format!(
                        "fix/<slug> from '{r}/X.Y', pull request into it, patch bump, candidate, promote, \
                         then forward into '{p}' by pull request"
                    ),
                )
            }
        };
        let mut d = Dict::new();
        d.insert("model", Value::str(&self.name));
        d.insert("long_lived", Value::List(self.long_lived()));
        d.insert("work", Value::str(work));
        d.insert("pull_requests_into", Value::List(self.pr_targets()));
        d.insert("release_candidate", Value::str(rc));
        d.insert("gate", Value::str(self.gate()));
        d.insert("release_gate", Value::str(gate));
        d.insert("hotfix", Value::str(hotfix));
        d
    }
}

/// Integration branch names a repository may already use.
const INTEGRATION_NAMES: [&str; 2] = ["dev", "develop"];

/// `GATES.get(model)`: the user's release action of a model, `None` for an unknown one.
pub fn gate_of(model: &Value) -> Result<Value, Crash> {
    if !model.is_hashable() {
        return Err(Crash::new(format!(
            "TypeError: unhashable type: '{}'",
            model.type_name()
        )));
    }
    Ok(match model.as_str() {
        Some("integration") => Value::str("merge"),
        Some("trunk" | "release-branches") => Value::str("promote"),
        _ => Value::Null,
    })
}

/// `detect(branches, production)`: the model a repository already follows, from its branch
/// names (local or `origin/`). `{model, integration_branch, reason}`; model `None` when there
/// is nothing to go by.
pub fn detect(branches: &[String], production: &Value) -> Dict {
    let names: Vec<&str> = branches
        .iter()
        .map(|b| b.strip_prefix("origin/").unwrap_or(b))
        .collect();
    let found = |model: Option<&str>, integration: Option<&str>, reason: String| {
        let mut d = Dict::new();
        d.insert("model", model.map_or(Value::Null, Value::str));
        d.insert(
            "integration_branch",
            integration.map_or(Value::Null, Value::str),
        );
        d.insert("reason", Value::Str(reason));
        d
    };
    if names.iter().any(|n| n.starts_with("release/")) {
        return found(
            Some("release-branches"),
            None,
            "release/* branches exist".into(),
        );
    }
    for name in INTEGRATION_NAMES {
        if names.contains(&name) {
            return found(
                Some("integration"),
                Some(name),
                format!("'{name}' branch exists"),
            );
        }
    }
    if !names.is_empty() {
        return found(
            Some("trunk"),
            None,
            format!("only '{}' and short-lived branches", production.py_str()),
        );
    }
    found(None, None, "no branches yet".into())
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "branching.py",
        "The project's branching model (devops.branching)",
    )
    .positional_choice("command", &["show", "rc-branch"], "")
    .option("--dir", "Project directory (default: current directory)")
    .option(
        "--version",
        "With rc-branch: the version (default: devops.version_files)",
    )
    .flag("--json", "With show: print JSON");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let cfg = match load_project_config(args.value("dir")) {
        Ok(c) => c,
        Err(e) => return Err(e.into_crash()),
    };
    let model = match BranchingModel::from_devops(cfg.section("devops")) {
        Ok(m) => m,
        Err(e) => {
            println(&format!("❌ {e}"));
            return Ok(1);
        }
    };

    if args.value("command") == Some("rc-branch") {
        let mut version = args.value("version").map(str::to_string);
        if version.as_deref().is_none_or(str::is_empty) && model.name == "release-branches" {
            let (v, problems) = project_version(&cfg)?;
            for p in &problems {
                println(&format!("❌ {p}"));
            }
            if v.is_none() {
                return Ok(1);
            }
            version = v;
        }
        return Ok(match model.rc_branch(version.as_deref()) {
            Ok(b) => {
                println(&b);
                0
            }
            Err(e) => {
                println(&format!("❌ {e}"));
                1
            }
        });
    }

    let data = model.describe();
    if args.flag("json") {
        println(&json::dumps(&Value::Dict(data), Some(2))?);
        return Ok(0);
    }
    let get = |k: &str| data.get_or_none(k);
    let default = if cfg.is_set("devops", "branching") {
        ""
    } else {
        " (default; not set in .dev-cycle.json)"
    };
    println(&format!(
        "🌿 Branching model: {}{default}",
        get("model").py_str()
    ));
    println(&format!(
        "   Long-lived branches : {}",
        join(", ", get("long_lived"))?
    ));
    println(&format!(
        "   Work branches       : {}",
        get("work").py_str()
    ));
    println(&format!(
        "   Release candidate   : {}",
        get("release_candidate").py_str()
    ));
    println(&format!(
        "   Release gate (user) : {}",
        get("release_gate").py_str()
    ));
    println(&format!(
        "   Hotfix              : {}",
        get("hotfix").py_str()
    ));
    if model.name != "integration" && cfg.is_set("devops", "integration_branch") {
        println(&format!(
            "   ℹ️ devops.integration_branch is not used by '{}'",
            model.name
        ));
    }
    Ok(0)
}
