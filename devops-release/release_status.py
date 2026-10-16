"""
Release Status - devops-release Skill
Version and release helpers: read the version from the project's version files, compute
the next release candidate tag, and verify a published release with the GitHub CLI.

Usage:
    python release_status.py version [--dir <project>]
    python release_status.py next-rc [--dir <project>]
    python release_status.py verify --tag vX.Y.Z[-rc.N] [--repo org/name] [--asset PATTERN ...]
"""
import argparse
import fnmatch
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

try:
    import tomllib  # Python 3.11+
except ImportError:  # pragma: no cover - Python 3.10
    tomllib = None

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
RC_TAG_RE = re.compile(r"^v(\d+\.\d+\.\d+)-rc\.(\d+)$")


def _toml_version(text: str, tables: list):
    if tomllib:
        data = tomllib.loads(text)
        for table in tables:
            node = data
            for part in table.split("."):
                node = node.get(part, {}) if isinstance(node, dict) else {}
            value = node.get("version") if isinstance(node, dict) else None
            if isinstance(value, str):
                return value
        return None
    for table in tables:
        m = re.search(rf"^\[{re.escape(table)}\][^\[]*?^version\s*=\s*\"([^\"]+)\"", text, re.M | re.S)
        if m:
            return m.group(1)
    return None


def read_version_file(path: str):
    """The version string in one file, or None when the file holds no version."""
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    name = os.path.basename(path).lower()
    if name == "cargo.toml":
        return _toml_version(text, ["package", "workspace.package"])
    if name == "pyproject.toml":
        return _toml_version(text, ["project", "tool.poetry"])
    if name == "package.json":
        value = json.loads(text).get("version")
        return value if isinstance(value, str) else None
    if name in ("version.txt", "version"):
        value = text.strip().lstrip("﻿").lstrip("vV")
        return value or None
    m = re.search(r"""version\s*[=:]\s*["']([^"']+)["']""", text)
    return m.group(1) if m else None


def project_version(cfg):
    """(version, problems). Problems when files are missing, have no version or disagree."""
    files = cfg.section("devops").get("version_files") or []
    if not files:
        return None, ["devops.version_files is empty in .dev-cycle.json"]
    found, problems = {}, []
    for rel in files:
        path = os.path.join(cfg.root, rel)
        if not os.path.isfile(path):
            problems.append(f"{rel}: file not found")
            continue
        value = read_version_file(path)
        if value is None:
            problems.append(f"{rel}: no version found")
        elif not SEMVER_RE.match(value):
            problems.append(f"{rel}: '{value}' is not a semantic version")
        else:
            found[rel] = value
    distinct = sorted(set(found.values()))
    if len(distinct) > 1:
        problems.append("version files disagree: " + ", ".join(f"{k}={v}" for k, v in found.items()))
    return (distinct[0] if len(distinct) == 1 else None), problems


def git_tags(root: str, pattern: str) -> list:
    res = subprocess.run(["git", "tag", "-l", pattern], cwd=root, capture_output=True, text=True, encoding="utf-8")
    return [t.strip() for t in res.stdout.splitlines() if t.strip()] if res.returncode == 0 else []


def next_rc_tag(version: str, existing_tags: list):
    """(tag, problem). Problem when the final tag already exists."""
    if f"v{version}" in existing_tags:
        return None, f"v{version} is already released; bump the version before cutting a candidate"
    numbers = [int(m.group(2)) for t in existing_tags for m in [RC_TAG_RE.match(t)] if m and m.group(1) == version]
    return f"v{version}-rc.{max(numbers, default=0) + 1}", None


def evaluate_release(tag: str, data: dict, asset_patterns: list) -> list:
    """Problems with a release as returned by `gh release view --json`."""
    problems = []
    is_rc = bool(RC_TAG_RE.match(tag)) or "-rc" in tag
    if data.get("isDraft"):
        problems.append("the release is still a draft")
    if bool(data.get("isPrerelease")) != is_rc:
        expected = "a pre-release" if is_rc else "a final release (not a pre-release)"
        problems.append(f"{tag} should be {expected}")
    names = [a.get("name", "") for a in data.get("assets") or []]
    if not names:
        problems.append("the release has no assets")
    for pattern in asset_patterns:
        if not any(fnmatch.fnmatch(n, pattern) for n in names):
            problems.append(f"no asset matches '{pattern}'")
    return problems


def fetch_release(tag: str, repo: str = None) -> dict:
    cmd = ["gh", "release", "view", tag, "--json", "tagName,isPrerelease,isDraft,assets,url"]
    if repo:
        cmd += ["--repo", repo]
    res = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    if res.returncode != 0:
        raise RuntimeError(res.stderr.strip() or f"gh release view {tag} failed")
    return json.loads(res.stdout)


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Version, release candidate and published release helpers")
    parser.add_argument("command", choices=["version", "next-rc", "verify"])
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--tag", default=None, help="With verify: the release tag")
    parser.add_argument("--repo", default=None, help="With verify: repository (default: devops.releases_repo, else the current one)")
    parser.add_argument("--asset", action="append", default=[], help="With verify: asset name pattern that must exist; repeatable")
    args = parser.parse_args()
    cfg = load_project_config(args.dir)

    if args.command in ("version", "next-rc"):
        version, problems = project_version(cfg)
        for p in problems:
            print(f"❌ {p}")
        if version is None:
            return 1
        if args.command == "version":
            print(version)
            return 1 if problems else 0
        tag, problem = next_rc_tag(version, git_tags(cfg.root, "v*"))
        if problem:
            print(f"❌ {problem}")
            return 1
        print(tag)
        return 1 if problems else 0

    if not args.tag:
        print("❌ verify needs --tag")
        return 1
    repo = args.repo or cfg.section("devops").get("releases_repo")
    try:
        data = fetch_release(args.tag, repo)
    except (RuntimeError, FileNotFoundError, json.JSONDecodeError) as e:
        print(f"❌ Release {args.tag} not found{' in ' + repo if repo else ''}: {e}")
        return 1
    problems = evaluate_release(args.tag, data, args.asset)
    for p in problems:
        print(f"❌ {p}")
    assets = len(data.get("assets") or [])
    status = "🟢 verified" if not problems else "🚨 not ready"
    print(f"{status}: {args.tag} ({'pre-release' if data.get('isPrerelease') else 'release'}, {assets} asset(s)) {data.get('url', '')}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
