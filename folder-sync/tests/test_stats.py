"""/folder-stats reads the sync state of a folder's index (BUG-005: it failed on any synced
index because the last sync time could not be formatted)."""
import os
import sqlite3
import sys
from datetime import datetime

import pytest

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.append(os.path.join(REPO, "dev-cycle-protocol", "tests"))

from _engine import run_engine  # noqa: E402

SYNCED_AT = 1761998400.0  # 2025-11-01 12:00:00 UTC


def run_stats(folder):
    return run_engine("folder-sync", "stats", ["--dir", str(folder)])


def write_state(brain, rows):
    """The `file_hashes` table as the sync creates and fills it."""
    conn = sqlite3.connect(brain / "sync_state.sqlite")
    conn.execute("""
        CREATE TABLE file_hashes (
            file_path TEXT PRIMARY KEY, relative_path TEXT, sha256 TEXT, mtime REAL,
            file_size INTEGER, chunk_count INTEGER, last_synced_at REAL
        )
    """)
    conn.executemany("INSERT INTO file_hashes VALUES (?, ?, ?, ?, ?, ?, ?)", rows)
    conn.commit()
    conn.close()


def test_synced_index_shows_statistics(tmp_path):
    folder = tmp_path / "notes"
    brain = folder / ".folder_brain"
    brain.mkdir(parents=True)
    write_state(brain, [
        (str(folder / "a.md"), "a.md", "x" * 64, 1.0, 1048576, 3, SYNCED_AT - 60),
        (str(folder / "docs" / "b.pdf"), "docs/b.pdf", "y" * 64, 1.0, 2097152, 7, SYNCED_AT),
    ])

    result = run_stats(folder)

    assert result.returncode == 0, result.stderr
    last_sync = datetime.fromtimestamp(SYNCED_AT).strftime("%d/%m/%Y %H:%M:%S")
    assert "📄 Total indexed files: 2" in result.stdout
    assert "🧩 Total vector chunks: 10" in result.stdout
    assert "💾 Volume total de dados: 3.0 MB" in result.stdout
    assert f"🕒 Last sync: {last_sync}" in result.stdout
    assert result.stdout.index("- `docs/b.pdf` (7 chunks)") < result.stdout.index("- `a.md` (3 chunks)")


@pytest.mark.parametrize("brain", [False, True])
def test_folder_without_synced_index(tmp_path, brain):
    if brain:
        (tmp_path / ".folder_brain").mkdir()

    result = run_stats(tmp_path)

    assert result.returncode == 0, result.stderr
    assert result.stdout == f"ℹ️ Folder '{tmp_path.name}' has no synced `.folder_brain` index yet.\n"
