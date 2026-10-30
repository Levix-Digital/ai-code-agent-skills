//! The local search index of the plans (`_store.py`'s index part and `_common.py`'s
//! `get_db_path`): a chromadb collection rebuilt from the files, so it can always be
//! dropped.

use std::path::PathBuf;

use kernel::out::err;
use kernel::py::{Crash, Value};
use vector_store::{Client, Collection, Include, MetaValue, Metadata, Where};

use crate::store::{Plan, PlanStore};

pub const INDEX_COLLECTION: &str = "plan_index";
const INDEX_DESCRIPTION: &str =
    "Local search index of plan files (docs/plans). Disposable: rebuilt from the files.";

pub fn store_crash(e: vector_store::StoreError) -> Crash {
    Crash::new(format!("chromadb.errors.ChromaError: {e}"))
}

/// `get_db_path()`: `$PLAN_MEMORY_DB`, else `~/.ai-code-agent-memory/plan-memory/db`.
pub fn get_db_path() -> PathBuf {
    match std::env::var_os("PLAN_MEMORY_DB") {
        Some(p) => PathBuf::from(p),
        None => vector_store::embed::home()
            .join(".ai-code-agent-memory")
            .join("plan-memory")
            .join("db"),
    }
}

/// `index_sha(plan)`: the file's hash chained with its Spec Kit artifacts.
pub fn index_sha(store: &PlanStore, plan: &Plan) -> Result<String, Crash> {
    let mut data = plan.sha256.clone().unwrap_or_default().into_bytes();
    for (rel, text) in store.spec_files(plan)? {
        data.extend(rel.as_bytes());
        data.extend(text.as_bytes());
    }
    Ok(kernel::sha256::hex(&data))
}

/// A metadata value as chromadb accepts it.
fn meta_value(v: &Value) -> Result<MetaValue, Crash> {
    Ok(match v {
        Value::Str(s) => MetaValue::Str(s.clone()),
        Value::Bool(b) => MetaValue::Bool(*b),
        Value::Int(i) => MetaValue::Int(
            i.parse()
                .map_err(|_| Crash::new("OverflowError: int too big to convert"))?,
        ),
        Value::Float(f) => MetaValue::Float(*f),
        Value::Null => MetaValue::Null,
        other => {
            return Err(Crash::new(format!(
                "ValueError: Expected metadata value to be a str, int, float, bool, SparseVector, or None, got {} which is a {}",
                other.py_repr(),
                other.type_name()
            )));
        }
    })
}

/// `str(value or "")`.
fn text_or_empty(v: &Value) -> String {
    if v.truthy() {
        v.py_str()
    } else {
        String::new()
    }
}

/// `index_metadata(plan, project)`.
pub fn index_metadata(plan: &Plan, project: &str) -> Result<Metadata, Crash> {
    let m = &plan.meta;
    let plan_id = m
        .get("plan_id")
        .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?;
    Ok(vec![
        ("project".into(), MetaValue::Str(project.to_string())),
        ("plan_id".into(), meta_value(plan_id)?),
        (
            "title".into(),
            MetaValue::Str(text_or_empty(m.get_or_none("title"))),
        ),
        (
            "version".into(),
            MetaValue::Str(text_or_empty(m.get_or_none("version"))),
        ),
        (
            "status".into(),
            MetaValue::Str(text_or_empty(m.get_or_none("status"))),
        ),
        ("kind".into(), meta_value(&plan.kind())?),
        (
            "created_at".into(),
            MetaValue::Str(text_or_empty(m.get_or_none("created_at"))),
        ),
        ("path".into(), MetaValue::Str(plan.path.clone())),
        (
            "sha256".into(),
            MetaValue::Str(plan.sha256.clone().unwrap_or_default()),
        ),
    ])
}

/// `PlanStore.collection()`.
pub fn collection() -> Result<(Client, Collection), Crash> {
    let db_path = get_db_path();
    kernel::py::path::makedirs(&db_path.to_string_lossy())?;
    let mut client = Client::persistent(&db_path).map_err(store_crash)?;
    let description = vec![(
        "description".to_string(),
        MetaValue::Str(INDEX_DESCRIPTION.into()),
    )];
    let col = client
        .get_or_create_collection(INDEX_COLLECTION, Some(&description))
        .map_err(store_crash)?;
    Ok((client, col))
}

/// A plan's id as a chromadb record id.
fn record_id(plan: &Plan) -> Result<String, Crash> {
    match plan.meta.get("plan_id") {
        Some(Value::Str(s)) => Ok(s.clone()),
        Some(other) => Err(Crash::new(format!(
            "ValueError: Expected ID to be a str, got {}",
            other.py_repr()
        ))),
        None => Err(Crash::new("KeyError: 'plan_id'")),
    }
}

/// `sync_index(plans, rebuild, quiet)`: brings the index in line with the plan files of
/// the project, and returns the open collection.
pub fn sync_index(
    store: &PlanStore,
    plans: Option<Vec<Plan>>,
    rebuild: bool,
    quiet: bool,
) -> Result<(Client, Collection), Crash> {
    let plans = match plans {
        Some(p) => p,
        None => store.load_all()?,
    };
    let (mut client, col) = collection()?;
    let project_filter = Where::eq("project", MetaValue::Str(store.project.clone()));
    let mut indexed = client
        .get(
            &col,
            None,
            Some(&project_filter),
            None,
            0,
            &[Include::Metadatas, Include::Documents],
        )
        .map_err(store_crash)?;
    if rebuild && !indexed.ids.is_empty() {
        client
            .delete(&col, Some(&indexed.ids), None)
            .map_err(store_crash)?;
        indexed = Default::default();
    }
    // `known`: id -> the indexed sha256, in first-seen order.
    let mut known: Vec<(String, Option<MetaValue>)> = Vec::new();
    for (i, id) in indexed.ids.iter().enumerate() {
        let sha = indexed
            .metadatas
            .get(i)
            .and_then(|m| m.as_ref())
            .and_then(|m| {
                m.iter()
                    .find(|(k, _)| k == "sha256")
                    .map(|(_, v)| v.clone())
            });
        match known.iter_mut().find(|(k, _)| k == id) {
            Some(entry) => entry.1 = sha,
            None => known.push((id.clone(), sha)),
        }
    }
    let mut shas: Vec<(String, String)> = Vec::new();
    for p in &plans {
        let id = record_id(p)?;
        let sha = index_sha(store, p)?;
        match shas.iter_mut().find(|(k, _)| *k == id) {
            Some(entry) => entry.1 = sha,
            None => shas.push((id, sha)),
        }
    }
    let sha_of = |id: &str| {
        shas.iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let mut changed: Vec<&Plan> = Vec::new();
    for p in &plans {
        let id = record_id(p)?;
        let current = known
            .iter()
            .find(|(k, _)| *k == id)
            .and_then(|(_, v)| v.clone());
        if current != Some(MetaValue::Str(sha_of(&id))) {
            changed.push(p);
        }
    }
    let plan_ids: Vec<String> = shas.iter().map(|(k, _)| k.clone()).collect();
    let stale: Vec<String> = known
        .iter()
        .filter(|(k, _)| !plan_ids.contains(k))
        .map(|(k, _)| k.clone())
        .collect();
    if !changed.is_empty() {
        let mut ids = Vec::new();
        let mut documents = Vec::new();
        let mut metadatas = Vec::new();
        for p in &changed {
            let id = record_id(p)?;
            documents.push(store.full_text(p)?);
            let mut meta = index_metadata(p, &store.project)?;
            for entry in meta.iter_mut() {
                if entry.0 == "sha256" {
                    entry.1 = MetaValue::Str(sha_of(&id));
                }
            }
            metadatas.push(meta);
            ids.push(id);
        }
        client
            .upsert(&col, &ids, &documents, &metadatas)
            .map_err(store_crash)?;
    }
    if !stale.is_empty() {
        client
            .delete(&col, Some(&stale), None)
            .map_err(store_crash)?;
    }
    if !quiet || !changed.is_empty() || !stale.is_empty() {
        err(&format!(
            "🔄 Plan index synced from {}: {} updated, {} removed, {} plan(s).\n",
            store.plans_dir,
            changed.len(),
            stale.len(),
            plans.len()
        ));
    }
    Ok((client, col))
}
