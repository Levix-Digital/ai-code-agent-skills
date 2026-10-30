"""The plan search index written by the engine (`plan-memory rebuild_index`) is the chromadb
store the Python scripts read and write: both sides use one index, in turns."""
import os
import shutil
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
REPO = os.path.dirname(SKILL)
sys.path.insert(0, SKILL)
sys.path.append(os.path.join(REPO, "dev-cycle-protocol", "tests"))

from _store import PlanStore  # noqa: E402
from _engine import run_engine  # noqa: E402

FIXTURE = os.path.join(REPO, "tests", "parity", "fixtures", "plans-list")


@pytest.fixture
def project(tmp_path, monkeypatch):
    root = tmp_path / "project"
    shutil.copytree(FIXTURE, root)
    monkeypatch.setenv("PLAN_MEMORY_DB", str(tmp_path / "db"))
    return root


def rebuild(project) -> str:
    res = run_engine("plan-memory", "rebuild_index", ["--dir", str(project)])
    assert res.returncode == 0, res.stderr
    return res.stdout + res.stderr


def indexed(store: PlanStore) -> dict:
    got = store.collection().get(where={"project": store.project})
    return {i: m for i, m in zip(got["ids"], got["metadatas"])}


def test_python_reads_the_index_the_engine_built(project):
    out = rebuild(project)
    assert "7 updated, 0 removed, 7 plan(s)" in out
    store = PlanStore(str(project))
    plans = store.load_all()
    meta = indexed(store)
    assert sorted(meta) == sorted(p.meta["plan_id"] for p in plans)
    for p in plans:
        assert meta[p.meta["plan_id"]]["sha256"] == store.index_sha(p)
    # Nothing changed: Python's own sync finds the index up to date.
    store.sync_index(plans)
    alpha = store.get("alpha")
    hits = store.collection().query(query_texts=[store.full_text(alpha)], n_results=1)
    assert hits["ids"][0] == ["alpha"]


def test_engine_and_python_take_turns(project, capsys):
    rebuild(project)
    store = PlanStore(str(project))
    path = store.path_for("alpha")
    with open(path, "a", encoding="utf-8") as f:
        f.write("\nOne more line.\n")
    store.sync_index(quiet=False)
    assert "1 updated, 0 removed, 7 plan(s)" in capsys.readouterr().err
    os.remove(store.path_for("beta"))
    store.sync_index()
    assert "beta" not in indexed(store)
    assert "6 updated, 0 removed, 6 plan(s)" in rebuild(project)
    assert len(indexed(store)) == 6
    store.sync_index(quiet=False)
    assert "0 updated, 0 removed, 6 plan(s)" in capsys.readouterr().err
