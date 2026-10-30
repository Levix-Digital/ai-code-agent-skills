//! Collections and their segments (`collections`, `collection_metadata`, `segments`).

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value as J;

use crate::{MetaValue, Metadata, StoreError};

pub const DEFAULT_TENANT: &str = "default_tenant";
pub const DEFAULT_DATABASE: &str = "default_database";
pub const VECTOR_SEGMENT_TYPE: &str = "urn:chroma:segment/vector/hnsw-local-persisted";
pub const METADATA_SEGMENT_TYPE: &str = "urn:chroma:segment/metadata/sqlite";

/// The schema chromadb 1.5 gives a new collection with the default embedding function.
pub const DEFAULT_SCHEMA: &str = r##"{"defaults":{"string":{"fts_index":{"enabled":false,"config":{}},"string_inverted_index":{"enabled":true,"config":{}}},"float_list":{"vector_index":{"enabled":false,"config":{"space":"l2","embedding_function":{"type":"known","name":"default","config":{}},"hnsw":{"ef_construction":100,"max_neighbors":16,"ef_search":100,"num_threads":4,"batch_size":100,"sync_threshold":1000,"resize_factor":1.2}}}},"sparse_vector":{"sparse_vector_index":{"enabled":false,"config":{"embedding_function":{"type":"unknown"},"bm25":false}}},"int":{"int_inverted_index":{"enabled":true,"config":{}}},"float":{"float_inverted_index":{"enabled":true,"config":{}}},"bool":{"bool_inverted_index":{"enabled":true,"config":{}}}},"keys":{"#document":{"string":{"fts_index":{"enabled":true,"config":{}},"string_inverted_index":{"enabled":false,"config":{}}}},"#embedding":{"float_list":{"vector_index":{"enabled":true,"config":{"space":"l2","embedding_function":{"type":"known","name":"default","config":{}},"source_key":"#document","hnsw":{"ef_construction":100,"max_neighbors":16,"ef_search":100,"num_threads":4,"batch_size":100,"sync_threshold":1000,"resize_factor":1.2}}}}}}}"##;

#[derive(Debug, Clone)]
pub struct CollectionRecord {
    pub id: String,
    pub name: String,
    pub dimension: Option<i64>,
    pub metadata: Metadata,
    pub schema: Option<J>,
    pub vector_segment: String,
    pub metadata_segment: String,
}

/// The HNSW settings of a collection's vector segment.
#[derive(Debug, Clone, PartialEq)]
pub struct HnswParams {
    pub space: String,
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub sync_threshold: usize,
}

impl CollectionRecord {
    /// The schema's `#embedding` settings; collections without a schema (older stores)
    /// keep theirs in the `hnsw:*` collection metadata.
    pub fn hnsw_params(&self) -> HnswParams {
        let mut p = HnswParams {
            space: "l2".into(),
            m: 16,
            ef_construction: 100,
            ef_search: 100,
            sync_threshold: 1000,
        };
        let config = self.schema.as_ref().and_then(|s| {
            s.pointer("/keys/#embedding/float_list/vector_index/config")
                .or_else(|| s.pointer("/defaults/float_list/vector_index/config"))
        });
        if let Some(config) = config {
            if let Some(space) = config.get("space").and_then(J::as_str) {
                p.space = space.to_string();
            }
            if let Some(h) = config.get("hnsw") {
                let num = |k: &str, d: usize| {
                    h.get(k)
                        .and_then(J::as_u64)
                        .map(|v| v as usize)
                        .unwrap_or(d)
                };
                p.m = num("max_neighbors", p.m);
                p.ef_construction = num("ef_construction", p.ef_construction);
                p.ef_search = num("ef_search", p.ef_search);
                p.sync_threshold = num("sync_threshold", p.sync_threshold);
            }
            return p;
        }
        for (k, v) in &self.metadata {
            match (k.as_str(), v) {
                ("hnsw:space", MetaValue::Str(s)) => p.space = s.clone(),
                ("hnsw:M", MetaValue::Int(i)) => p.m = *i as usize,
                ("hnsw:construction_ef", MetaValue::Int(i)) => p.ef_construction = *i as usize,
                ("hnsw:search_ef", MetaValue::Int(i)) => p.ef_search = *i as usize,
                ("hnsw:sync_threshold", MetaValue::Int(i)) => p.sync_threshold = *i as usize,
                _ => {}
            }
        }
        p
    }
}

fn database_id(conn: &Connection) -> Result<String, StoreError> {
    conn.query_row(
        "SELECT id FROM databases WHERE name = ?1 AND tenant_id = ?2",
        params![DEFAULT_DATABASE, DEFAULT_TENANT],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| StoreError::new(format!("Database {DEFAULT_DATABASE} not found")))
}

fn read_metadata(
    conn: &Connection,
    table: &str,
    id_col: &str,
    id: &str,
) -> Result<Metadata, StoreError> {
    let sql = format!(
        "SELECT key, str_value, int_value, float_value, bool_value FROM {table} WHERE {id_col} = ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([id], |r| {
        let key: String = r.get(0)?;
        let s: Option<String> = r.get(1)?;
        let i: Option<i64> = r.get(2)?;
        let f: Option<f64> = r.get(3)?;
        let b: Option<i64> = r.get(4)?;
        let v = if let Some(s) = s {
            MetaValue::Str(s)
        } else if let Some(i) = i {
            MetaValue::Int(i)
        } else if let Some(f) = f {
            MetaValue::Float(f)
        } else if let Some(b) = b {
            MetaValue::Bool(b != 0)
        } else {
            MetaValue::Null
        };
        Ok((key, v))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Loads a collection by name (`Some(name)`) or id.
fn load(
    conn: &Connection,
    by_name: bool,
    key: &str,
) -> Result<Option<CollectionRecord>, StoreError> {
    let db_id = database_id(conn)?;
    let sql = if by_name {
        "SELECT id, name, dimension, schema_str FROM collections WHERE name = ?1 AND database_id = ?2"
    } else {
        "SELECT id, name, dimension, schema_str FROM collections WHERE id = ?1 AND database_id = ?2"
    };
    let row = conn
        .query_row(sql, params![key, db_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .optional()?;
    let Some((id, name, dimension, schema_str)) = row else {
        return Ok(None);
    };
    let schema = match schema_str {
        Some(s) => Some(serde_json::from_str(&s)?),
        None => None,
    };
    let metadata = read_metadata(conn, "collection_metadata", "collection_id", &id)?;
    let segment = |scope: &str| -> Result<String, StoreError> {
        conn.query_row(
            "SELECT id FROM segments WHERE collection = ?1 AND scope = ?2",
            params![id, scope],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| StoreError::new(format!("Collection {id} has no {scope} segment")))
    };
    Ok(Some(CollectionRecord {
        vector_segment: segment("VECTOR")?,
        metadata_segment: segment("METADATA")?,
        id,
        name,
        dimension,
        metadata,
        schema,
    }))
}

pub fn get_by_name(conn: &Connection, name: &str) -> Result<Option<CollectionRecord>, StoreError> {
    load(conn, true, name)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<CollectionRecord, StoreError> {
    load(conn, false, id)?
        .ok_or_else(|| StoreError::new(format!("Collection {id} does not exist.")))
}

/// Upserts metadata rows; `Null` values delete their key.
pub fn write_metadata(
    conn: &Connection,
    table: &str,
    id_col: &str,
    id: &dyn rusqlite::ToSql,
    metadata: &Metadata,
    str_col: &str,
) -> Result<(), StoreError> {
    let deleted: Vec<&String> = metadata
        .iter()
        .filter(|(_, v)| matches!(v, MetaValue::Null))
        .map(|(k, _)| k)
        .collect();
    for key in deleted {
        conn.execute(
            &format!("DELETE FROM {table} WHERE {id_col} = ?1 AND key = ?2"),
            params![id, key],
        )?;
    }
    let sql = format!(
        "INSERT INTO {table} ({id_col}, key, {str_col}, int_value, float_value, bool_value) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT ({id_col}, key) DO UPDATE SET \
         {str_col} = excluded.{str_col}, int_value = excluded.int_value, \
         float_value = excluded.float_value, bool_value = excluded.bool_value"
    );
    for (key, v) in metadata {
        let (s, i, f, b): (Option<&str>, Option<i64>, Option<f64>, Option<bool>) = match v {
            MetaValue::Str(s) => (Some(s), None, None, None),
            MetaValue::Int(i) => (None, Some(*i), None, None),
            MetaValue::Float(f) => (None, None, Some(*f), None),
            MetaValue::Bool(b) => (None, None, None, Some(*b)),
            _ => continue,
        };
        conn.execute(&sql, params![id, key, s, i, f, b])?;
    }
    Ok(())
}

/// `create_collection(..., get_or_create=True)` with the default embedding function.
pub fn get_or_create(
    conn: &Connection,
    name: &str,
    metadata: Option<&Metadata>,
) -> Result<CollectionRecord, StoreError> {
    validate_name(name)?;
    conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<CollectionRecord, StoreError> {
        conn.execute(
            "UPDATE acquire_write SET lock_status = TRUE WHERE id = 1",
            [],
        )?;
        if let Some(existing) = get_by_name(conn, name)? {
            return Ok(existing);
        }
        let db_id = database_id(conn)?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO collections (id, name, config_json_str, schema_str, dimension, database_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, "{}", DEFAULT_SCHEMA, Option::<i64>::None, db_id],
        )?;
        if let Some(metadata) = metadata {
            write_metadata(
                conn,
                "collection_metadata",
                "collection_id",
                &id,
                metadata,
                "str_value",
            )?;
        }
        for (kind, scope) in [
            (VECTOR_SEGMENT_TYPE, "VECTOR"),
            (METADATA_SEGMENT_TYPE, "METADATA"),
        ] {
            conn.execute(
                "INSERT INTO segments (id, type, scope, collection) VALUES (?1, ?2, ?3, ?4)",
                params![uuid::Uuid::new_v4().to_string(), kind, scope, id],
            )?;
        }
        get_by_id(conn, &id)
    })();
    match result {
        Ok(c) => {
            conn.execute_batch("COMMIT")?;
            Ok(c)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// chromadb's collection name rule.
fn validate_name(name: &str) -> Result<(), StoreError> {
    let ok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    let n = name.chars().count();
    let ends = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    if (3..=512).contains(&n)
        && name.chars().all(ok_char)
        && ends(name.chars().next())
        && ends(name.chars().last())
    {
        return Ok(());
    }
    Err(StoreError::new(format!(
        "Validation error: name: Expected a name containing 3-512 characters from [a-zA-Z0-9._-], starting and ending with a character in [a-zA-Z0-9]. Got: {name}"
    )))
}

pub fn set_dimension(conn: &Connection, id: &str, dimension: i64) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE collections SET dimension = ?1 WHERE id = ?2",
        params![dimension, id],
    )?;
    Ok(())
}

pub fn update_schema(conn: &Connection, id: &str, schema: &J) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE collections SET schema_str = ?1 WHERE id = ?2",
        params![serde_json::to_string(schema)?, id],
    )?;
    Ok(())
}

/// `Schema.ensure_key_from_metadata`: gives a new key (or a new value type of a key) the
/// default index settings of its type. Returns whether the schema changed.
pub fn ensure_key(schema: &mut J, key: &str, value: &MetaValue) -> bool {
    if key.starts_with("chroma:") {
        return false;
    }
    let Some(type_key) = value.type_key() else {
        return false;
    };
    let default = schema.pointer(&format!("/defaults/{type_key}")).cloned();
    let Some(keys) = schema.get_mut("keys").and_then(J::as_object_mut) else {
        return false;
    };
    let entry = keys
        .entry(key.to_string())
        .or_insert_with(|| J::Object(Default::default()));
    let Some(types) = entry.as_object_mut() else {
        return false;
    };
    if types.get(type_key).is_some_and(|v| !v.is_null()) {
        return false;
    }
    match default {
        Some(d) if !d.is_null() => {
            types.insert(type_key.to_string(), d);
            true
        }
        _ => false,
    }
}
