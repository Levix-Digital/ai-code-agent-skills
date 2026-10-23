"""
Branching - devops-release Skill
The branching models a project can choose (`devops.branching` in .dev-cycle.json) and what
each one means: where work branches start, where pull requests go, the branch a release
candidate is tagged on, how a hotfix travels and which action of the user releases. The
orchestrator, the CI policy checker and the onboarding read the model from here.

Usage:
    python branching.py show [--dir <project>] [--json]
    python branching.py rc-branch [--version X.Y.Z] [--dir <project>]
"""
import argparse
import json
import os
import re
import sys
from dataclasses import dataclass

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
from project_config import BRANCHING_MODELS, load_project_config  # noqa: E402

VERSION_RE = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)")
INTEGRATION_NAMES = ("dev", "develop")

# The user's release action: merging into the production branch, or promoting an accepted candidate.
GATES = {"integration": "merge", "trunk": "promote", "release-branches": "promote"}


@dataclass(frozen=True)
class BranchingModel:
    name: str
    production: str
    integration: str       # where work branches start and pull requests go
    release_prefix: str

    @classmethod
    def from_devops(cls, devops: dict) -> "BranchingModel":
        name = devops.get("branching") or "integration"
        if name not in BRANCHING_MODELS:
            raise ValueError(f"devops.branching '{name}' is not one of: {', '.join(BRANCHING_MODELS)}")
        production = devops.get("production_branch") or "main"
        integration = (devops.get("integration_branch") or "dev") if name == "integration" else production
        return cls(name, production, integration, devops.get("release_branch_prefix") or "release")

    @property
    def gate(self) -> str:
        return GATES[self.name]

    @property
    def long_lived(self) -> list:
        if self.name == "integration":
            return [self.production, self.integration]
        if self.name == "release-branches":
            return [self.production, f"{self.release_prefix}/X.Y"]
        return [self.production]

    @property
    def pr_targets(self) -> list:
        """Branches (or patterns) that receive pull requests, so CI must run on them."""
        if self.name == "integration":
            return [self.integration, self.production]
        if self.name == "release-branches":
            return [self.production, f"{self.release_prefix}/*"]
        return [self.production]

    def release_branch(self, version: str) -> str:
        m = VERSION_RE.match(version or "")
        if not m:
            raise ValueError(f"'{version}' is not a version like 1.4.0")
        return f"{self.release_prefix}/{m.group(1)}.{m.group(2)}"

    def rc_branch(self, version: str = None) -> str:
        """The branch a candidate of `version` is tagged on."""
        if self.name == "release-branches":
            return self.release_branch(version)
        return self.integration

    def describe(self) -> dict:
        p, i, r = self.production, self.integration, self.release_prefix
        work = f"feat/, fix/, docs/, ... from '{i}', pull requests into '{i}'"
        if self.name == "integration":
            rc = f"vX.Y.Z-rc.N tagged on '{i}' after the version bump"
            gate = f"the user merges the pull request '{i}' -> '{p}'; tag-on-merge creates vX.Y.Z and the release"
            hotfix = f"hotfix/<slug> from '{p}', pull request into '{p}', then '{p}' merged back into '{i}'"
        elif self.name == "trunk":
            rc = f"vX.Y.Z-rc.N tagged on '{p}' after the version bump; fixes found in acceptance go into '{p}' and a new candidate"
            gate = "the user runs the promote workflow for the accepted candidate; it tags that commit vX.Y.Z and releases it"
            hotfix = (f"fix/<slug> into '{p}', patch bump, candidate, promote; when '{p}' holds unreleased work, "
                      f"hotfix/X.Y.Z from the release tag, the fix by pull request into it, candidate tagged there, then the fix "
                      f"into '{p}' by pull request")
        else:
            work += f"; fixes of a release in acceptance or maintenance from '{r}/X.Y', pull requests into it"
            rc = (f"cut '{r}/X.Y' from '{p}' after the version bump; vX.Y.Z-rc.N tagged on '{r}/X.Y'; "
                  f"fixes go into '{r}/X.Y' and forward into '{p}' by pull request")
            gate = "the user runs the promote workflow for the accepted candidate; it tags that commit vX.Y.Z and releases it"
            hotfix = (f"fix/<slug> from '{r}/X.Y', pull request into it, patch bump, candidate, promote, "
                      f"then forward into '{p}' by pull request")
        return {"model": self.name, "long_lived": self.long_lived, "work": work, "pull_requests_into": self.pr_targets,
                "release_candidate": rc, "gate": self.gate, "release_gate": gate, "hotfix": hotfix}


def model_of(cfg) -> BranchingModel:
    return BranchingModel.from_devops(cfg.section("devops"))


def detect(branches: list, production: str = "main") -> dict:
    """The model a repository already follows, from its branch names (local or `origin/`).
    {model, integration_branch, reason}; model None when there is nothing to go by."""
    names = {b[len("origin/"):] if b.startswith("origin/") else b for b in branches}
    if any(n.startswith("release/") for n in names):
        return {"model": "release-branches", "integration_branch": None, "reason": "release/* branches exist"}
    for name in INTEGRATION_NAMES:
        if name in names:
            return {"model": "integration", "integration_branch": name, "reason": f"'{name}' branch exists"}
    if names:
        return {"model": "trunk", "integration_branch": None, "reason": f"only '{production}' and short-lived branches"}
    return {"model": None, "integration_branch": None, "reason": "no branches yet"}


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="The project's branching model (devops.branching)")
    parser.add_argument("command", choices=["show", "rc-branch"])
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--version", default=None, help="With rc-branch: the version (default: devops.version_files)")
    parser.add_argument("--json", action="store_true", help="With show: print JSON")
    args = parser.parse_args()
    cfg = load_project_config(args.dir)
    try:
        model = model_of(cfg)
    except ValueError as e:
        print(f"❌ {e}")
        return 1

    if args.command == "rc-branch":
        version = args.version
        if not version and model.name == "release-branches":
            from release_status import project_version
            version, problems = project_version(cfg)
            for p in problems:
                print(f"❌ {p}")
            if version is None:
                return 1
        try:
            print(model.rc_branch(version))
        except ValueError as e:
            print(f"❌ {e}")
            return 1
        return 0

    data = model.describe()
    if args.json:
        print(json.dumps(data, indent=2, ensure_ascii=False))
        return 0
    print(f"🌿 Branching model: {data['model']}{'' if cfg.is_set('devops', 'branching') else ' (default; not set in .dev-cycle.json)'}")
    print(f"   Long-lived branches : {', '.join(data['long_lived'])}")
    print(f"   Work branches       : {data['work']}")
    print(f"   Release candidate   : {data['release_candidate']}")
    print(f"   Release gate (user) : {data['release_gate']}")
    print(f"   Hotfix              : {data['hotfix']}")
    if model.name != "integration" and cfg.is_set("devops", "integration_branch"):
        print(f"   ℹ️ devops.integration_branch is not used by '{model.name}'")
    return 0


if __name__ == "__main__":
    sys.exit(main())
