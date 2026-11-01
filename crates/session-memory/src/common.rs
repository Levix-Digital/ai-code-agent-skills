//! `_common.py`: the store location, the collection and the project namespace, so every
//! record carries a `project` metadata field and project-scoped IDs.

use std::path::PathBuf;

use kernel::out::println;
use kernel::py::{Crash, Value};
use vector_store::{Client, Collection};

pub const COLLECTION_NAME: &str = "session_history";

/// The exception chromadb raised, as its last traceback line.
pub fn store_crash(e: vector_store::StoreError) -> Crash {
    Crash::new(format!("chromadb.errors.ChromaError: {e}"))
}

/// `resolve_project(explicit)`: `--project` > `SESSION_MEMORY_PROJECT` > the Git
/// repository root's name > the current folder's name, in kebab-case. `Err(1)` after
/// printing the error when nothing is left.
pub fn resolve_project(explicit: Option<&str>) -> Result<Result<String, u8>, Crash> {
    let env = std::env::var("SESSION_MEMORY_PROJECT").ok();
    let mut name = explicit
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .or_else(|| env.filter(|n| !n.is_empty()));
    if name.is_none() {
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output();
        name = Some(match out {
            Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
                Ok(top) => kernel::py::path::basename(std::path::Path::new(
                    kernel::py::num::strip(&kernel::py::universal_newlines(&top)),
                )),
                Err(_) => cwd_name(),
            },
            _ => cwd_name(),
        });
    }
    let project = plan_memory::store::normalize_project(&Value::str(name.unwrap_or_default()))?;
    if project.is_empty() {
        println("Error: could not resolve a project name. Pass --project <name>.");
        return Ok(Err(1));
    }
    Ok(Ok(project))
}

fn cwd_name() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    kernel::py::path::basename(std::path::Path::new(&cwd))
}

/// `get_db_path()`: `$SESSION_MEMORY_DB`, else `~/.ai-code-agent-memory/session-memory/db`.
pub fn get_db_path() -> PathBuf {
    match std::env::var_os("SESSION_MEMORY_DB") {
        Some(p) => PathBuf::from(p),
        None => vector_store::embed::home()
            .join(".ai-code-agent-memory")
            .join("session-memory")
            .join("db"),
    }
}

/// `get_collection()`.
pub fn get_collection() -> Result<(Client, Collection), Crash> {
    let db_path = get_db_path();
    kernel::py::path::makedirs(&db_path.to_string_lossy())?;
    let mut client = Client::persistent(&db_path).map_err(store_crash)?;
    let col = client
        .get_or_create_collection(COLLECTION_NAME, None)
        .map_err(store_crash)?;
    Ok((client, col))
}

/// `master_id(project)`.
pub fn master_id(project: &str) -> String {
    format!("{project}::master_summary")
}

/// `chunk_id(project, session_id, start_turn, end_turn)`.
pub fn chunk_id(project: &str, session_id: &str, start: i64, end: i64) -> String {
    format!("{project}::turns::{session_id}::{start:03}_to_{end:03}")
}
