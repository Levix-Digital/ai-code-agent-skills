//! `query.py`: semantic search over a folder's `.folder_brain` index, at document level
//! (macro) and passage level (micro).

use std::path::Path;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::json::{dumps, dumps_ascii};
use kernel::py::num::{self, BigInt};
use kernel::py::path::{basename, exists, join, realpath};
use kernel::py::{Crash, Dict, Value};
use vector_store::{Client, MetaValue, Metadata};

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new("query.py", "Semantic query over the Folder Brain")
        .option("--dir", "Target directory (default: current)")
        .required_option("--query", "Texto da consulta ou pergunta")
        .int_option("--top-k", "Number of chunks to return")
        .choice_option("--mode", &["hybrid", "macro", "micro"], "")
        .flag("--json", "Return output as JSON");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let top_k = match args.value("top_k") {
        Some(v) => num::int(v, 10)?,
        None => BigInt::from_i64(6),
    };
    query_brain(
        args.value("dir").unwrap_or("."),
        args.value("query").unwrap_or_default(),
        &top_k,
        args.value("mode").unwrap_or("hybrid"),
        args.flag("json"),
    )?;
    Ok(0)
}

fn store_crash(e: vector_store::StoreError) -> Crash {
    Crash::new(format!("chromadb.errors.ChromaError: {e}"))
}

/// A metadata value as chromadb returns it to Python.
fn value_of(v: &MetaValue) -> Value {
    let list = |items: Vec<Value>| Value::List(items);
    match v {
        MetaValue::Str(s) => Value::str(s.clone()),
        MetaValue::Int(i) => Value::Int(i.to_string()),
        MetaValue::Float(f) => Value::Float(*f),
        MetaValue::Bool(b) => Value::Bool(*b),
        MetaValue::Null => Value::Null,
        MetaValue::StrList(l) => list(l.iter().map(|s| Value::str(s.clone())).collect()),
        MetaValue::IntList(l) => list(l.iter().map(|i| Value::Int(i.to_string())).collect()),
        MetaValue::FloatList(l) => list(l.iter().map(|f| Value::Float(*f)).collect()),
        MetaValue::BoolList(l) => list(l.iter().map(|b| Value::Bool(*b)).collect()),
    }
}

/// `meta.get(key)`.
fn get(meta: &Metadata, key: &str) -> Value {
    meta.iter()
        .find(|(k, _)| k == key)
        .map_or(Value::Null, |(_, v)| value_of(v))
}

/// `round(x, 4)`: the decimal nearest to the exact value, ties to even.
fn round4(x: f64) -> f64 {
    format!("{x:.4}").parse().unwrap_or(x)
}

/// The matches of one collection: `None` stands for an error the script ignores, after the
/// matches already taken.
fn search(
    client: &mut Client,
    name: &str,
    query: &str,
    n: impl Fn(u64) -> Option<usize>,
    keys: &[&str],
    text_key: &str,
    out: &mut Vec<Dict>,
) {
    let Ok(col) = client.get_collection(name) else {
        return;
    };
    let Ok(count) = client.count(&col) else {
        return;
    };
    let Some(n) = n(count) else {
        return;
    };
    let Ok(res) = client.query(&col, query, n, None) else {
        return;
    };
    for i in 0..res.ids.len() {
        let Some(Some(meta)) = res.metadatas.get(i) else {
            return;
        };
        let mut d = Dict::new();
        for k in keys {
            d.insert(*k, get(meta, k));
        }
        d.insert(
            "distance",
            Value::Float(round4(f64::from(res.distances[i]))),
        );
        let doc = res.documents.get(i).cloned().flatten();
        d.insert(text_key, doc.map_or(Value::Null, Value::str));
        out.push(d);
    }
}

fn field<'a>(d: &'a Dict, key: &str) -> &'a Value {
    d.get(key).unwrap_or(&Value::Null)
}

/// `file_path.replace('\\', '/')`.
fn link_path(v: &Value) -> Result<String, Crash> {
    match v {
        Value::Str(s) => Ok(s.replace('\\', "/")),
        other => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'replace'",
            other.tp_name()
        ))),
    }
}

fn distance(d: &Dict) -> f64 {
    match field(d, "distance") {
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

fn query_brain(
    target_dir: &str,
    query: &str,
    top_k: &BigInt,
    mode: &str,
    json_output: bool,
) -> Result<(), Crash> {
    let root = realpath(target_dir);
    let brain = join(&root, ".folder_brain");
    let chroma_path = join(&brain, "chroma_db");

    if !exists(&chroma_path) {
        let msg = "❌ This folder's knowledge base has not been synced yet.\n👉 Run `/sync` or `python sync.py` in this folder first.";
        if json_output {
            let mut d = Dict::new();
            d.insert("error", Value::str(msg));
            println(&dumps_ascii(&Value::Dict(d), None)?);
        } else {
            println(msg);
        }
        return Ok(());
    }

    let mut client = Client::persistent(Path::new(&chroma_path)).map_err(store_crash)?;
    let mut macro_matches = Vec::new();
    let mut micro_chunks = Vec::new();

    if mode == "hybrid" || mode == "macro" {
        search(
            &mut client,
            "folder_macro",
            query,
            |count| Some(if count > 0 { count.min(3) as usize } else { 1 }),
            &["file_name", "relative_path", "file_path", "word_count"],
            "summary",
            &mut macro_matches,
        );
    }
    if mode == "hybrid" || mode == "micro" {
        let k = top_k.to_f64().unwrap_or(f64::MAX);
        search(
            &mut client,
            "folder_micro",
            query,
            |count| {
                if count == 0 {
                    return None;
                }
                let n = k.min(count as f64);
                // chromadb rejects a number of results that is not positive.
                (n >= 1.0).then_some(n as usize)
            },
            &["file_name", "relative_path", "file_path", "heading", "page"],
            "content",
            &mut micro_chunks,
        );
    }

    if json_output {
        let mut d = Dict::new();
        d.insert("query", Value::str(query));
        d.insert("folder", Value::str(root.as_str()));
        d.insert(
            "macro_matches",
            Value::List(macro_matches.into_iter().map(Value::Dict).collect()),
        );
        d.insert(
            "micro_chunks",
            Value::List(micro_chunks.into_iter().map(Value::Dict).collect()),
        );
        println(&dumps(&Value::Dict(d), Some(2))?);
        return Ok(());
    }

    println(&format!(
        "\n🧠 **Semantic Search Results in Folder:** `{}`",
        basename(Path::new(&root))
    ));
    println(&format!("🔍 **Query/Topic:** *{query}*\n"));

    if !macro_matches.is_empty() {
        println("### 📚 Relevant Documents (Macro View)");
        for m in &macro_matches {
            let link = format!(
                "[{}](file:///{})",
                field(m, "file_name").py_str(),
                link_path(field(m, "file_path"))?
            );
            println(&format!(
                "- **{link}** (`{}`) | Score: {:.2}",
                field(m, "relative_path").py_str(),
                1.0 - distance(m)
            ));
        }
        println("");
    }

    if micro_chunks.is_empty() {
        println("No relevant passage found for this query.");
        return Ok(());
    }
    println("### 🧩 Specific Passages Found (Micro View)");
    for (idx, c) in micro_chunks.iter().enumerate() {
        let link = format!(
            "[{}](file:///{})",
            field(c, "file_name").py_str(),
            link_path(field(c, "file_path"))?
        );
        let heading = field(c, "heading");
        let heading_info = if heading.truthy() {
            format!(" > {}", heading.py_str())
        } else {
            String::new()
        };
        let page = field(c, "page");
        let page_info = if page.truthy() {
            format!(" (p. {})", page.py_str())
        } else {
            String::new()
        };
        let score = (1.0 - distance(c)) * 100.0;
        println(&format!(
            "**[{}] {link}{heading_info}{page_info}** *(Relevance: {score:.2}%)*",
            idx + 1
        ));
        println("```text");
        println(&field(c, "content").py_str());
        println("```\n");
    }
    Ok(())
}
