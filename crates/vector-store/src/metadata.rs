//! The metadata segment: the records' ids, metadata and documents in SQLite
//! (`embeddings`, `embedding_metadata`, `embedding_metadata_array` and the trigram index
//! `embedding_fulltext_search`), and the `where` filters over them.

use std::collections::BTreeMap;

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::Value as J;

use crate::log::{LogRecord, Op};
use crate::{DOCUMENT_KEY, MetaValue, Metadata, StoreError, sysdb};

/// A `where` filter.
#[derive(Debug, Clone, PartialEq)]
pub enum Where {
    And(Vec<Where>),
    Or(Vec<Where>),
    Cmp {
        key: String,
        op: CmpOp,
        value: MetaValue,
    },
    In {
        key: String,
        not: bool,
        values: Vec<MetaValue>,
    },
    Contains {
        key: String,
        not: bool,
        value: MetaValue,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl CmpOp {
    fn sql(self) -> &'static str {
        match self {
            CmpOp::Eq | CmpOp::Ne => "=",
            CmpOp::Gt => ">",
            CmpOp::Gte => ">=",
            CmpOp::Lt => "<",
            CmpOp::Lte => "<=",
        }
    }
}

impl Where {
    /// `{key: value}`.
    pub fn eq(key: &str, value: MetaValue) -> Where {
        Where::Cmp {
            key: key.to_string(),
            op: CmpOp::Eq,
            value,
        }
    }

    fn sql(&self, out: &mut Vec<Sql>) -> String {
        match self {
            Where::And(children) => {
                let mut s = "(1".to_string();
                for c in children {
                    s.push_str(" AND ");
                    s.push_str(&c.sql(out));
                }
                s + ")"
            }
            Where::Or(children) => {
                let mut s = "(0".to_string();
                for c in children {
                    s.push_str(" OR ");
                    s.push_str(&c.sql(out));
                }
                s + ")"
            }
            Where::Cmp { key, op, value } => {
                let not = if *op == CmpOp::Ne { "NOT " } else { "" };
                let subq = match value {
                    MetaValue::Int(_) | MetaValue::Float(_) => {
                        let (i, f) = match value {
                            MetaValue::Int(i) => (*i, *i as f64),
                            MetaValue::Float(f) => (*f as i64, *f),
                            _ => unreachable!(),
                        };
                        out.extend([
                            Sql::Text(key.clone()),
                            Sql::Integer(i),
                            Sql::Text(key.clone()),
                            Sql::Real(f),
                        ]);
                        format!(
                            "SELECT id FROM embedding_metadata WHERE key = ? AND key IS NOT NULL AND int_value {o} ? \
                             UNION SELECT id FROM embedding_metadata WHERE key = ? AND key IS NOT NULL AND float_value {o} ?",
                            o = op.sql()
                        )
                    }
                    other => {
                        let (col, v) = scalar(other);
                        out.extend([Sql::Text(key.clone()), v]);
                        format!(
                            "SELECT id FROM embedding_metadata WHERE key = ? AND key IS NOT NULL AND {col} {} ?",
                            op.sql()
                        )
                    }
                };
                format!("embeddings.id {not}IN ({subq})")
            }
            Where::In { key, not, values } => {
                let one = |values: Vec<MetaValue>, out: &mut Vec<Sql>| -> String {
                    let col = values
                        .first()
                        .map(|v| scalar(v).0)
                        .unwrap_or("string_value");
                    out.push(Sql::Text(key.clone()));
                    let marks: Vec<&str> = values.iter().map(|_| "?").collect();
                    out.extend(values.iter().map(|v| scalar(v).1));
                    format!(
                        "embeddings.id {}IN (SELECT id FROM embedding_metadata WHERE key = ? AND key IS NOT NULL AND {col} IN ({}))",
                        if *not { "NOT " } else { "" },
                        marks.join(", ")
                    )
                };
                let alt: Option<Vec<MetaValue>> = match values.first() {
                    Some(MetaValue::Int(_)) => Some(
                        values
                            .iter()
                            .map(|v| match v {
                                MetaValue::Int(i) => MetaValue::Float(*i as f64),
                                o => o.clone(),
                            })
                            .collect(),
                    ),
                    Some(MetaValue::Float(_)) => Some(
                        values
                            .iter()
                            .map(|v| match v {
                                MetaValue::Float(f) => MetaValue::Int(*f as i64),
                                o => o.clone(),
                            })
                            .collect(),
                    ),
                    _ => None,
                };
                let first = one(values.clone(), out);
                match alt {
                    Some(alt) => {
                        let second = one(alt, out);
                        format!("({first} {} {second})", if *not { "AND" } else { "OR" })
                    }
                    None => first,
                }
            }
            Where::Contains { key, not, value } => {
                let (col, v) = scalar(value);
                out.extend([Sql::Text(key.clone()), v]);
                format!(
                    "embeddings.id {}IN (SELECT id FROM embedding_metadata_array WHERE key = ? AND key IS NOT NULL AND {col} = ?)",
                    if *not { "NOT " } else { "" }
                )
            }
        }
    }
}

fn scalar(v: &MetaValue) -> (&'static str, Sql) {
    match v {
        MetaValue::Str(s) => ("string_value", Sql::Text(s.clone())),
        MetaValue::Int(i) => ("int_value", Sql::Integer(*i)),
        MetaValue::Float(f) => ("float_value", Sql::Real(*f)),
        MetaValue::Bool(b) => ("bool_value", Sql::Integer(*b as i64)),
        _ => ("string_value", Sql::Null),
    }
}

/// One record as `get` returns it.
#[derive(Debug, Clone)]
pub struct Row {
    pub offset_id: u32,
    pub id: String,
    pub document: Option<String>,
    pub metadata: Option<Metadata>,
}

/// `SqliteMetadataReader.get`: the matching records in insertion order.
pub fn get(
    conn: &Connection,
    segment_id: &str,
    ids: Option<&[String]>,
    whr: Option<&Where>,
    offset: u32,
    limit: Option<u32>,
    with_metadata: bool,
) -> Result<Vec<Row>, StoreError> {
    let mut args: Vec<Sql> = vec![Sql::Text(segment_id.to_string())];
    let mut filter = String::from("embeddings.segment_id = ?");
    if let Some(ids) = ids {
        let marks: Vec<&str> = ids.iter().map(|_| "?").collect();
        filter.push_str(&format!(
            " AND embeddings.embedding_id IN ({})",
            marks.join(", ")
        ));
        args.extend(ids.iter().map(|i| Sql::Text(i.clone())));
    }
    if let Some(w) = whr {
        filter.push_str(" AND ");
        filter.push_str(&w.sql(&mut args));
    }
    args.push(Sql::Integer(limit.unwrap_or(u32::MAX) as i64));
    args.push(Sql::Integer(offset as i64));
    let inner = format!(
        "SELECT DISTINCT embeddings.id, embeddings.embedding_id FROM embeddings WHERE {filter} \
         ORDER BY embeddings.id ASC LIMIT ? OFFSET ?"
    );
    let sql = if with_metadata {
        format!(
            "SELECT sub.id, sub.embedding_id, m.key, m.string_value, m.int_value, m.float_value, m.bool_value \
             FROM ({inner}) AS sub LEFT JOIN embedding_metadata m ON sub.id = m.id"
        )
    } else {
        format!("SELECT sub.id, sub.embedding_id FROM ({inner}) AS sub")
    };
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(args))?;
    let mut records: BTreeMap<u32, Row> = BTreeMap::new();
    while let Some(r) = rows.next()? {
        let offset_id: u32 = r.get(0)?;
        let id: String = r.get(1)?;
        let rec = records.entry(offset_id).or_insert(Row {
            offset_id,
            id,
            document: None,
            metadata: with_metadata.then(Metadata::new),
        });
        if !with_metadata {
            continue;
        }
        let Some(key) = r.get::<_, Option<String>>(2)? else {
            continue;
        };
        let v = if let Some(s) = r.get::<_, Option<String>>(3)? {
            MetaValue::Str(s)
        } else if let Some(i) = r.get::<_, Option<i64>>(4)? {
            MetaValue::Int(i)
        } else if let Some(f) = r.get::<_, Option<f64>>(5)? {
            MetaValue::Float(f)
        } else if let Some(b) = r.get::<_, Option<i64>>(6)? {
            MetaValue::Bool(b != 0)
        } else {
            continue;
        };
        if let Some(m) = rec.metadata.as_mut() {
            put(m, key, v);
        }
    }
    if with_metadata && !records.is_empty() {
        let marks: Vec<&str> = records.keys().map(|_| "?").collect();
        let sql = format!(
            "SELECT id, key, string_value, int_value, float_value, bool_value FROM embedding_metadata_array WHERE id IN ({})",
            marks.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(records.keys().map(|k| *k as i64)))?;
        while let Some(r) = rows.next()? {
            let offset_id: u32 = r.get(0)?;
            let key: String = r.get(1)?;
            let Some(m) = records
                .get_mut(&offset_id)
                .and_then(|rec| rec.metadata.as_mut())
            else {
                continue;
            };
            let pos = m.iter().position(|(k, _)| *k == key);
            let current = pos.map(|p| &mut m[p].1);
            macro_rules! push {
                ($variant:ident, $v:expr) => {{
                    match current {
                        Some(MetaValue::$variant(list)) => list.push($v),
                        _ => put(m, key, MetaValue::$variant(vec![$v])),
                    }
                }};
            }
            if let Some(s) = r.get::<_, Option<String>>(2)? {
                push!(StrList, s)
            } else if let Some(i) = r.get::<_, Option<i64>>(3)? {
                push!(IntList, i)
            } else if let Some(f) = r.get::<_, Option<f64>>(4)? {
                push!(FloatList, f)
            } else if let Some(b) = r.get::<_, Option<i64>>(5)? {
                push!(BoolList, b != 0)
            }
        }
    }
    Ok(records
        .into_values()
        .map(|mut rec| {
            if let Some(mut meta) = rec.metadata.take() {
                if let Some(p) = meta.iter().position(|(k, _)| k == DOCUMENT_KEY)
                    && let MetaValue::Str(doc) = meta.remove(p).1
                {
                    rec.document = Some(doc);
                }
                if !meta.is_empty() {
                    rec.metadata = Some(meta);
                }
            }
            rec
        })
        .collect())
}

fn put(m: &mut Metadata, key: String, v: MetaValue) {
    match m.iter().position(|(k, _)| *k == key) {
        Some(p) => m[p].1 = v,
        None => m.push((key, v)),
    }
}

pub fn count(conn: &Connection, segment_id: &str) -> Result<u64, StoreError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(id) FROM embeddings WHERE segment_id = ?1",
        [segment_id],
        |r| r.get(0),
    )?;
    Ok(n as u64)
}

/// `write_metadata`: scalars go to `embedding_metadata`, arrays to
/// `embedding_metadata_array`; a key moves between the two when its type does.
fn write_metadata(conn: &Connection, offset_id: i64, meta: &Metadata) -> Result<(), StoreError> {
    let mut scalars = Metadata::new();
    let mut arrays = Metadata::new();
    let mut deleted = Vec::new();
    for (k, v) in meta {
        if v.is_array() {
            arrays.push((k.clone(), v.clone()));
        } else {
            if matches!(v, MetaValue::Null) {
                deleted.push(k.clone());
            }
            put(&mut scalars, k.clone(), v.clone());
        }
    }
    for (k, _) in &arrays {
        put(&mut scalars, k.clone(), MetaValue::Null);
    }
    let mut array_keys_to_delete = deleted;
    for (k, v) in &scalars {
        if !matches!(v, MetaValue::Null) {
            array_keys_to_delete.push(k.clone());
        }
    }
    if !scalars.is_empty() {
        sysdb::write_metadata(
            conn,
            "embedding_metadata",
            "id",
            &offset_id,
            &scalars,
            "string_value",
        )?;
    }
    for k in &array_keys_to_delete {
        conn.execute(
            "DELETE FROM embedding_metadata_array WHERE id = ?1 AND key = ?2",
            params![offset_id, k],
        )?;
    }
    for (k, v) in &arrays {
        conn.execute(
            "DELETE FROM embedding_metadata_array WHERE id = ?1 AND key = ?2",
            params![offset_id, k],
        )?;
        let rows: Vec<Sql> = match v {
            MetaValue::StrList(l) => l.iter().map(|s| Sql::Text(s.clone())).collect(),
            MetaValue::IntList(l) => l.iter().map(|i| Sql::Integer(*i)).collect(),
            MetaValue::FloatList(l) => l.iter().map(|f| Sql::Real(*f)).collect(),
            MetaValue::BoolList(l) => l.iter().map(|b| Sql::Integer(*b as i64)).collect(),
            _ => Vec::new(),
        };
        let col = match v {
            MetaValue::StrList(_) => "string_value",
            MetaValue::IntList(_) => "int_value",
            MetaValue::FloatList(_) => "float_value",
            _ => "bool_value",
        };
        for value in rows {
            conn.execute(
                &format!(
                    "INSERT INTO embedding_metadata_array (id, key, {col}) VALUES (?1, ?2, ?3)"
                ),
                params![offset_id, k, value],
            )?;
        }
    }
    Ok(())
}

fn delete_document(conn: &Connection, offset_id: i64) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM embedding_fulltext_search WHERE rowid = ?1",
        [offset_id],
    )?;
    Ok(())
}

fn add_document(conn: &Connection, offset_id: i64, doc: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO embedding_fulltext_search (rowid, string_value) VALUES (?1, ?2)",
        params![offset_id, doc],
    )?;
    Ok(())
}

/// `SqliteMetadataWriter.apply_logs`, inside the caller's transaction. New metadata keys
/// enter `schema`; returns whether it changed.
pub fn apply_logs(
    conn: &Connection,
    segment_id: &str,
    logs: &[LogRecord],
    mut schema: Option<&mut J>,
) -> Result<bool, StoreError> {
    if logs.is_empty() {
        return Ok(false);
    }
    let mut modified = false;
    let mut max_seq_id: i64 = 0;
    for log in logs {
        let r = &log.record;
        max_seq_id = max_seq_id.max(log.seq_id);
        let mut meta = r.metadata.clone();
        if let Some(doc) = &r.document {
            let mut m = meta.unwrap_or_default();
            put(
                &mut m,
                DOCUMENT_KEY.to_string(),
                MetaValue::Str(doc.clone()),
            );
            meta = Some(m);
        }
        let mut track = |meta: &Metadata| {
            if let Some(s) = schema.as_deref_mut() {
                for (k, v) in meta {
                    if sysdb::ensure_key(s, k, v) {
                        modified = true;
                    }
                }
            }
        };
        match r.op {
            Op::BackfillFn => continue,
            Op::Add => {
                let offset: Option<i64> = conn
                    .query_row(
                        "INSERT INTO embeddings (segment_id, embedding_id, seq_id) VALUES (?1, ?2, ?3) \
                         ON CONFLICT (segment_id, embedding_id) DO NOTHING RETURNING id",
                        params![segment_id, r.id, log.seq_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(offset) = offset {
                    if let Some(m) = &meta {
                        track(m);
                        write_metadata(conn, offset, m)?;
                    }
                    if let Some(doc) = &r.document {
                        add_document(conn, offset, doc)?;
                    }
                }
            }
            Op::Update => {
                let offset: Option<i64> = conn
                    .query_row(
                        "UPDATE embeddings SET seq_id = ?1 WHERE segment_id = ?2 AND embedding_id = ?3 RETURNING id",
                        params![log.seq_id, segment_id, r.id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(offset) = offset {
                    if let Some(m) = &meta {
                        track(m);
                        write_metadata(conn, offset, m)?;
                    }
                    if let Some(doc) = &r.document {
                        delete_document(conn, offset)?;
                        add_document(conn, offset, doc)?;
                    }
                }
            }
            Op::Upsert => {
                let offset: i64 = conn.query_row(
                    "INSERT INTO embeddings (segment_id, embedding_id, seq_id) VALUES (?1, ?2, ?3) \
                     ON CONFLICT (segment_id, embedding_id) DO UPDATE SET seq_id = excluded.seq_id RETURNING id",
                    params![segment_id, r.id, log.seq_id],
                    |row| row.get(0),
                )?;
                if let Some(m) = &meta {
                    track(m);
                    write_metadata(conn, offset, m)?;
                }
                if let Some(doc) = &r.document {
                    delete_document(conn, offset)?;
                    add_document(conn, offset, doc)?;
                }
            }
            Op::Delete => {
                let offset: Option<i64> = conn
                    .query_row(
                        "DELETE FROM embeddings WHERE segment_id = ?1 AND embedding_id = ?2 RETURNING id",
                        params![segment_id, r.id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(offset) = offset {
                    conn.execute("DELETE FROM embedding_metadata WHERE id = ?1", [offset])?;
                    conn.execute(
                        "DELETE FROM embedding_metadata_array WHERE id = ?1",
                        [offset],
                    )?;
                    delete_document(conn, offset)?;
                }
            }
        }
    }
    conn.execute(
        "INSERT INTO max_seq_id (segment_id, seq_id) VALUES (?1, ?2) \
         ON CONFLICT (segment_id) DO UPDATE SET seq_id = excluded.seq_id",
        params![segment_id, max_seq_id],
    )?;
    Ok(modified)
}
