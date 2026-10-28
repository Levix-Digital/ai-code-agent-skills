//! `_docs.py`: the project layout and configuration, frontmatter documents, ADR ids and the
//! ASCII slugs of file names.

use std::cmp::Ordering;
use std::process::Command;
use std::sync::LazyLock;

use kernel::py::json::{self, LoadError};
use kernel::py::num::{self, decimal_value, strip};
use kernel::py::path::{SEP, abspath, basename, isdir, isfile, join, makedirs, relpath};
use kernel::py::yaml::{self, DumpOptions};
use kernel::py::{Crash, Dict, NONE, Value, io_crash, matches_to_end, read_text, write_text};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// `ADR_STATUSES`.
pub const ADR_STATUSES: [&str; 5] = [
    "proposed",
    "accepted",
    "rejected",
    "deprecated",
    "superseded",
];

/// `ADR_FIELD_ORDER`.
pub const ADR_FIELD_ORDER: [&str; 10] = [
    "id",
    "title",
    "status",
    "date",
    "version",
    "subsystems",
    "supersedes",
    "superseded_by",
    "amends",
    "amended_by",
];

pub const MARKER_PREFIX: &str = "<!-- architecture-docs:";

/// `MARKER.format(name=name, edge=edge)`.
pub fn marker(name: &str, edge: &str) -> String {
    format!("{MARKER_PREFIX}{name}:{edge} -->")
}

/// `DEFAULTS`.
fn defaults() -> Dict {
    [
        ("index", Value::str("TECDOC.md")),
        ("architecture_dir", Value::str(join("docs", "architecture"))),
        ("adr_dir", Value::str(join("docs", "adr"))),
        ("max_lines", Value::int(800)),
        ("language", Value::str("en")),
        (
            "language_scope",
            Value::list_of_str(&["index", "architecture", "adr"]),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// `value.get(key)` on what may not be a dict: `None` when the key is missing.
pub fn get<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Crash> {
    match value {
        Value::Dict(d) => Ok(d.get(key).unwrap_or(&NONE)),
        other => Err(no_attribute(other, "get")),
    }
}

pub fn no_attribute(value: &Value, attr: &str) -> Crash {
    Crash::new(format!(
        "AttributeError: '{}' object has no attribute '{attr}'",
        value.type_name()
    ))
}

/// `os.path.join(base, value)` for a value from the configuration.
fn join_value(base: &str, value: &Value) -> Result<String, Crash> {
    match value {
        Value::Str(s) => Ok(join(base, s)),
        other => Err(Crash::new(format!(
            "TypeError: join() argument must be str, bytes, or os.PathLike object, not '{}'",
            other.type_name()
        ))),
    }
}

/// Characters that decompose to ASCII in the Unicode version of `unicode-normalization` but
/// are unknown to Python 3.13's (15.1), which drops them.
fn newer_than_python(c: char) -> bool {
    matches!(c as u32, 0x209d..=0x209f | 0xa7f1 | 0x1ccd6..=0x1ccf9)
}

/// `ascii_slug(text)`.
pub fn ascii_slug(text: &str) -> String {
    let known: String = text.chars().filter(|&c| !newer_than_python(c)).collect();
    let ascii: String = known
        .nfkd()
        .filter(char::is_ascii)
        .collect::<String>()
        .to_ascii_lowercase();
    let mut slug = String::new();
    let mut in_run = false;
    for c in ascii.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = slug[..slug.len().min(60)].trim_end_matches('-');
    if slug.is_empty() {
        "untitled".into()
    } else {
        slug.into()
    }
}

/// `repo_root(start_dir)`: the Git top level, or the directory itself outside Git.
pub fn repo_root(start_dir: Option<&str>) -> String {
    let start = abspath(start_dir.filter(|d| !d.is_empty()).unwrap_or(""))
        .to_string_lossy()
        .into_owned();
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&start)
        .output();
    match out {
        Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
            Ok(text) => abspath(strip(&kernel::py::universal_newlines(&text)))
                .to_string_lossy()
                .into_owned(),
            Err(_) => start,
        },
        _ => start,
    }
}

pub struct Project {
    pub root: String,
    pub manifest: Value,
    pub config: Dict,
    pub name: Value,
    pub index_path: String,
    pub architecture_dir: String,
    pub adr_dir: String,
    pub plans_dir: String,
}

impl Project {
    pub fn new(start_dir: Option<&str>) -> Result<Self, Crash> {
        let root = repo_root(start_dir);
        let manifest_path = join(&root, ".dev-cycle.json");
        let manifest = if isfile(&manifest_path) {
            match json::load_file(std::path::Path::new(&manifest_path)) {
                Ok(v) => v,
                Err(LoadError::Decode(e)) => {
                    return Err(Crash::new(format!("json.decoder.JSONDecodeError: {e}")));
                }
                Err(LoadError::Crash(c)) => return Err(c),
            }
        } else {
            Value::Dict(Dict::new())
        };
        let mut config = defaults();
        let section = match &manifest {
            Value::Dict(d) => d.get("architecture_docs").cloned(),
            other => return Err(no_attribute(other, "get")),
        };
        match section {
            None => {}
            Some(Value::Dict(over)) => {
                for (k, v) in over.iter() {
                    config.insert_value(k.clone(), v.clone());
                }
            }
            Some(other) => {
                return Err(Crash::new(format!(
                    "TypeError: '{}' object is not a mapping",
                    other.type_name()
                )));
            }
        }
        let project_name = get(&manifest, "project_name")?;
        let name = if project_name.truthy() {
            project_name.clone()
        } else {
            Value::Str(basename(std::path::Path::new(&root)))
        };
        let index_path = join_value(&root, config.get_or_none("index"))?;
        let architecture_dir = join_value(&root, config.get_or_none("architecture_dir"))?;
        let adr_dir = join_value(&root, config.get_or_none("adr_dir"))?;
        let plans = get(&manifest, "plans")?;
        let plans_value = if plans.truthy() {
            get(plans, "dir")?.clone()
        } else {
            Value::Null
        };
        let plans_rel = if plans_value.truthy() {
            plans_value
        } else {
            let legacy = get(&manifest, "plans_dir")?;
            if legacy.truthy() {
                legacy.clone()
            } else {
                Value::str(join("docs", "plans"))
            }
        };
        let plans_dir = join_value(&root, &plans_rel)?;
        Ok(Project {
            root,
            manifest,
            config,
            name,
            index_path,
            architecture_dir,
            adr_dir,
            plans_dir,
        })
    }

    /// `project.rel(path)`: relative to the root, with `/`.
    pub fn rel(&self, path: &str) -> Result<String, Crash> {
        Ok(relpath(path, &self.root)?.replace(SEP, "/"))
    }

    /// True once the project uses the docs/adr + docs/architecture layout.
    pub fn adopted(&self) -> bool {
        isdir(&self.adr_dir) || isdir(&self.architecture_dir)
    }

    pub fn architecture_docs(&self) -> Result<Vec<Doc>, Crash> {
        if !isdir(&self.architecture_dir) {
            return Ok(Vec::new());
        }
        listdir(&self.architecture_dir)?
            .into_iter()
            .filter(|n| n.ends_with(".md"))
            .map(|n| read_doc(&join(&self.architecture_dir, &n)))
            .collect()
    }

    pub fn adrs(&self) -> Result<Vec<Doc>, Crash> {
        if !isdir(&self.adr_dir) {
            return Ok(Vec::new());
        }
        listdir(&self.adr_dir)?
            .into_iter()
            .filter(|n| is_adr_file(n))
            .map(|n| read_doc(&join(&self.adr_dir, &n)))
            .collect()
    }

    /// `adr_by_id()`: the ADRs and the dict from id to ADR, as entries of (id, index into the
    /// ADRs). Equal ids share one entry, which keeps the first id and the last ADR.
    pub fn adr_by_id(&self) -> Result<(Vec<Doc>, AdrIds), Crash> {
        let docs = self.adrs()?;
        let mut entries = AdrIds::new();
        for (i, d) in docs.iter().enumerate() {
            let id = get(&d.meta, "id")?;
            if !id.is_hashable() {
                return Err(Crash::new(format!(
                    "TypeError: unhashable type: '{}'",
                    id.type_name()
                )));
            }
            match entries.iter_mut().find(|(k, _)| same_key(k, id)) {
                Some(entry) => entry.1 = i,
                None => entries.push((id.clone(), i)),
            }
        }
        Ok((docs, entries))
    }

    /// `adr_path(adr_id, title)`.
    pub fn adr_path(&self, adr_id: &str, title: &str) -> String {
        join(
            &self.adr_dir,
            &format!("{}-{}.md", zero_pad4(adr_id), ascii_slug(title)),
        )
    }
}

/// The entries of the `adr_by_id()` dict: (id, index into the ADRs).
pub type AdrIds = Vec<(Value, usize)>;

/// Dict key equality: `==`, and `.nan`, which PyYAML loads as one shared object.
fn same_key(a: &Value, b: &Value) -> bool {
    a.py_eq(b) || matches!((a, b), (Value::Float(x), Value::Float(y)) if x.is_nan() && y.is_nan())
}

/// The entry of `adrs[target]`, for an integer id.
pub fn find_adr(entries: &[(Value, usize)], target: &str) -> Option<usize> {
    let key = Value::Int(target.to_string());
    entries.iter().find(|(k, _)| k.py_eq(&key)).map(|(_, i)| *i)
}

/// `next_adr_id()`: one more than the largest integer id, from 1.
pub fn next_adr_id(entries: &[(Value, usize)]) -> Result<String, Crash> {
    let mut max = "0".to_string();
    let mut any = false;
    for (k, _) in entries {
        let text = match k {
            Value::Int(i) => i.clone(),
            Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
            _ => continue,
        };
        if !any || cmp_int(&text, &max) == Ordering::Greater {
            max = text;
            any = true;
        }
    }
    Ok(num::int(&max, 10)?
        .add(&num::BigInt::from_i64(1))
        .to_string())
}

/// Compares two integers given as decimal text.
pub fn cmp_int(a: &str, b: &str) -> Ordering {
    let (na, da) = a.strip_prefix('-').map_or((false, a), |d| (true, d));
    let (nb, db) = b.strip_prefix('-').map_or((false, b), |d| (true, d));
    let mag = da.len().cmp(&db.len()).then_with(|| da.cmp(db));
    match (na, nb) {
        (false, false) => mag,
        (true, true) => mag.reverse(),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
    }
}

/// `re.match(r"\d{4}-.*\.md$", name)`, with Python's Unicode digits.
fn is_adr_file(name: &str) -> bool {
    static REST: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-.*\.md$").expect("valid pattern"));
    let mut chars = name.chars();
    (0..4).all(|_| chars.next().and_then(decimal_value).is_some())
        && matches_to_end(chars.as_str(), |t| REST.is_match(t))
}

/// `sorted(os.listdir(path))`.
fn listdir(path: &str) -> Result<Vec<String>, Crash> {
    let entries = std::fs::read_dir(path).map_err(|e| io_crash(&e, std::path::Path::new(path)))?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

pub struct Doc {
    pub path: String,
    pub meta: Value,
    pub body: String,
    pub has_frontmatter: bool,
}

impl Doc {
    /// The text of the first `# ` heading.
    pub fn h1(&self) -> Option<String> {
        static H1: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?m)^# (.+)$").expect("valid pattern"));
        H1.captures(&self.body).map(|c| strip(&c[1]).to_string())
    }
}

/// `_plain(value)`: dates as ISO text, inside dicts and lists too.
fn plain(value: Value) -> Value {
    match value {
        Value::Date(d) => Value::Str(d.iso()),
        Value::DateTime(d) => Value::Str(d.iso('T')),
        Value::Dict(d) => {
            let mut out = Dict::new();
            for (k, v) in d.iter() {
                out.push_new(k.clone(), plain(v.clone()));
            }
            Value::Dict(out)
        }
        Value::List(items) => Value::List(items.into_iter().map(plain).collect()),
        other => other,
    }
}

/// `split_frontmatter(text)`: (meta, body, has_frontmatter).
pub fn split_frontmatter(text: &str) -> Result<(Value, String, bool), Crash> {
    if text.starts_with("---\n")
        && let Some(i) = text[4..].find("\n---\n")
    {
        let end = 4 + i;
        let loaded = yaml::safe_load(&text[4..end])?;
        let meta = if loaded.truthy() {
            loaded
        } else {
            Value::Dict(Dict::new())
        };
        let body = text[end + 5..].trim_start_matches('\n').to_string();
        return Ok((plain(meta), body, true));
    }
    Ok((Value::Dict(Dict::new()), text.to_string(), false))
}

pub fn read_doc(path: &str) -> Result<Doc, Crash> {
    let text = read_text(std::path::Path::new(path))?;
    let (meta, body, has_frontmatter) = split_frontmatter(&text)?;
    Ok(Doc {
        path: path.to_string(),
        meta,
        body,
        has_frontmatter,
    })
}

/// `write_doc(doc)`: the frontmatter as `yaml.safe_dump` writes it, then the body.
pub fn write_doc(doc: &Doc) -> Result<(), Crash> {
    write_meta(doc, &doc.meta)
}

/// `write_doc(doc, order)`: the fields in `order` first, then the others as they are.
pub fn write_doc_ordered(doc: &Doc, order: &[&str]) -> Result<(), Crash> {
    let Value::Dict(meta) = &doc.meta else {
        return write_doc(doc);
    };
    let mut ordered = Dict::new();
    for k in order {
        if let Some(v) = meta.get(k) {
            ordered.insert(*k, v.clone());
        }
    }
    for (k, v) in meta.iter() {
        if !k.as_str().is_some_and(|k| order.contains(&k)) {
            ordered.push_new(k.clone(), v.clone());
        }
    }
    write_meta(doc, &Value::Dict(ordered))
}

fn write_meta(doc: &Doc, meta: &Value) -> Result<(), Crash> {
    let options = DumpOptions {
        default_flow_style: None,
        allow_unicode: true,
        width: Some(4096),
    };
    let front = yaml::safe_dump(meta, &options)?;
    makedirs(&kernel::py::path::dirname(&doc.path))?;
    let text = format!("---\n{front}---\n\n{}\n", strip(&doc.body));
    write_text(std::path::Path::new(&doc.path), &text)
}

/// `as_id_list(value)`: the first number in each item, as ADR ids.
pub fn as_id_list(value: &Value) -> Result<Vec<String>, Crash> {
    let items = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Str(s) if s.is_empty() => return Ok(Vec::new()),
        Value::List(items) if items.is_empty() => return Ok(Vec::new()),
        Value::List(items) => items.clone(),
        other => vec![other.clone()],
    };
    let mut out = Vec::new();
    for v in items {
        let text = v.py_str();
        let digits: String = text
            .chars()
            .skip_while(|c| decimal_value(*c).is_none())
            .take_while(|c| decimal_value(*c).is_some())
            .collect();
        if !digits.is_empty() {
            out.push(num::int(&digits, 10)?.to_string());
        }
    }
    Ok(out)
}

/// `f"{value:04d}"` for an integer's decimal text.
pub fn zero_pad4(int_text: &str) -> String {
    match int_text.strip_prefix('-') {
        Some(digits) => format!("-{digits:0>3}"),
        None => format!("{int_text:0>4}"),
    }
}

/// `adr_label(adr_id)`.
pub fn adr_label(int_text: &str) -> String {
    format!("ADR-{}", zero_pad4(int_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_like_python() {
        assert_eq!(ascii_slug("Ingestão de PDFs"), "ingestao-de-pdfs");
        assert_eq!(ascii_slug("ﬁle ½ ™"), "file-12-tm");
        assert_eq!(ascii_slug("!!!"), "untitled");
        assert_eq!(
            ascii_slug(&"ab-".repeat(30)),
            "ab-".repeat(20).trim_end_matches('-')
        );
    }

    #[test]
    fn adr_ids_and_labels() {
        let ids = as_id_list(&Value::List(vec![
            Value::str("ADR-0003"),
            Value::int(7),
            Value::Bool(true),
        ]))
        .unwrap();
        assert_eq!(ids, vec!["3", "7"]);
        assert!(as_id_list(&Value::str("")).unwrap().is_empty());
        assert_eq!(as_id_list(&Value::str("see ١٢")).unwrap(), vec!["12"]);
        assert_eq!(adr_label("3"), "ADR-0003");
        assert_eq!(adr_label("-5"), "ADR--005");
        assert_eq!(adr_label("12345"), "ADR-12345");
    }

    #[test]
    fn adr_ids_follow_python_dict_keys() {
        let ids: AdrIds = vec![
            (Value::Float(2.0), 0),
            (Value::Bool(true), 1),
            (Value::str("9"), 2),
            (Value::int(-3), 3),
        ];
        // 2.0 matches 2 but is no int, so the next id comes from True (1).
        assert_eq!(find_adr(&ids, "2"), Some(0));
        assert_eq!(find_adr(&ids, "1"), Some(1));
        assert_eq!(find_adr(&ids, "9"), None);
        assert_eq!(next_adr_id(&ids).unwrap(), "2");
        assert_eq!(next_adr_id(&[(Value::int(-3), 0)]).unwrap(), "-2");
        assert_eq!(next_adr_id(&AdrIds::new()).unwrap(), "1");
    }

    #[test]
    fn frontmatter_dates_become_text() {
        let (meta, body, has) =
            split_frontmatter("---\ndate: 2026-01-02\nat: 2026-01-02 10:00:00\n---\n\n\nBody\n")
                .unwrap();
        assert!(has);
        assert_eq!(body, "Body\n");
        assert_eq!(
            meta.py_repr(),
            "{'date': '2026-01-02', 'at': '2026-01-02T10:00:00'}"
        );
        let (meta, _, has) = split_frontmatter("---\n[]\n---\nx").unwrap();
        assert!(has);
        assert_eq!(meta.py_repr(), "{}");
    }
}
