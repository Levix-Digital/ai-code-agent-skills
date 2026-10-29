//! `branching.py`'s `BranchingModel`: the branching model a project chose (`devops.branching`)
//! and the branches it names. The values stay as configured, as Python keeps them, so a
//! non-string branch fails where the script failed.

use kernel::project_config::BRANCHING_MODELS;
use kernel::py::{Dict, Value};

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
}
