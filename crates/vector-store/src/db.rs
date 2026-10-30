//! The SQLite file of the store (`chroma.sqlite3`): the connection settings and the
//! migrations, applied the way chromadb 1.5 applies them, so both read the same file.
//! The migration files are chromadb's own (Apache-2.0), copied byte for byte: their text
//! and MD5 hash are recorded in the `migrations` table and checked on every open.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};

use crate::StoreError;

pub const DB_FILE: &str = "chroma.sqlite3";

struct Migration {
    dir: &'static str,
    version: i64,
    filename: &'static str,
    sql: &'static str,
    hash: &'static str,
}

/// In the order chromadb applies them: the directories, then the versions.
const MIGRATIONS: &[Migration] = &[
    Migration {
        dir: "sysdb",
        version: 1,
        filename: "00001-collections.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00001-collections.sqlite.sql"),
        hash: "38352d725ad1c16074fac420b22b4633",
    },
    Migration {
        dir: "sysdb",
        version: 2,
        filename: "00002-segments.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00002-segments.sqlite.sql"),
        hash: "2913cb6a503055a95f625448037e8912",
    },
    Migration {
        dir: "sysdb",
        version: 3,
        filename: "00003-collection-dimension.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00003-collection-dimension.sqlite.sql"),
        hash: "42d22d0574d31d419c2a0e7f625c93aa",
    },
    Migration {
        dir: "sysdb",
        version: 4,
        filename: "00004-tenants-databases.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00004-tenants-databases.sqlite.sql"),
        hash: "048867ce8fcdefe4023c7110e4433591",
    },
    Migration {
        dir: "sysdb",
        version: 5,
        filename: "00005-remove-topic.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00005-remove-topic.sqlite.sql"),
        hash: "b1367c826b8fba5f96f27befdc1d42d2",
    },
    Migration {
        dir: "sysdb",
        version: 6,
        filename: "00006-collection-segment-metadata.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00006-collection-segment-metadata.sqlite.sql"),
        hash: "4eea7468935bf25d4604a0fed2366116",
    },
    Migration {
        dir: "sysdb",
        version: 7,
        filename: "00007-collection-config.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00007-collection-config.sqlite.sql"),
        hash: "1c7e63bba346a42a18b6ab7f1c989bed",
    },
    Migration {
        dir: "sysdb",
        version: 8,
        filename: "00008-maintenance-log.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00008-maintenance-log.sqlite.sql"),
        hash: "0a0e7e93111a01789addf64961c6127c",
    },
    Migration {
        dir: "sysdb",
        version: 9,
        filename: "00009-segment-collection-not-null.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00009-segment-collection-not-null.sqlite.sql"),
        hash: "054355aef9e63702bf54ea29e61563f1",
    },
    Migration {
        dir: "sysdb",
        version: 10,
        filename: "00010-collection-schema.sqlite.sql",
        sql: include_str!("../migrations/sysdb/00010-collection-schema.sqlite.sql"),
        hash: "5c3a5ac4b79df76799b4721827ed5e1d",
    },
    Migration {
        dir: "metadb",
        version: 1,
        filename: "00001-embedding-metadata.sqlite.sql",
        sql: include_str!("../migrations/metadb/00001-embedding-metadata.sqlite.sql"),
        hash: "2b4cf52c4bb2676e21d6860a4409f856",
    },
    Migration {
        dir: "metadb",
        version: 2,
        filename: "00002-embedding-metadata.sqlite.sql",
        sql: include_str!("../migrations/metadb/00002-embedding-metadata.sqlite.sql"),
        hash: "12a570f7121b3a8ce750a2a7c36da20f",
    },
    Migration {
        dir: "metadb",
        version: 3,
        filename: "00003-full-text-tokenize.sqlite.sql",
        sql: include_str!("../migrations/metadb/00003-full-text-tokenize.sqlite.sql"),
        hash: "f97ad6334aeaa8f419f01110b648b97a",
    },
    Migration {
        dir: "metadb",
        version: 4,
        filename: "00004-metadata-indices.sqlite.sql",
        sql: include_str!("../migrations/metadb/00004-metadata-indices.sqlite.sql"),
        hash: "fb36603a45ee2cd0254cef3ef86585e8",
    },
    Migration {
        dir: "metadb",
        version: 5,
        filename: "00005-max-seq-id-int.sqlite.sql",
        sql: include_str!("../migrations/metadb/00005-max-seq-id-int.sqlite.sql"),
        hash: "0e9de46758761b373ce682925edcc326",
    },
    Migration {
        dir: "metadb",
        version: 6,
        filename: "00006-metadata-array-support.sqlite.sql",
        sql: include_str!("../migrations/metadb/00006-metadata-array-support.sqlite.sql"),
        hash: "e026f01ea92c1baa1493f4ad5ca7cfe7",
    },
    Migration {
        dir: "embeddings_queue",
        version: 1,
        filename: "00001-embeddings.sqlite.sql",
        sql: include_str!("../migrations/embeddings_queue/00001-embeddings.sqlite.sql"),
        hash: "d3755dfd232be8e8301f4d7fcfb3a486",
    },
    Migration {
        dir: "embeddings_queue",
        version: 2,
        filename: "00002-embeddings-queue-config.sqlite.sql",
        sql: include_str!(
            "../migrations/embeddings_queue/00002-embeddings-queue-config.sqlite.sql"
        ),
        hash: "8fbfe4ffb3e57f1d8bfdc58510a82e85",
    },
];

/// Opens `<path>/chroma.sqlite3`, creating it when missing, and applies the migrations
/// it lacks.
pub fn open(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open(path.join(DB_FILE))?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.pragma_update(None, "case_sensitive_like", "ON")?;
    conn.busy_timeout(Duration::from_secs(1000))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (
                dir TEXT NOT NULL,
                version INTEGER NOT NULL,
                filename TEXT NOT NULL,
                sql TEXT NOT NULL,
                hash TEXT NOT NULL,
                PRIMARY KEY (dir, version)
            );
         CREATE TABLE IF NOT EXISTS acquire_write (
                id INTEGER PRIMARY KEY,
                lock_status INTEGER NOT NULL
            );
         INSERT INTO acquire_write (lock_status) VALUES (TRUE);",
    )?;
    let mut unapplied = Vec::new();
    for dir in ["sysdb", "metadb", "embeddings_queue"] {
        let source: Vec<&Migration> = MIGRATIONS.iter().filter(|m| m.dir == dir).collect();
        let mut stmt = conn.prepare(
            "SELECT dir, version, filename, hash FROM migrations WHERE dir = ?1 ORDER BY version ASC",
        )?;
        let applied = stmt
            .query_map([dir], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for ((d, version, filename, hash), m) in applied.iter().zip(&source) {
            if *version != m.version || d != m.dir || filename != m.filename {
                return Err(StoreError::new(format!(
                    "Inconsistent version: db {d}/{version}/{filename}, source {}/{}/{}",
                    m.dir, m.version, m.filename
                )));
            }
            if hash != m.hash {
                return Err(StoreError::new(format!(
                    "Inconsistent hash: db {hash}, source {}",
                    m.hash
                )));
            }
        }
        if applied.len() > source.len() {
            return Err(StoreError::new(format!(
                "Migrations in the database are newer than this version: {dir}"
            )));
        }
        unapplied.extend(source.into_iter().skip(applied.len()));
    }
    if unapplied.is_empty() {
        return Ok(());
    }
    conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<(), StoreError> {
        for m in unapplied {
            conn.execute_batch("PRAGMA foreign_keys = ON")?;
            conn.execute_batch(m.sql)?;
            conn.execute(
                "INSERT INTO migrations (dir, version, filename, sql, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![m.dir, m.version, m.filename, m.sql, m.hash],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    Ok(())
}

/// `max_seq_id` of a segment, 0 when it has none.
pub fn max_seq_id(conn: &Connection, segment_id: &str) -> Result<u64, StoreError> {
    let v: Option<i64> = conn
        .query_row(
            "SELECT seq_id FROM max_seq_id WHERE segment_id = ?1",
            [segment_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.unwrap_or(0) as u64)
}
