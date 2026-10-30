//! The write-ahead log (`embeddings_queue`): every write is a log record first, applied
//! to the segments right after.

use rusqlite::{Connection, OptionalExtension, params};

use crate::{DOCUMENT_KEY, MetaValue, Metadata, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Update,
    Upsert,
    Delete,
    BackfillFn,
}

impl Op {
    fn code(self) -> i64 {
        match self {
            Op::Add => 0,
            Op::Update => 1,
            Op::Upsert => 2,
            Op::Delete => 3,
            Op::BackfillFn => 4,
        }
    }

    fn from_code(code: i64) -> Result<Op, StoreError> {
        Ok(match code {
            0 => Op::Add,
            1 => Op::Update,
            2 => Op::Upsert,
            3 => Op::Delete,
            4 => Op::BackfillFn,
            _ => return Err(StoreError::new("Invalid operation code")),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Record {
    pub id: String,
    pub op: Op,
    pub embedding: Option<Vec<f32>>,
    pub metadata: Option<Metadata>,
    pub document: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub seq_id: i64,
    pub record: Record,
}

pub fn topic(collection_id: &str) -> String {
    format!("persistent://default/default/{collection_id}")
}

/// `push_logs`: appends the records in one transaction.
pub fn push(conn: &Connection, collection_id: &str, records: &[Record]) -> Result<(), StoreError> {
    if records.is_empty() {
        return Ok(());
    }
    let topic = topic(collection_id);
    conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<(), StoreError> {
        let mut stmt = conn.prepare(
            "INSERT INTO embeddings_queue (topic, id, operation, vector, encoding, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for r in records {
            let mut map = serde_json::Map::new();
            for (k, v) in r.metadata.iter().flatten() {
                map.insert(k.clone(), v.to_json());
            }
            if let Some(doc) = &r.document {
                map.insert(
                    DOCUMENT_KEY.to_string(),
                    serde_json::Value::String(doc.clone()),
                );
            }
            let vector: Option<Vec<u8>> = r
                .embedding
                .as_ref()
                .map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect());
            let encoding = r.embedding.as_ref().map(|_| "FLOAT32");
            stmt.execute(params![
                topic,
                r.id,
                r.op.code(),
                vector,
                encoding,
                serde_json::to_string(&serde_json::Value::Object(map))?
            ])?;
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

/// The records after `offset`, oldest first.
pub fn read(
    conn: &Connection,
    collection_id: &str,
    offset: i64,
) -> Result<Vec<LogRecord>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT seq_id, id, operation, vector, encoding, metadata FROM embeddings_queue \
         WHERE topic = ?1 AND seq_id > ?2 ORDER BY seq_id ASC",
    )?;
    let rows = stmt.query_map(params![topic(collection_id), offset], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<Vec<u8>>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (seq_id, id, op, vector, encoding, metadata) = row?;
        let embedding = match (vector, encoding.as_deref()) {
            (Some(bytes), Some("FLOAT32")) => {
                if bytes.len() % 4 != 0 {
                    return Err(StoreError::new("Invalid embedding"));
                }
                Some(
                    bytes
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect(),
                )
            }
            (Some(_), Some(other)) => {
                return Err(StoreError::new(format!("Unsupported encoding {other}")));
            }
            _ => None,
        };
        let (metadata, document) = match metadata {
            Some(text) => {
                let parsed: serde_json::Value = serde_json::from_str(&text)?;
                let mut meta = Metadata::new();
                let mut document = None;
                if let serde_json::Value::Object(map) = parsed {
                    for (k, v) in map {
                        if k == DOCUMENT_KEY {
                            match v {
                                serde_json::Value::String(s) => document = Some(s),
                                _ => return Err(StoreError::new("Document not found in metadata")),
                            }
                        } else {
                            meta.push((k, MetaValue::from_json(&v)?));
                        }
                    }
                }
                (Some(meta), document)
            }
            None => (None, None),
        };
        out.push(LogRecord {
            seq_id,
            record: Record {
                id,
                op: Op::from_code(op)?,
                embedding,
                metadata,
                document,
            },
        });
    }
    Ok(out)
}

/// `automatically_purge` of the legacy queue configuration, recorded on first use.
pub fn automatically_purge(conn: &Connection) -> Result<bool, StoreError> {
    let row: Option<String> = conn
        .query_row(
            "SELECT config_json_str FROM embeddings_queue_config",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(text) = row {
        let config: serde_json::Value = serde_json::from_str(&text)?;
        return Ok(config
            .get("automatically_purge")
            .and_then(|v| v.as_bool())
            .unwrap_or(true));
    }
    let size: i64 = conn.query_row("SELECT COUNT(*) from embeddings_queue", [], |r| r.get(0))?;
    let purge = size == 0;
    let text = format!(
        "{{\"automatically_purge\":{purge},\"_type\":\"EmbeddingsQueueConfigurationInternal\"}}"
    );
    conn.execute(
        "INSERT INTO embeddings_queue_config (config_json_str) VALUES (?1)",
        [text],
    )?;
    Ok(purge)
}

/// Deletes the records every segment has applied.
pub fn purge(conn: &Connection, collection_id: &str, max_seq_id: u64) -> Result<(), StoreError> {
    if !automatically_purge(conn)? {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM embeddings_queue WHERE topic = ?1 AND seq_id < ?2",
        params![topic(collection_id), max_seq_id as i64],
    )?;
    Ok(())
}
