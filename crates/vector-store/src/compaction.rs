//! Applying the log to the segments (`LocalCompactionManager`): after every write, and
//! once per process before the first read of a collection. The vector segment lives in
//! memory and is written to its folder every `sync_threshold` applied records.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::hnsw::{DEFAULT_MAX_ELEMENTS, Hnsw, IdMap, METADATA_FILE, Space};
use crate::log::{self, LogRecord, Op};
use crate::sysdb::{self, CollectionRecord};
use crate::{StoreError, db, metadata};

/// An open vector segment (`LocalHnswIndex`).
pub struct VectorSegment {
    pub index: Hnsw,
    pub id_map: IdMap,
    dir: PathBuf,
    since_persist: u64,
    last_seen: u64,
    sync_threshold: usize,
}

impl VectorSegment {
    /// `LocalHnswSegmentReader.from_segment`: the persisted index, if any.
    fn read(
        root: &Path,
        c: &CollectionRecord,
        dim: usize,
        conn: &Connection,
    ) -> Result<Option<VectorSegment>, StoreError> {
        let dir = root.join(&c.vector_segment);
        let pickle = dir.join(METADATA_FILE);
        if !dir.exists() || !pickle.exists() {
            return Ok(None);
        }
        let id_map = IdMap::read(&pickle)?;
        if id_map.id_to_label.is_empty() {
            return Ok(None);
        }
        let p = c.hnsw_params();
        let index = Hnsw::load(&dir, dim, Space::parse(&p.space), p.ef_search)?;
        Ok(Some(VectorSegment {
            index,
            id_map,
            dir,
            since_persist: 0,
            last_seen: db::max_seq_id(conn, &c.vector_segment)?,
            sync_threshold: p.sync_threshold,
        }))
    }

    /// `LocalHnswSegmentWriter.from_segment`: the persisted index, or a new one whose
    /// files are created right away.
    fn write(
        root: &Path,
        c: &CollectionRecord,
        dim: usize,
        conn: &Connection,
    ) -> Result<VectorSegment, StoreError> {
        let dir = root.join(&c.vector_segment);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        let pickle = dir.join(METADATA_FILE);
        if pickle.exists() {
            let id_map = IdMap::read(&pickle)?;
            if !id_map.id_to_label.is_empty() {
                if let Some(legacy) = id_map.max_seq_id {
                    conn.execute(
                        "INSERT INTO max_seq_id (segment_id, seq_id) VALUES (?1, ?2) ON CONFLICT (segment_id) DO NOTHING",
                        params![c.vector_segment, legacy as i64],
                    )?;
                }
                if let Some(seg) = Self::read(root, c, dim, conn)? {
                    return Ok(seg);
                }
            }
        }
        let p = c.hnsw_params();
        let mut index = Hnsw::new(
            dim,
            Space::parse(&p.space),
            DEFAULT_MAX_ELEMENTS,
            p.m,
            p.ef_construction,
            0,
        );
        index.write_files(&dir)?;
        index.ef = p.ef_search;
        Ok(VectorSegment {
            index,
            id_map: IdMap::default(),
            dir,
            since_persist: 0,
            last_seen: 0,
            sync_threshold: p.sync_threshold,
        })
    }

    /// `apply_log_chunk`.
    fn apply(
        &mut self,
        conn: &Connection,
        segment_id: &str,
        logs: &[&LogRecord],
    ) -> Result<(), StoreError> {
        let mut next_label = self.id_map.total_elements_added + 1;
        if logs.is_empty() {
            return Ok(());
        }
        let mut max_seq_id: u64 = 0;
        let mut batch: BTreeMap<u32, Vec<&LogRecord>> = BTreeMap::new();
        for log in logs {
            if log.seq_id <= self.last_seen as i64 {
                continue;
            }
            self.since_persist += 1;
            max_seq_id = max_seq_id.max(log.seq_id as u64);
            let r = &log.record;
            match r.op {
                Op::BackfillFn => continue,
                Op::Add => {
                    if !self.id_map.id_to_label.contains_key(&r.id) {
                        if r.embedding.is_none() {
                            return Err(StoreError::new("Error applying log chunk"));
                        }
                        self.id_map.id_to_label.insert(r.id.clone(), next_label);
                        self.id_map.label_to_id.insert(next_label, r.id.clone());
                        batch.entry(next_label).or_default().push(log);
                        next_label += 1;
                    }
                }
                Op::Update => {
                    if let Some(&label) = self.id_map.id_to_label.get(&r.id)
                        && r.embedding.is_some()
                    {
                        batch.entry(label).or_default().push(log);
                    }
                }
                Op::Delete => {
                    if let Some(label) = self.id_map.id_to_label.remove(&r.id) {
                        self.id_map.label_to_id.remove(&label);
                        batch.entry(label).or_default().push(log);
                    }
                }
                Op::Upsert => {
                    let (label, new) = match self.id_map.id_to_label.get(&r.id) {
                        Some(&l) => (l, false),
                        None => (next_label, true),
                    };
                    if r.embedding.is_none() {
                        return Err(StoreError::new("Error applying log chunk"));
                    }
                    self.id_map.id_to_label.insert(r.id.clone(), label);
                    self.id_map.label_to_id.insert(label, r.id.clone());
                    batch.entry(label).or_default().push(log);
                    if new {
                        next_label += 1;
                    }
                }
            }
        }
        let len = self.index.cur_count;
        if len + batch.len() >= self.index.max_elements {
            self.index.resize((len + batch.len()).next_power_of_two())?;
        }
        for (label, records) in &batch {
            for log in records {
                match log.record.op {
                    Op::BackfillFn => {}
                    Op::Add | Op::Upsert | Op::Update => {
                        let e = log.record.embedding.as_ref().expect("embedding");
                        self.index
                            .add(*label as u64, e)
                            .map_err(|_| StoreError::new("Error applying log chunk"))?;
                    }
                    Op::Delete => {
                        self.index
                            .mark_delete(*label as u64)
                            .map_err(|_| StoreError::new("Error applying log chunk"))?;
                    }
                }
            }
        }
        self.id_map.total_elements_added = next_label - 1;
        if self.since_persist >= self.sync_threshold as u64 {
            self.index.persist(&self.dir)?;
            self.id_map.write(&self.dir.join(METADATA_FILE))?;
            conn.execute(
                "INSERT OR REPLACE INTO max_seq_id (segment_id, seq_id) VALUES (?1, ?2)",
                params![segment_id, max_seq_id as i64],
            )?;
            self.since_persist = 0;
        }
        self.last_seen = max_seq_id;
        Ok(())
    }
}

/// The open vector segments of the process (`LocalSegmentManager`'s pool).
#[derive(Default)]
pub struct Segments {
    pool: HashMap<String, VectorSegment>,
}

impl Segments {
    /// The vector segment of a collection, opened for reading: `None` when nothing is
    /// on disk and nothing was written by this process.
    pub fn reader(
        &mut self,
        root: &Path,
        c: &CollectionRecord,
        dim: usize,
        conn: &Connection,
    ) -> Result<Option<&mut VectorSegment>, StoreError> {
        if !self.pool.contains_key(&c.vector_segment) {
            match VectorSegment::read(root, c, dim, conn)? {
                Some(seg) => {
                    self.pool.insert(c.vector_segment.clone(), seg);
                }
                None => return Ok(None),
            }
        }
        Ok(self.pool.get_mut(&c.vector_segment))
    }

    fn writer(
        &mut self,
        root: &Path,
        c: &CollectionRecord,
        dim: usize,
        conn: &Connection,
    ) -> Result<&mut VectorSegment, StoreError> {
        if !self.pool.contains_key(&c.vector_segment) {
            let seg = VectorSegment::write(root, c, dim, conn)?;
            self.pool.insert(c.vector_segment.clone(), seg);
        }
        Ok(self.pool.get_mut(&c.vector_segment).expect("segment"))
    }

    /// The vector segment's `max_seq_id`, 0 while it has never been written.
    fn hnsw_max_seq_id(
        &mut self,
        root: &Path,
        c: &CollectionRecord,
        dim: usize,
        conn: &Connection,
    ) -> Result<u64, StoreError> {
        match self.reader(root, c, dim, conn)? {
            Some(_) => db::max_seq_id(conn, &c.vector_segment),
            None => Ok(0),
        }
    }

    /// `BackfillMessage`: applies the unapplied log records to both segments.
    pub fn backfill(
        &mut self,
        root: &Path,
        conn: &Connection,
        collection_id: &str,
    ) -> Result<(), StoreError> {
        let c = sysdb::get_by_id(conn, collection_id)?;
        let schema_persisted = c.schema.is_some();
        let Some(dim) = c.dimension else {
            return Ok(());
        };
        let dim = dim as usize;
        let mt_max = db::max_seq_id(conn, &c.metadata_segment)?;
        let hnsw_max = self.hnsw_max_seq_id(root, &c, dim, conn)?;
        let logs = log::read(conn, &c.id, mt_max.min(hnsw_max) as i64)?;
        let mt_logs: Vec<LogRecord> = logs
            .iter()
            .filter(|l| l.seq_id > mt_max as i64)
            .cloned()
            .collect();
        let hnsw_logs: Vec<&LogRecord> =
            logs.iter().filter(|l| l.seq_id > hnsw_max as i64).collect();
        conn.execute_batch("BEGIN")?;
        let applied = (|| -> Result<(), StoreError> {
            let mut schema = c.schema.clone();
            let changed =
                metadata::apply_logs(conn, &c.metadata_segment, &mt_logs, schema.as_mut())?;
            if schema_persisted
                && changed
                && let Some(s) = &schema
            {
                sysdb::update_schema(conn, &c.id, s)?;
            }
            Ok(())
        })();
        match applied {
            Ok(()) => conn.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
        let seg = self.writer(root, &c, dim, conn)?;
        seg.apply(conn, &c.vector_segment, &hnsw_logs)
    }

    /// `PurgeLogsMessage`: drops the log records both segments have applied.
    pub fn purge(
        &mut self,
        root: &Path,
        conn: &Connection,
        collection_id: &str,
    ) -> Result<(), StoreError> {
        let c = sysdb::get_by_id(conn, collection_id)?;
        let Some(dim) = c.dimension else {
            return Ok(());
        };
        let mt_max = db::max_seq_id(conn, &c.metadata_segment)?;
        let hnsw_max = self.hnsw_max_seq_id(root, &c, dim as usize, conn)?;
        log::purge(conn, &c.id, mt_max.min(hnsw_max))
    }

    pub fn segment(&mut self, vector_segment: &str) -> Option<&mut VectorSegment> {
        self.pool.get_mut(vector_segment)
    }
}
