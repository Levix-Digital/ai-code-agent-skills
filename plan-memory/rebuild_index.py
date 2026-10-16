"""
Rebuild Index Utility - plan-memory Skill
Drops and rebuilds the local search index of the project's plans from docs/plans.
The index is disposable: the plan files are the source of truth.
"""
import argparse

from _store import PlanStore, setup_stdout

if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Rebuild the local plan search index from docs/plans")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    args = parser.parse_args()

    store = PlanStore(args.dir, args.project)
    store.sync_index(rebuild=True, quiet=False)
    print(f"✅ Index rebuilt for '{store.project}'.")
