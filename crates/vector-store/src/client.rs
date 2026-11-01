//! The client calls the scripts make (`chromadb.PersistentClient` and `Collection`), with
//! the default embedding function.

use std::collections::{BinaryHeap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::compaction::Segments;
use crate::embed::Embedder;
use crate::hnsw::Space;
use crate::log::{self, Op, Record};
use crate::metadata::{self, Where};
use crate::{Metadata, StoreError, db, sysdb};

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Include {
    Documents,
    Metadatas,
}

#[derive(Debug, Clone, Default)]
pub struct GetResult {
    pub ids: Vec<String>,
    pub documents: Vec<Option<String>>,
    pub metadatas: Vec<Option<Metadata>>,
}

#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    pub ids: Vec<String>,
    pub documents: Vec<Option<String>>,
    pub metadatas: Vec<Option<Metadata>>,
    pub distances: Vec<f32>,
}

pub struct Client {
    root: PathBuf,
    conn: Connection,
    segments: Segments,
    backfilled: HashSet<String>,
    embedder: Option<Embedder>,
}

impl Client {
    /// `chromadb.PersistentClient(path=...)`.
    pub fn persistent(path: &Path) -> Result<Client, StoreError> {
        fs::create_dir_all(path)?;
        let conn = db::open(path)?;
        log::automatically_purge(&conn)?;
        Ok(Client {
            root: path.to_path_buf(),
            conn,
            segments: Segments::default(),
            backfilled: HashSet::new(),
            embedder: None,
        })
    }

    pub fn get_or_create_collection(
        &mut self,
        name: &str,
        metadata: Option<&Metadata>,
    ) -> Result<Collection, StoreError> {
        let c = sysdb::get_or_create(&self.conn, name, metadata)?;
        Ok(Collection {
            id: c.id,
            name: c.name,
        })
    }

    pub fn get_collection(&mut self, name: &str) -> Result<Collection, StoreError> {
        match sysdb::get_by_name(&self.conn, name)? {
            Some(c) => Ok(Collection {
                id: c.id,
                name: c.name,
            }),
            None => Err(StoreError::new(format!(
                "Collection [{name}] does not exist"
            ))),
        }
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, StoreError> {
        if self.embedder.is_none() {
            self.embedder = Some(Embedder::load()?);
        }
        self.embedder.as_ref().expect("embedder").embed(texts)
    }

    /// `try_backfill_collection`: once per process, before the first read.
    fn ensure_backfilled(&mut self, col: &Collection) -> Result<(), StoreError> {
        if self.backfilled.contains(&col.id) {
            return Ok(());
        }
        self.segments.backfill(&self.root, &self.conn, &col.id)?;
        self.segments.purge(&self.root, &self.conn, &col.id)?;
        self.backfilled.insert(col.id.clone());
        Ok(())
    }

    /// Writes the records to the log, then applies them.
    fn push(&mut self, col: &Collection, records: Vec<Record>) -> Result<(), StoreError> {
        log::push(&self.conn, &col.id, &records)?;
        self.segments.backfill(&self.root, &self.conn, &col.id)?;
        self.segments.purge(&self.root, &self.conn, &col.id)
    }

    /// `validate_embedding`: the first embeddings fix the collection's dimension.
    fn check_dimension(
        &mut self,
        col: &Collection,
        embeddings: &[Vec<f32>],
    ) -> Result<(), StoreError> {
        let Some(low) = embeddings.iter().map(Vec::len).min() else {
            return Ok(());
        };
        let high = embeddings.iter().map(Vec::len).max().unwrap_or(low);
        if low != high {
            return Err(StoreError::new(
                "All embeddings must have the same dimension",
            ));
        }
        let c = sysdb::get_by_id(&self.conn, &col.id)?;
        match c.dimension {
            Some(d) if d as usize != low => Err(StoreError::new(format!(
                "Collection expecting embedding with dimension of {d}, got {low}"
            ))),
            Some(_) => Ok(()),
            None => sysdb::set_dimension(&self.conn, &col.id, low as i64),
        }
    }

    pub fn get(
        &mut self,
        col: &Collection,
        ids: Option<&[String]>,
        whr: Option<&Where>,
        limit: Option<u32>,
        offset: u32,
        include: &[Include],
    ) -> Result<GetResult, StoreError> {
        self.ensure_backfilled(col)?;
        let c = sysdb::get_by_id(&self.conn, &col.id)?;
        let want_docs = include.contains(&Include::Documents);
        let want_meta = include.contains(&Include::Metadatas);
        let rows = metadata::get(
            &self.conn,
            &c.metadata_segment,
            ids,
            whr,
            offset,
            limit,
            want_docs || want_meta,
        )?;
        let mut out = GetResult::default();
        for r in rows {
            out.ids.push(r.id);
            if want_docs {
                out.documents.push(r.document);
            }
            if want_meta {
                out.metadatas.push(r.metadata);
            }
        }
        Ok(out)
    }

    pub fn count(&mut self, col: &Collection) -> Result<u64, StoreError> {
        self.ensure_backfilled(col)?;
        let c = sysdb::get_by_id(&self.conn, &col.id)?;
        metadata::count(&self.conn, &c.metadata_segment)
    }

    /// `add(ids, documents, metadatas)`, embedding the documents; an id already in the
    /// collection keeps its record.
    pub fn add(
        &mut self,
        col: &Collection,
        ids: &[String],
        documents: &[String],
        metadatas: &[Metadata],
    ) -> Result<(), StoreError> {
        unique(ids)?;
        let embeddings = self.embed(documents)?;
        self.check_dimension(col, &embeddings)?;
        let records = ids
            .iter()
            .enumerate()
            .map(|(i, id)| Record {
                id: id.clone(),
                op: Op::Add,
                embedding: Some(embeddings[i].clone()),
                metadata: metadatas.get(i).cloned(),
                document: documents.get(i).cloned(),
            })
            .collect();
        self.push(col, records)
    }

    /// `upsert(ids, documents, metadatas)`, embedding the documents.
    pub fn upsert(
        &mut self,
        col: &Collection,
        ids: &[String],
        documents: &[String],
        metadatas: &[Metadata],
    ) -> Result<(), StoreError> {
        unique(ids)?;
        let embeddings = self.embed(documents)?;
        self.upsert_embeddings(col, ids, &embeddings, documents, metadatas)
    }

    /// `upsert(ids, embeddings, documents, metadatas)`.
    pub fn upsert_embeddings(
        &mut self,
        col: &Collection,
        ids: &[String],
        embeddings: &[Vec<f32>],
        documents: &[String],
        metadatas: &[Metadata],
    ) -> Result<(), StoreError> {
        unique(ids)?;
        self.check_dimension(col, embeddings)?;
        let records = ids
            .iter()
            .enumerate()
            .map(|(i, id)| Record {
                id: id.clone(),
                op: Op::Upsert,
                embedding: Some(embeddings[i].clone()),
                metadata: metadatas.get(i).cloned(),
                document: documents.get(i).cloned(),
            })
            .collect();
        self.push(col, records)
    }

    /// `delete(ids=...)` or `delete(where=...)`.
    pub fn delete(
        &mut self,
        col: &Collection,
        ids: Option<&[String]>,
        whr: Option<&Where>,
    ) -> Result<(), StoreError> {
        let targets: Vec<String> = match whr {
            Some(w) => {
                self.ensure_backfilled(col)?;
                let c = sysdb::get_by_id(&self.conn, &col.id)?;
                metadata::get(
                    &self.conn,
                    &c.metadata_segment,
                    ids,
                    Some(w),
                    0,
                    None,
                    false,
                )?
                .into_iter()
                .map(|r| r.id)
                .collect()
            }
            None => ids.map(|i| i.to_vec()).unwrap_or_default(),
        };
        let records = targets
            .into_iter()
            .map(|id| Record {
                id,
                op: Op::Delete,
                embedding: None,
                metadata: None,
                document: None,
            })
            .collect();
        self.push(col, records)
    }

    /// `query(query_texts=[text], n_results, where)`: the nearest records, nearest first.
    pub fn query(
        &mut self,
        col: &Collection,
        text: &str,
        n_results: usize,
        whr: Option<&Where>,
    ) -> Result<QueryResult, StoreError> {
        let embedding = self.embed(&[text.to_string()])?.remove(0);
        self.query_embedding(col, embedding, n_results, whr)
    }

    /// `query(query_embeddings=[embedding], n_results, where)`.
    pub fn query_embedding(
        &mut self,
        col: &Collection,
        embedding: Vec<f32>,
        n_results: usize,
        whr: Option<&Where>,
    ) -> Result<QueryResult, StoreError> {
        self.ensure_backfilled(col)?;
        let c = sysdb::get_by_id(&self.conn, &col.id)?;
        let Some(dim) = c.dimension else {
            return Ok(QueryResult::default());
        };
        let allowed_ids: Vec<String> = match whr {
            None => Vec::new(),
            Some(w) => {
                let ids: Vec<String> = metadata::get(
                    &self.conn,
                    &c.metadata_segment,
                    None,
                    Some(w),
                    0,
                    None,
                    false,
                )?
                .into_iter()
                .map(|r| r.id)
                .collect();
                if ids.is_empty() {
                    return Ok(QueryResult::default());
                }
                ids
            }
        };
        let space = Space::parse(&c.hnsw_params().space);
        let Some(seg) = self
            .segments
            .reader(&self.root, &c, dim as usize, &self.conn)?
        else {
            return Err(StoreError::new("Nothing found on disk"));
        };
        let mut allowed = Vec::new();
        for id in &allowed_ids {
            match seg.id_map.id_to_label.get(id) {
                Some(&l) => allowed.push(l as u64),
                None => return Err(StoreError::new("Error finding id")),
            }
        }
        let q = if space == Space::Cosine {
            let n = embedding
                .iter()
                .map(|x| x * x)
                .sum::<f32>()
                .sqrt()
                .max(1e-32);
            embedding.iter().map(|x| x / n).collect()
        } else {
            embedding
        };
        let index = &seg.index;
        let found: Vec<(f32, u64)> = if index.is_empty() {
            Vec::new()
        } else {
            let with_deleted = index.cur_count;
            let deleted = (with_deleted - index.len()) as f32 / with_deleted as f32;
            if deleted > 0.2 && index.len() < 100 {
                let allow: HashSet<u64> = allowed.iter().copied().collect();
                let mut heap: BinaryHeap<(OrdF, u64)> = BinaryHeap::new();
                for label in index.labels().0 {
                    if !allow.is_empty() && !allow.contains(&label) {
                        continue;
                    }
                    let v = index
                        .get(label)
                        .ok_or_else(|| StoreError::new("Error querying knn"))?;
                    let d = space.distance(&v, &q);
                    if heap.len() < n_results {
                        heap.push((OrdF(d), label));
                    } else if heap.peek().is_some_and(|t| t.0.0 > d) {
                        heap.pop();
                        heap.push((OrdF(d), label));
                    }
                }
                heap.into_sorted_vec()
                    .into_iter()
                    .map(|(d, l)| (d.0, l))
                    .collect()
            } else {
                index.search(&q, n_results, &allowed)
            }
        };
        let mut out = QueryResult::default();
        for (d, label) in &found {
            let id = seg
                .id_map
                .label_to_id
                .get(&(*label as u32))
                .cloned()
                .ok_or_else(|| StoreError::new("Error finding id"))?;
            out.ids.push(id);
            out.distances.push(*d);
        }
        if !out.ids.is_empty() {
            let rows = metadata::get(
                &self.conn,
                &c.metadata_segment,
                Some(&out.ids),
                None,
                0,
                None,
                true,
            )?;
            for id in &out.ids {
                let row = rows.iter().find(|r| &r.id == id);
                out.documents.push(row.and_then(|r| r.document.clone()));
                out.metadatas.push(row.and_then(|r| r.metadata.clone()));
            }
        }
        Ok(out)
    }
}

fn unique(ids: &[String]) -> Result<(), StoreError> {
    let seen: HashSet<&String> = ids.iter().collect();
    if seen.len() != ids.len() {
        return Err(StoreError::new("Expected IDs to be unique"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF(f32);

impl Eq for OrdF {}

impl PartialOrd for OrdF {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdF {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
