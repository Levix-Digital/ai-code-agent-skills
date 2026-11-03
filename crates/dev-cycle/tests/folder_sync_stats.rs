//! `/folder-stats` reads the sync state of a folder's index (BUG-005: it failed on any synced
//! index because the last sync time could not be formatted). Formerly
//! `folder-sync/tests/test_stats.py`.

mod common;

use chrono::TimeZone;
use common::{Project, has, run_in};

/// 2025-11-01 12:00:00 UTC.
const SYNCED_AT: f64 = 1761998400.0;

/// The `file_hashes` table as the sync creates and fills it.
fn write_state(brain: &std::path::Path, rows: &[(String, &str, String, f64, i64, i64, f64)]) {
    let conn = rusqlite::Connection::open(brain.join("sync_state.sqlite")).expect("sqlite");
    conn.execute(
        "CREATE TABLE file_hashes (
            file_path TEXT PRIMARY KEY, relative_path TEXT, sha256 TEXT, mtime REAL,
            file_size INTEGER, chunk_count INTEGER, last_synced_at REAL
        )",
        [],
    )
    .expect("table");
    for r in rows {
        conn.execute(
            "INSERT INTO file_hashes VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![r.0, r.1, r.2, r.3, r.4, r.5, r.6],
        )
        .expect("row");
    }
}

#[test]
fn synced_index_shows_statistics() {
    let p = Project::new();
    let folder = p.path("notes");
    let brain = folder.join(".folder_brain");
    std::fs::create_dir_all(&brain).expect("folders");
    let path = |rel: &str| folder.join(rel).to_string_lossy().into_owned();
    write_state(
        &brain,
        &[
            (
                path("a.md"),
                "a.md",
                "x".repeat(64),
                1.0,
                1048576,
                3,
                SYNCED_AT - 60.0,
            ),
            (
                path("docs/b.pdf"),
                "docs/b.pdf",
                "y".repeat(64),
                1.0,
                2097152,
                7,
                SYNCED_AT,
            ),
        ],
    );

    let dir = folder.to_string_lossy().into_owned();
    let res = run_in(&p.dir, "folder-sync", "stats", &["--dir", &dir]);

    assert!(res.ok(), "{}", res.stderr);
    let last_sync = chrono::Local
        .timestamp_opt(SYNCED_AT as i64, 0)
        .unwrap()
        .format("%d/%m/%Y %H:%M:%S")
        .to_string();
    has(
        &res.stdout,
        &[
            "📄 Total indexed files: 2",
            "🧩 Total vector chunks: 10",
            "💾 Volume total de dados: 3.0 MB",
            &format!("🕒 Last sync: {last_sync}"),
        ],
    );
    let b = res.stdout.find("- `docs/b.pdf` (7 chunks)").expect("b.pdf");
    let a = res.stdout.find("- `a.md` (3 chunks)").expect("a.md");
    assert!(b < a);
}

fn folder_without_synced_index(brain: bool) {
    let p = Project::new();
    if brain {
        p.mkdir(".folder_brain");
    }
    let root = p.root();
    let res = p.run("folder-sync", "stats", &["--dir", &root]);
    assert!(res.ok(), "{}", res.stderr);
    let name = p.dir.file_name().unwrap().to_string_lossy();
    // Windows writes the line ending as \r\n, as Python's print did.
    assert_eq!(
        res.stdout.replace("\r\n", "\n"),
        format!("ℹ️ Folder '{name}' has no synced `.folder_brain` index yet.\n")
    );
}

#[test]
fn folder_without_index() {
    folder_without_synced_index(false);
}

#[test]
fn folder_with_an_empty_index_folder() {
    folder_without_synced_index(true);
}
