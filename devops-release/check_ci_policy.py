"""
Check CI Policy - devops-release Skill
Checks a repository's GitHub Actions workflows against the devops-release policy:
tests on pull requests into the integration and production branches, pre-releases for
-rc tags, a tag-on-merge job that can actually trigger the release, release permissions,
and existing version files.

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

RELEASE_ACTION_RE = re.compile(r"softprops/action-gh-release|ncipollo/release-action|gh\s+release\s+create")
TEST_STEP_RE = re.compile(r"\b(cargo\s+test|pytest|npm\s+(run\s+)?test|pnpm\s+(run\s+)?test|yarn\s+test|go\s+test|dotnet\s+test|mvn\s+test|gradle\w*\s+test|make\s+test|tox)\b")
TAG_PUSH_RE = re.compile(r"git\s+tag\b[\s\S]*?git\s+push\b|git\s+push\s+origin\s+\S*v\$|gh\s+release\s+create")
REUSABLE_CALL_RE = re.compile(r"^\s*uses:\s*\./\.github/workflows/", re.M)
PRERELEASE_RE = re.compile(r"^\s*prerelease\s*:|--prerelease", re.I | re.M)
RC_TEST_RE = re.compile(r"-rc\b|\brc\.")
CONTENTS_WRITE_RE = re.compile(r"contents\s*:\s*write")
CUSTOM_SECRET_RE = re.compile(r"secrets\.(?!GITHUB_TOKEN\b)[A-Z0-9_]+")


class Workflow:
    def __init__(self, path: str):
        self.path = path
        self.name = os.path.basename(path)
        with open(path, "r", encoding="utf-8") as f:
            self.text = f.read()
        data = yaml.safe_load(self.text) or {}
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
    integration = devops.get("integration_branch") or "dev"
    production = devops.get("production_branch") or "main"
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
    for branch in (integration, production):
        if not any(w.pr_covers(branch) for w in tests):
            errors.append(f"no test workflow runs on pull requests into '{branch}' (see templates/ci.yml)")

    releases = [w for w in workflows if w.publishes_release]
    if not releases:
        warnings.append("no workflow publishes releases (see templates/release.yml)")
    for w in releases:
        if not CONTENTS_WRITE_RE.search(w.text):
            errors.append(f"{w.name}: publishing a release needs 'permissions: contents: write'")
        if use_rc and (w.push_tags() or "workflow_call" in w.triggers) and not (PRERELEASE_RE.search(w.text) and RC_TEST_RE.search(w.text)):
            errors.append(f"{w.name}: '-rc' tags must publish pre-releases (prerelease: ${{{{ contains(<tag>, '-rc') }}}})")

    taggers = [w for w in workflows if w.push_covers_branch(production) and TAG_PUSH_RE.search(w.text)]
    if not taggers:
        warnings.append(f"no workflow creates the final tag when '{production}' receives the merge (see templates/tag-on-merge.yml)")
    for w in taggers:
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
