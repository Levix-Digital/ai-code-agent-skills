//! `sync.py`: incremental sync of a folder into its `.folder_brain` index (a document-level
//! and a chunk-level collection), tracked by a SQLite table of file hashes.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::path::{join, realpath};
use kernel::py::{Crash, float_repr, io_crash, lower};
use rusqlite::types::Value;
use rusqlite::{Connection, params};
use vector_store::{Client, Collection, MetaValue, Metadata, Where};

use crate::extractors::{
    CLOUD_SHORTCUT_EXTENSIONS, SUPPORTED_EXTENSIONS, chunk_document, extract_text_from_file,
    is_ignored,
};
use crate::sqlite::{Cell, sqlite_crash};

pub fn main(argv: &[String]) -> u8 {
    let parser = Parser::new("sync.py", "Incremental folder sync into ChromaDB")
        .option("--dir", "Directory to sync (default: current)")
        .flag("--force", "Force a full reindex, ignoring the cache");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return exit.report(),
    };
    match sync_folder(args.value("dir").unwrap_or("."), args.flag("force")) {
        Ok(()) => 0,
        Err(c) => c.report(),
    }
}

fn store_crash(e: vector_store::StoreError) -> Crash {
    Crash::new(format!("chromadb.errors.ChromaError: {e}"))
}

fn now() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    }
}

/// `st_mtime`: seconds plus nanoseconds, as Python computes the float.
fn mtime(meta: &std::fs::Metadata) -> f64 {
    let Ok(t) = meta.modified() else {
        return 0.0;
    };
    let (secs, nanos) = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos()),
        Err(e) => {
            let d = e.duration();
            if d.subsec_nanos() == 0 {
                (-(d.as_secs() as i64), 0)
            } else {
                (-(d.as_secs() as i64) - 1, 1_000_000_000 - d.subsec_nanos())
            }
        }
    };
    secs as f64 + f64::from(nanos) * 1e-9
}

/// `PurePath.suffix`: from the last dot of the final component, unless the dot leads or
/// ends it.
fn suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => &name[i..],
        _ => "",
    }
}

fn separators(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// The parts of `path` below `root`.
fn rel_parts(path: &str, root: &str) -> Vec<String> {
    path.strip_prefix(root)
        .unwrap_or(path)
        .split(separators)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// `os.walk(top)`, top-down: each directory with the names of its non-directory entries.
/// Symbolic links to directories are listed as directories and not followed.
fn walk(top: &str, out: &mut Vec<(String, Vec<String>)>) {
    let Ok(rd) = std::fs::read_dir(top) else {
        return;
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in rd {
        let Ok(entry) = entry else {
            break;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = match entry.file_type() {
            Ok(t) if t.is_symlink() => std::fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()),
            Ok(t) => t.is_dir(),
            Err(_) => false,
        };
        if is_dir {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    out.push((top.to_string(), files));
    for d in dirs {
        let path = join(top, &d);
        let link = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink());
        if !link {
            walk(&path, out);
        }
    }
}

struct Current {
    key: String,
    path: PathBuf,
    name: String,
    ext: String,
    mtime: f64,
    size: u64,
}

struct Known {
    key: Cell,
    sha256: Cell,
    mtime: Cell,
    chunk_count: Cell,
}

/// A Python number that may turn into a float.
#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    fn add_cell(self, c: &Cell) -> Result<Num, Crash> {
        Ok(match (self, c) {
            (Num::Int(a), Cell::Int(b)) => Num::Int(a + b),
            (Num::Int(a), Cell::Real(b)) => Num::Float(a as f64 + b),
            (Num::Float(a), Cell::Int(b)) => Num::Float(a + *b as f64),
            (Num::Float(a), Cell::Real(b)) => Num::Float(a + b),
            (n, other) => {
                let left = match n {
                    Num::Int(_) => "int",
                    Num::Float(_) => "float",
                };
                return Err(Crash::new(format!(
                    "TypeError: unsupported operand type(s) for +=: '{left}' and '{}'",
                    other.type_name()
                )));
            }
        })
    }

    fn add(self, n: usize) -> Num {
        match self {
            Num::Int(a) => Num::Int(a + n as i64),
            Num::Float(a) => Num::Float(a + n as f64),
        }
    }

    fn text(self) -> String {
        match self {
            Num::Int(a) => a.to_string(),
            Num::Float(a) => float_repr(a),
        }
    }
}

fn cell_value(c: &Cell) -> Value {
    match c {
        Cell::Null => Value::Null,
        Cell::Int(i) => Value::Integer(*i),
        Cell::Real(f) => Value::Real(*f),
        Cell::Text(t) => Value::Text(t.clone()),
        Cell::Blob(b) => Value::Blob(b.clone()),
    }
}

/// `delete(where={"file_path": value})`, any error ignored as the script does.
fn delete_file(client: &mut Client, cols: [&Collection; 2], value: &Cell) {
    let v = match value {
        Cell::Text(t) => MetaValue::Str(t.clone()),
        Cell::Int(i) => MetaValue::Int(*i),
        Cell::Real(f) => MetaValue::Float(*f),
        Cell::Null | Cell::Blob(_) => return,
    };
    let whr = Where::eq("file_path", v);
    for col in cols {
        if client.delete(col, None, Some(&whr)).is_err() {
            return;
        }
    }
}

fn s(v: &str) -> MetaValue {
    MetaValue::Str(v.to_string())
}

fn int(v: usize) -> MetaValue {
    MetaValue::Int(v as i64)
}

fn sync_folder(target_dir: &str, force: bool) -> Result<(), Crash> {
    let root = realpath(target_dir);
    if !std::fs::metadata(&root).is_ok_and(|m| m.is_dir()) {
        println(&format!(
            "Error: directory '{target_dir}' does not exist or is not a valid folder."
        ));
        return Ok(());
    }

    let brain = join(&root, ".folder_brain");
    std::fs::create_dir_all(&brain).map_err(|e| io_crash(&e, Path::new(&brain)))?;
    let chroma_path = join(&brain, "chroma_db");

    println("\n🧠 [Folder Brain] Starting incremental sync in:");
    println(&format!("📂 {root}"));
    println(&format!("💾 ChromaDB local: {chroma_path}\n"));

    let mut client = Client::persistent(Path::new(&chroma_path)).map_err(store_crash)?;
    let describe = |d: &str| -> Metadata { vec![("description".to_string(), s(d))] };
    let macro_col = client
        .get_or_create_collection("folder_macro", Some(&describe("Document-level overviews")))
        .map_err(store_crash)?;
    let micro_col = client
        .get_or_create_collection(
            "folder_micro",
            Some(&describe("Paragraph-level micro chunks")),
        )
        .map_err(store_crash)?;

    let conn = Connection::open(join(&brain, "sync_state.sqlite")).map_err(sqlite_crash)?;
    conn.execute(
        "
        CREATE TABLE IF NOT EXISTS file_hashes (
            file_path TEXT PRIMARY KEY,
            relative_path TEXT,
            sha256 TEXT,
            mtime REAL,
            file_size INTEGER,
            chunk_count INTEGER,
            last_synced_at REAL
        )
    ",
        [],
    )
    .map_err(sqlite_crash)?;

    let known: Vec<Known> = {
        let mut stmt = conn
            .prepare("SELECT file_path, sha256, mtime, chunk_count FROM file_hashes")
            .map_err(sqlite_crash)?;
        stmt.query_map([], |r| {
            Ok(Known {
                key: Cell::from_ref(r.get_ref(0)?),
                sha256: Cell::from_ref(r.get_ref(1)?),
                mtime: Cell::from_ref(r.get_ref(2)?),
                chunk_count: Cell::from_ref(r.get_ref(3)?),
            })
        })
        .map_err(sqlite_crash)?
        .collect::<Result<_, _>>()
        .map_err(sqlite_crash)?
    };
    let find_known = |key: &str| {
        known
            .iter()
            .rev()
            .find(|k| matches!(&k.key, Cell::Text(t) if t == key))
    };

    let mut current: Vec<Current> = Vec::new();
    let mut cloud_files: Vec<String> = Vec::new();
    let mut tree = Vec::new();
    walk(&root, &mut tree);
    for (dir, files) in tree {
        if is_ignored(&rel_parts(&dir, &root)) {
            continue;
        }
        for f in files {
            let file_path = join(&dir, &f);
            if is_ignored(&rel_parts(&file_path, &root)) {
                continue;
            }
            let ext = lower(suffix(&f));
            if CLOUD_SHORTCUT_EXTENSIONS.contains(&ext.as_str()) {
                cloud_files.push(f);
                continue;
            }
            if SUPPORTED_EXTENSIONS.contains(&ext.as_str())
                && let Ok(meta) = std::fs::metadata(&file_path)
            {
                current.push(Current {
                    key: file_path.clone(),
                    path: PathBuf::from(&file_path),
                    name: f,
                    ext,
                    mtime: mtime(&meta),
                    size: meta.len(),
                });
            }
        }
    }

    let (mut new_count, mut modified_count, mut unchanged_count, mut deleted_count) = (0, 0, 0, 0);
    let mut total_chunks = Num::Int(0);
    let start_time = now();

    for f in &current {
        let rel_path = f
            .key
            .strip_prefix(&root)
            .unwrap_or(&f.key)
            .trim_start_matches(separators)
            .replace('\\', "/");
        let known_entry = find_known(&f.key);
        let is_known = known_entry.is_some();
        let mut is_modified = false;
        let sha = || -> Result<String, Crash> {
            let data = std::fs::read(&f.path).map_err(|e| io_crash(&e, &f.path))?;
            Ok(kernel::sha256::hex(&data))
        };

        let current_hash = match known_entry {
            Some(known) if !force => {
                let km = match &known.mtime {
                    Cell::Int(i) => *i as f64,
                    Cell::Real(r) => *r,
                    other => {
                        return Err(Crash::new(format!(
                            "TypeError: unsupported operand type(s) for -: '{}' and 'float'",
                            other.type_name()
                        )));
                    }
                };
                if (km - f.mtime).abs() < 0.001 {
                    unchanged_count += 1;
                    total_chunks = total_chunks.add_cell(&known.chunk_count)?;
                    continue;
                }
                let h = sha()?;
                if matches!(&known.sha256, Cell::Text(t) if *t == h) {
                    unchanged_count += 1;
                    total_chunks = total_chunks.add_cell(&known.chunk_count)?;
                    conn.execute(
                        "UPDATE file_hashes SET mtime = ? WHERE file_path = ?",
                        params![f.mtime, f.key],
                    )
                    .map_err(sqlite_crash)?;
                    continue;
                }
                is_modified = true;
                h
            }
            _ => sha()?,
        };

        let label = if is_modified {
            "Modified"
        } else if force && is_known {
            "Reindexing (force)"
        } else {
            "New"
        };
        println(&format!("  ⚡ [{label}] {rel_path}"));

        let Some(extracted) = extract_text_from_file(&f.path, &f.ext) else {
            continue;
        };
        let chunks = chunk_document(&extracted);

        if is_known || is_modified {
            delete_file(
                &mut client,
                [&macro_col, &micro_col],
                &Cell::Text(f.key.clone()),
            );
        }

        let preview: String = extracted.content.chars().take(1500).collect();
        let macro_id = format!("doc_{}", kernel::md5::hex(f.key.as_bytes()));
        client
            .add(
                &macro_col,
                std::slice::from_ref(&macro_id),
                &[format!(
                    "File: {rel_path}\nWord count: {}\nPreview:\n{preview}",
                    extracted.word_count
                )],
                &[vec![
                    ("file_path".to_string(), s(&f.key)),
                    ("relative_path".to_string(), s(&rel_path)),
                    ("file_name".to_string(), s(&f.name)),
                    ("file_type".to_string(), s(&f.ext)),
                    ("word_count".to_string(), int(extracted.word_count)),
                ]],
            )
            .map_err(store_crash)?;

        if !chunks.is_empty() {
            let mut ids = Vec::new();
            let mut docs = Vec::new();
            let mut metas = Vec::new();
            for (idx, ch) in chunks.iter().enumerate() {
                ids.push(format!("{macro_id}_ch_{idx}"));
                let page_str = ch.page.map(|p| format!(" (p. {p})")).unwrap_or_default();
                docs.push(format!(
                    "[{rel_path} > {}{page_str}]\n{}",
                    ch.heading, ch.text
                ));
                metas.push(vec![
                    ("file_path".to_string(), s(&f.key)),
                    ("relative_path".to_string(), s(&rel_path)),
                    ("file_name".to_string(), s(&f.name)),
                    ("heading".to_string(), s(&ch.heading)),
                    ("page".to_string(), int(ch.page.unwrap_or(0))),
                    ("chunk_index".to_string(), int(idx)),
                    ("total_chunks".to_string(), int(chunks.len())),
                ]);
            }
            client
                .add(&micro_col, &ids, &docs, &metas)
                .map_err(store_crash)?;
            total_chunks = total_chunks.add(chunks.len());
        }

        conn.execute(
            "
            INSERT OR REPLACE INTO file_hashes
            (file_path, relative_path, sha256, mtime, file_size, chunk_count, last_synced_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        ",
            params![
                f.key,
                rel_path,
                current_hash,
                f.mtime,
                f.size as i64,
                chunks.len() as i64,
                now()
            ],
        )
        .map_err(sqlite_crash)?;

        if is_modified {
            modified_count += 1;
        } else {
            new_count += 1;
        }
    }

    for k in &known {
        let present = matches!(&k.key, Cell::Text(t) if current.iter().any(|c| c.key == *t));
        if present {
            continue;
        }
        println(&format!("  🗑️  [Removido] {}", k.key.py_str()));
        delete_file(&mut client, [&macro_col, &micro_col], &k.key);
        conn.execute(
            "DELETE FROM file_hashes WHERE file_path = ?",
            [cell_value(&k.key)],
        )
        .map_err(sqlite_crash)?;
        deleted_count += 1;
    }
    drop(conn);

    let elapsed: f64 = format!("{:.2}", now() - start_time)
        .parse()
        .unwrap_or_default();
    let rule = "=".repeat(60);
    println(&format!("\n{rule}"));
    println("✨ Sync completed successfully!");
    println(&format!("⏱️  Tempo decorrido: {}s", float_repr(elapsed)));
    println(&format!("📄 New files indexed: {new_count}"));
    println(&format!("📝 Modified files: {modified_count}"));
    println(&format!("⚡ Unchanged files (cached): {unchanged_count}"));
    if deleted_count > 0 {
        println(&format!(
            "🗑️  Files removed from the index: {deleted_count}"
        ));
    }
    println(&format!(
        "🧩 Total chunks in the index: {}",
        total_chunks.text()
    ));
    println(&rule);

    if !cloud_files.is_empty() {
        println(&format!(
            "\n⚠️  Warning: found {} Google Docs shortcuts (.gdoc) that must be downloaded:",
            cloud_files.len()
        ));
        for cf in &cloud_files {
            println(&format!("   • {cf}"));
        }
        println(
            "   👉 Tip: download them from Google Drive as .docx or .pdf to include them in the next sync.",
        );
    }
    Ok(())
}
