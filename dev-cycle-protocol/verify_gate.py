"""
Verify Gate Utility - dev-cycle-protocol Skill
Automated gate checks of the role-based development cycle:
  gate 3  - before asking for plan approval
  gate rc - release candidate acceptance (all manual tests of the run Pass)
  gate 8  - release readiness, before the release pull request (runs every role's checker)
Project-agnostic: adapts to the project's .dev-cycle.json and detected stack.
"""
import argparse
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from project_config import load_project_config  # noqa: E402

SKILLS_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def run_cmd(cmd: list, cwd: str) -> tuple:
    try:
        res = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True, encoding="utf-8", errors="replace")
        return res.returncode, res.stdout.strip()
    except Exception as e:
        return -1, str(e)


def check_secret_hygiene(cwd: str) -> list:
    violations = []
    dangerous_patterns = [".env", ".env.local", "id_rsa", "credentials.json", "*secret*", "*private_key*"]

    # 1. Dangerous files tracked in Git
    for pat in dangerous_patterns:
        code, out = run_cmd(["git", "ls-files", pat], cwd)
        if code == 0 and out.strip():
            for f in out.splitlines():
                if f.strip():
                    violations.append(f"Tracked credential file committed to Git: {f.strip()}")

    # 2. Dangerous files staged for commit
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    if code == 0 and status_out.strip():
        for line in status_out.splitlines():
            status_tag = line[:2]
            filename = line[3:].strip()
            if status_tag[0] in ["A", "M", "R"]:
                for pat in [".env", "id_rsa", "credentials.json"]:
                    if pat in filename.lower():
                        if not any(safe in filename.lower() for safe in [".example", ".sample", "template"]):
                            violations.append(f"Staged credential file ready to be committed: {filename}")
    return violations


def run_checker(skill: str, script: str, args: list) -> tuple:
    """Runs a sibling skill's checker; returns (exit_code, output), or (None, reason) when missing."""
    path = os.path.join(SKILLS_ROOT, skill, script)
    if not os.path.isfile(path):
        return None, f"{skill}/{script} not installed"
    res = subprocess.run([sys.executable, path, *args], capture_output=True, text=True,
                         encoding="utf-8", errors="replace")
    return res.returncode, (res.stdout + res.stderr).strip()


def report_checker(label: str, code, output: str, blocking: bool = True, max_lines: int = 20) -> bool:
    """Prints a checker's outcome; returns False when it blocks the gate."""
    lines = output.splitlines() if output else []
    if code is None:
        print(f"⚠️ {label}: skipped ({output}).")
        return True
    if code == 0:
        print(f"✅ {label} passed. {lines[-1] if lines else ''}")
        return True
    print(f"{'❌' if blocking else '⚠️'} {label} failed:")
    for line in lines[:max_lines]:
        print(f"   {line}")
    if len(lines) > max_lines:
        print(f"   ... ({len(lines) - max_lines} more lines)")
    return not blocking


def verify_gate_3(cwd: str, project_name: str) -> bool:
    """Plan approval gate."""
    print("=" * 80)
    print(f"🔒 VERIFYING GATE 3 (PLAN APPROVAL): {project_name}")
    print("=" * 80)

    passed = True
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty_files = [l for l in status_out.splitlines() if l.strip()]
    if dirty_files:
        print(f"⚠️ Working tree has {len(dirty_files)} uncommitted/untracked changes.")
    else:
        print("✅ Git working tree is clean.")

    try:
        sys.path.insert(0, os.path.join(SKILLS_ROOT, "plan-memory"))
        from _store import PlanStore
        store = PlanStore(cwd)
        in_force = [p for p in store.load_all() if p.meta.get("status") in ("active", "in_progress")]
        if in_force:
            latest = store.latest(in_force)
            print(f"✅ Active plan found in {store.plans_dir}: '{latest.meta['plan_id']}' ({len(in_force)} active/in progress)")
            _, tracked = run_cmd(["git", "ls-files", "--", latest.path], cwd)
            if not tracked:
                print("⚠️ The active plan file is not committed yet. Commit it after approval.")
        else:
            print(f"⚠️ No active plan in {store.plans_dir}. Save the plan with plan-memory in Phase 2 (full path).")
    except Exception as e:
        print(f"ℹ️ Could not check plan-memory plans: {e}")

    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed (no unencrypted credential files detected).")

    print("-" * 80)
    print("🟢 GATE 3 STATUS: READY FOR EXPLICIT USER APPROVAL" if passed else "🚨 GATE 3 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed


def verify_gate_rc(cwd: str, project_name: str, run: str) -> bool:
    """Release candidate acceptance gate: every manual test of the run is Pass."""
    print("=" * 80)
    print(f"✅ VERIFYING RELEASE CANDIDATE ACCEPTANCE: {project_name} ({run or 'no run'})")
    print("=" * 80)
    if not run:
        print("❌ --run <rc tag> is required (the run started with quality-assurance/manual_tests.py start-run).")
        print("=" * 80 + "\n")
        return False
    code, out = run_checker("quality-assurance", "manual_tests.py", ["gate", "--run", run, "--dir", cwd])
    passed = code is not None and report_checker(f"Manual test run {run}", code, out)
    if code is None:
        print(f"❌ {out}")
    print("-" * 80)
    print(f"🟢 RC GATE STATUS: CANDIDATE ACCEPTED" if passed else "🚨 RC GATE STATUS: NOT ACCEPTED")
    print("=" * 80 + "\n")
    return passed


def verify_gate_8(cwd: str, project_name: str, run: str = None) -> bool:
    """Release readiness gate, before opening the pull request into the production branch."""
    print("=" * 80)
    print(f"🚀 VERIFYING GATE 8 (RELEASE READINESS): {project_name}")
    print("=" * 80)

    passed = True
    cfg = load_project_config(cwd)

    # 1. Documentation and manual tests at the configured paths
    for label, rel in (("UserDoc", cfg.section("docs").get("user_doc")),
                       ("TecDoc", cfg.section("docs").get("tec_doc")),
                       ("Manual test suite", cfg.section("tests").get("manual_tests"))):
        if rel and os.path.exists(os.path.join(cwd, rel)):
            print(f"✅ {label} verified ({rel} exists).")
        else:
            print(f"❌ {label} missing ({rel or 'path not configured'} required).")
            passed = False

    # 2. Technical documentation (architecture-docs)
    code, out = run_checker("architecture-docs", "check_docs.py", ["--dir", cwd, "--max-warnings", "10"])
    if code is not None and "does not use the architecture-docs layout" in out:
        print("⚠️ TecDoc not migrated to the architecture-docs layout (docs/architecture + docs/adr) yet.")
    else:
        passed &= report_checker("Architecture docs check (architecture-docs)", code, out)

    # 3. README (user-docs)
    passed &= report_checker("README check (user-docs)",
                             *run_checker("user-docs", "check_readme.py", ["--dir", cwd]))

    # 4. Manual test suite format (quality-assurance)
    passed &= report_checker("Manual test suite (quality-assurance)",
                             *run_checker("quality-assurance", "manual_tests.py", ["validate", "--dir", cwd]))

    # 5. Design and core dependencies (software-design)
    passed &= report_checker("Design check (software-design)",
                             *run_checker("software-design", "check_design.py", ["--dir", cwd]))

    # 6. CI policy (devops-release), when the project uses GitHub Actions
    if os.path.isdir(os.path.join(cwd, ".github", "workflows")):
        passed &= report_checker("CI policy (devops-release)",
                                 *run_checker("devops-release", "check_ci_policy.py", ["--dir", cwd]))
    else:
        print("⚠️ No .github/workflows: CI policy check skipped.")

    # 7. Version files agree (devops-release)
    if cfg.section("devops").get("version_files"):
        passed &= report_checker("Version files (devops-release)",
                                 *run_checker("devops-release", "release_status.py", ["version", "--dir", cwd]))
    else:
        print("⚠️ devops.version_files is not set: version consistency not checked.")

    # 8. Release candidate acceptance (quality-assurance)
    if cfg.section("devops").get("release_candidates", True):
        if run:
            code, out = run_checker("quality-assurance", "manual_tests.py", ["gate", "--run", run, "--dir", cwd])
            ok = code is not None and report_checker(f"Release candidate run {run} (quality-assurance)", code, out)
            if code is None:
                print(f"❌ {out}")
            passed &= ok
        else:
            print("⚠️ Release candidates are on but no --run was given: acceptance of the candidate not checked.")

    # 9. Secret hygiene
    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed.")

    # 10. Git status
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty = [l for l in status_out.splitlines() if l.strip()]
    if dirty:
        print(f"⚠️ Uncommitted changes remaining before release: {len(dirty)} files.")
    else:
        print("✅ Working directory clean for release commit.")

    print("-" * 80)
    print("🟢 GATE 8 STATUS: READY FOR THE RELEASE PULL REQUEST (the user merges)" if passed
          else "🚨 GATE 8 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Verify gate compliance for dev-cycle-protocol")
    parser.add_argument("--gate", type=str, required=True, choices=["3", "8", "rc", "gate3", "gate8", "gaterc"],
                        help="Gate to verify (3=plan approval, rc=candidate acceptance, 8=release readiness)")
    parser.add_argument("--dir", type=str, default=None, help="Workspace directory")
    parser.add_argument("--project", type=str, default=None, help="Project name override")
    parser.add_argument("--run", type=str, default=None, help="Release candidate run label, for example v1.4.0-rc.1")
    args = parser.parse_args()

    cwd = os.path.abspath(args.dir or os.getcwd())
    p_name = args.project or load_project_config(cwd).project_name

    gate = args.gate.replace("gate", "")
    if gate == "3":
        success = verify_gate_3(cwd, p_name)
    elif gate == "rc":
        success = verify_gate_rc(cwd, p_name, args.run)
    else:
        success = verify_gate_8(cwd, p_name, args.run)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
