"""
Check CI Policy - devops-release Skill
Checks a repository's GitHub Actions workflows against the devops-release policy, for the
project's branching model (`devops.branching`): tests on pull requests into every branch
that receives them, a linter or static analysis on the same pull requests, pre-releases for -rc tags, the release gate (a tag-on-merge job, or a
promote workflow) able to actually trigger the release, release permissions, and existing
version files.

Usage:
    python check_ci_policy.py [--dir <project>]
Exit code 1 on errors.
"""
import argparse
import fnmatch
import glob
import os
import re
import sys

import yaml

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from branching import BranchingModel  # noqa: E402

RELEASE_ACTION_RE = re.compile(r"softprops/action-gh-release|ncipollo/release-action|gh\s+release\s+create")
TEST_STEP_RE = re.compile(r"\b(cargo\s+test|pytest|npm\s+(run\s+)?test|pnpm\s+(run\s+)?test|yarn\s+test|go\s+test|dotnet\s+test|mvn\s+test|gradle\w*\s+test|make\s+test|tox)\b")
LINT_STEP_RE = re.compile(r"\b(clippy|cargo\s+fmt\b[^\n]*--check|ruff|flake8|pylint|mypy|black\s+--check|eslint|"
                          r"biome|prettier\s+--check|golangci-lint|go\s+vet|gofmt|staticcheck|rubocop|ktlint|detekt|"
                          r"dotnet\s+format|shellcheck|codeql|(npm|pnpm|yarn)\s+(run\s+)?lint|make\s+lint)\b")
TAG_PUSH_RE = re.compile(r"git\s+tag\b[\s\S]*?git\s+push\b|git\s+push\s+origin\s+\S*v\$|gh\s+release\s+create")
REUSABLE_CALL_RE = re.compile(r"^\s*uses:\s*\./\.github/workflows/", re.M)
PRERELEASE_RE = re.compile(r"^\s*prerelease\s*:|--prerelease", re.I | re.M)
RC_TEST_RE = re.compile(r"-rc\b|\brc\.")
CONTENTS_WRITE_RE = re.compile(r"contents\s*:\s*write")
CUSTOM_SECRET_RE = re.compile(r"secrets\.(?!GITHUB_TOKEN\b)[A-Z0-9_]+")
ENVIRONMENT_RE = re.compile(r"^\s*environment\s*:", re.M)


class Workflow:
    def __init__(self, path: str):
        self.path = path
        self.name = os.path.basename(path)
        with open(path, "r", encoding="utf-8") as f:
            self.text = f.read()
        data = yaml.safe_load(self.text) or {}
        jobs = data.get("jobs") if isinstance(data, dict) else None
        steps = [step for job in (jobs or {}).values() if isinstance(job, dict)
                 for step in job.get("steps") or [] if isinstance(step, dict)]
        # What the steps run, without the comments (a template names linters in its comments).
        self.commands = "\n".join(str(step.get(k) or "") for step in steps for k in ("run", "uses"))
        # PyYAML reads the bare key `on` as boolean True.
        raw = data.get("on", data.get(True, {})) if isinstance(data, dict) else {}
        if isinstance(raw, str):
            raw = {raw: None}
        elif isinstance(raw, list):
            raw = {k: None for k in raw}
        self.triggers = raw or {}

    def _filter(self, event: str, key: str):
        """None when the event is absent; [] when present without a filter (all); else the list."""
        if event not in self.triggers:
            return None
        conf = self.triggers.get(event) or {}
        values = conf.get(key) if isinstance(conf, dict) else None
        if values is None:
            return []
        return [values] if isinstance(values, str) else list(values)

    def pr_covers(self, branch: str) -> bool:
        for event in ("pull_request", "pull_request_target"):
            branches = self._filter(event, "branches")
            if branches is not None and (branches == [] or any(fnmatch.fnmatch(branch, b) for b in branches)):
                return True
        return False

    def push_covers_branch(self, branch: str) -> bool:
        branches = self._filter("push", "branches")
        return bool(branches) and any(fnmatch.fnmatch(branch, b) for b in branches)

    def push_tags(self) -> bool:
        return bool(self._filter("push", "tags"))

    @property
    def runs_tests(self) -> bool:
        return bool(TEST_STEP_RE.search(self.text))

    @property
    def runs_lint(self) -> bool:
        return bool(LINT_STEP_RE.search(self.commands))

    @property
    def publishes_release(self) -> bool:
        return bool(RELEASE_ACTION_RE.search(self.text))


def load_workflows(root: str) -> list:
    paths = sorted(glob.glob(os.path.join(root, ".github", "workflows", "*.yml")) +
                   glob.glob(os.path.join(root, ".github", "workflows", "*.yaml")))
    out = []
    for p in paths:
        try:
            out.append(Workflow(p))
        except yaml.YAMLError as e:
            out.append(e)
    return out


def run(project_dir: str = None):
    """Returns (errors, warnings)."""
    errors, warnings = [], []
    cfg = load_project_config(project_dir)
    devops = cfg.section("devops")
    try:
        model = BranchingModel.from_devops(devops)
    except ValueError as e:
        return [str(e)], warnings
    production = model.production
    use_rc = devops.get("release_candidates", True)

    for rel in devops.get("version_files") or []:
        if not os.path.isfile(os.path.join(cfg.root, rel)):
            errors.append(f"devops.version_files: '{rel}' does not exist")
    if not devops.get("version_files"):
        warnings.append("devops.version_files is empty; list the files that hold the version")

    loaded = load_workflows(cfg.root)
    workflows = [w for w in loaded if isinstance(w, Workflow)]
    for bad in loaded:
        if not isinstance(bad, Workflow):
            errors.append(f"workflow YAML could not be parsed: {bad}")
    if not workflows:
        errors.append("no GitHub Actions workflows in .github/workflows")
        return errors, warnings

    tests = [w for w in workflows if w.runs_tests]
    for target in model.pr_targets:
        sample = target.replace("*", "0.0")   # release/* is checked with a branch it must cover
        if not any(w.pr_covers(sample) for w in tests):
            errors.append(f"no test workflow runs on pull requests into '{target}' (see templates/ci.yml)")
        if not any(w.runs_lint and w.pr_covers(sample) for w in workflows):
            warnings.append(f"no workflow runs a linter or static analysis on pull requests into '{target}' "
                            "(see the lint job of templates/ci.yml)")

    releases = [w for w in workflows if w.publishes_release]
    if not releases:
        warnings.append("no workflow publishes releases (see templates/release.yml)")
    for w in releases:
        if not CONTENTS_WRITE_RE.search(w.text):
            errors.append(f"{w.name}: publishing a release needs 'permissions: contents: write'")
        if use_rc and (w.push_tags() or "workflow_call" in w.triggers) and not (PRERELEASE_RE.search(w.text) and RC_TEST_RE.search(w.text)):
            errors.append(f"{w.name}: '-rc' tags must publish pre-releases (prerelease: ${{{{ contains(<tag>, '-rc') }}}})")

    merged = [t.replace("*", "0.0") for t in model.pr_targets]
    taggers = [w for w in workflows if TAG_PUSH_RE.search(w.text) and any(w.push_covers_branch(b) for b in merged)]
    if model.gate == "merge":
        gate = [w for w in taggers if w.push_covers_branch(production)]
        if not gate:
            warnings.append(f"no workflow creates the final tag when '{production}' receives the merge (see templates/tag-on-merge.yml)")
    else:
        for w in taggers:
            errors.append(f"{w.name}: creates a tag on every merge, but under '{model.name}' the release gate is the user's "
                          "promotion of an accepted candidate (see templates/promote.yml)")
        gate = [w for w in workflows if "workflow_dispatch" in w.triggers and TAG_PUSH_RE.search(w.text)]
        if not gate:
            warnings.append(f"no promote workflow releases an accepted candidate under '{model.name}' (see templates/promote.yml)")
        for w in gate:
            if not ENVIRONMENT_RE.search(w.text):
                warnings.append(f"{w.name}: no 'environment:' with required reviewers holds the promotion for the user's approval")
    for w in gate:
        calls_release = bool(REUSABLE_CALL_RE.search(w.text)) or w.publishes_release
        uses_own_token = bool(CUSTOM_SECRET_RE.search(w.text))
        if not calls_release and not uses_own_token:
            errors.append(f"{w.name}: pushes a tag with GITHUB_TOKEN, which does not trigger the release workflow; "
                          "call the release as a reusable workflow or push with a dedicated token")
    return errors, warnings


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Check GitHub Actions workflows against the devops-release policy")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    args = parser.parse_args()
    errors, warnings = run(args.dir)
    for e in errors:
        print(f"❌ {e}")
    for w in warnings:
        print(f"⚠️ {w}")
    print(f"\n{'✅' if not errors else '❌'} devops-release CI policy: {len(errors)} error(s), {len(warnings)} warning(s).")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
