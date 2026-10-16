"""
Verify Gate Utility - dev-cycle-protocol Skill
Automated gate compliance validator for the 8-step software lifecycle.
Validates Pre-Implementation (Gate 3) and Pre-Release (Gate 8) standards.
Project-agnostic: automatically adapts to the detected tech stack.
"""
import os
import sys
import argparse
import subprocess
import glob
import re

def run_cmd(cmd: list[str], cwd: str) -> tuple[int, str]:
    try:
        res = subprocess.run(
            cmd,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )
        return res.returncode, res.stdout.strip()
    except Exception as e:
        return -1, str(e)

def check_secret_hygiene(cwd: str) -> list[str]:
    violations = []
    dangerous_patterns = [".env", ".env.local", "id_rsa", "credentials.json", "*secret*", "*private_key*"]
    
    # 1. Check if any dangerous files are tracked in git
    for pat in dangerous_patterns:
        code, out = run_cmd(["git", "ls-files", pat], cwd)
        if code == 0 and out.strip():
            for f in out.splitlines():
                if f.strip():
                    violations.append(f"Tracked credential file committed to Git: {f.strip()}")

    # 2. Check if any dangerous files are staged in git
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    if code == 0 and status_out.strip():
        for line in status_out.splitlines():
            status_tag = line[:2]
            filename = line[3:].strip()
            # If staged (A, M, R in first char)
            if status_tag[0] in ["A", "M", "R"]:
                for pat in [".env", "id_rsa", "credentials.json"]:
                    if pat in filename.lower():
                        # Verify it's not a sample/example file
                        if not any(safe in filename.lower() for safe in [".example", ".sample", "template"]):
                            violations.append(f"Staged credential file ready to be committed: {filename}")

    return violations

def verify_gate_3(cwd: str, project_name: str) -> bool:
    """Pre-Implementation Gate (Gate 3)"""
    print("=" * 80)
    print(f"🔒 VERIFYING GATE 3 (PRE-IMPLEMENTATION GATE): {project_name}")
    print("=" * 80)

    passed = True
    # 1. Check Git Status
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty_files = [l for l in status_out.splitlines() if l.strip()]
    if len(dirty_files) > 0:
        print(f"⚠️ Working tree has {len(dirty_files)} uncommitted/untracked changes.")
    else:
        print("✅ Git working tree is clean.")

    # 2. Check the project's plan files (plan-memory: docs/plans)
    try:
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "plan-memory"))
        from _store import PlanStore
        store = PlanStore(cwd)
        in_force = [p for p in store.load_all() if p.meta.get("status") in ("active", "in_progress")]
        if in_force:
            latest = store.latest(in_force)
            print(f"✅ Active plan found in {store.plans_dir}: '{latest.meta['plan_id']}' ({len(in_force)} active/in progress)")
            _, tracked = run_cmd(["git", "ls-files", "--", latest.path], cwd)
            if not tracked:
                print("⚠️ The active plan file is not committed yet. Commit it with the approved plan.")
        else:
            print(f"⚠️ Warning: No active plan in {store.plans_dir}. Save the blueprint with plan-memory in Step 2!")
    except Exception as e:
        print(f"ℹ️ Could not check plan-memory plans: {e}")

    # 3. Secret Hygiene Check
    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed (no unencrypted credential files detected).")

    print("-" * 80)
    if passed:
        print("🟢 GATE 3 STATUS: READY FOR EXPLICIT USER APPROVAL")
    else:
        print("🚨 GATE 3 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed

def verify_gate_8(cwd: str, project_name: str) -> bool:
    """Pre-Release / Closure Gate (Gate 8)"""
    print("=" * 80)
    print(f"🚀 VERIFYING GATE 8 (PRE-RELEASE & PRODUCTION CLOSURE GATE): {project_name}")
    print("=" * 80)

    passed = True

    # 1. Dual-Doc Verification
    has_userdoc = os.path.exists(os.path.join(cwd, "README.md"))
    has_tecdoc = os.path.exists(os.path.join(cwd, "TECDOC.md")) or os.path.exists(os.path.join(cwd, "docs", "ARCHITECTURE.md"))
    has_manual_tests = os.path.exists(os.path.join(cwd, "tests", "MANUAL_TESTS.md")) or os.path.exists(os.path.join(cwd, "MANUAL_TESTS.md"))

    if has_userdoc:
        print("✅ UserDoc verified (README.md exists).")
    else:
        print("❌ UserDoc missing (README.md required).")
        passed = False

    if has_tecdoc:
        print("✅ TecDoc verified (TECDOC.md exists).")
    else:
        print("❌ TecDoc missing (TECDOC.md required).")
        passed = False

    if has_manual_tests:
        print("✅ Cumulative manual tests suite verified (MANUAL_TESTS.md exists).")
    else:
        print("❌ Cumulative manual tests suite missing (tests/MANUAL_TESTS.md required).")
        passed = False

    # 2. Check Secret Hygiene
    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed.")

    # 3. Check Git Status
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty = [l for l in status_out.splitlines() if l.strip()]
    if dirty:
        print(f"⚠️ Uncommitted changes remaining before release: {len(dirty)} files.")
    else:
        print("✅ Working directory clean for release commit.")

    print("-" * 80)
    if passed:
        print("🟢 GATE 8 STATUS: READY FOR PRODUCTION RELEASE & MERGE")
    else:
        print("🚨 GATE 8 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Verify gate compliance for dev-cycle-protocol")
    parser.add_argument("--gate", type=str, required=True, choices=["3", "8", "gate3", "gate8"], help="Gate to verify (3=Pre-Implementation, 8=Pre-Release)")
    parser.add_argument("--dir", type=str, default=None, help="Workspace directory")
    parser.add_argument("--project", type=str, default=None, help="Project name override")
    args = parser.parse_args()

    cwd = os.path.abspath(args.dir or os.getcwd())
    p_name = args.project or os.path.basename(cwd)

    gate_num = args.gate.replace("gate", "")
    if gate_num == "3":
        success = verify_gate_3(cwd, p_name)
    else:
        success = verify_gate_8(cwd, p_name)

    if not success:
        sys.exit(1)
