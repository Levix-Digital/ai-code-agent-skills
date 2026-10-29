//! `roadmap.py`, `_roadmap.py` and `_roadmap_store.py`: the releases of the product in
//! delivery order and the backlog items each one delivers (default: docs/ROADMAP.md). The
//! roadmap is the single source of the item-to-release assignment; the backlog holds the
//! items. Writes the file; never commits.

use std::path::Path;

use architecture_docs::docs::{join_value, repo_root};
use kernel::out::println;
use kernel::project_config::{ConfigError, load_project_config};
use kernel::py::argparse::{Parsed, Parser};
use kernel::py::num::{decimal_value, is_space, strip};
use kernel::py::path::{dirname, isfile, join, makedirs, relpath};
use kernel::py::unicode::upper;
use kernel::py::{
    Crash, Dict, Value, io_crash, json, repr_str, today, universal_newlines, utf8_error_text,
    write_text,
};

use crate::backlog::{Backlog, Item, id_match, load_backlog, lstrip, rstrip};

pub const STATUSES: [&str; 3] = ["planned", "in_progress", "released"];
const NO_ITEMS: &str = "-";
const FIELD_ORDER: [&str; 5] = ["Status", "Target", "Released", "Goal", "Items"];
const OPEN: [&str; 3] = ["open", "planned", "in_progress"];
const TEMPLATE: &str = include_str!("../../../plan-memory/templates/ROADMAP_TEMPLATE.md");

/// `RoadmapError` and `BacklogError` (printed after `❌`), or an exception the script does not
/// catch.
#[derive(Debug)]
pub enum RoadmapError {
    Error(String),
    Crash(Crash),
}

impl From<Crash> for RoadmapError {
    fn from(c: Crash) -> Self {
        RoadmapError::Crash(c)
    }
}

fn error<T>(msg: impl Into<String>) -> Result<T, RoadmapError> {
    Err(RoadmapError::Error(msg.into()))
}

/// `open(path, encoding="utf-8", newline="").read()`: the text as it is.
pub fn read_exact(path: &str) -> Result<String, Crash> {
    let bytes = std::fs::read(path).map_err(|e| io_crash(&e, Path::new(path)))?;
    String::from_utf8(bytes).map_err(|e| {
        Crash::new(format!(
            "UnicodeDecodeError: {}",
            utf8_error_text(e.as_bytes(), &e.utf8_error())
        ))
    })
}

/// `SECTION_RE.match(line)` (`^##\s+(?!#)(.+?)\s*$`) on a line without trailing spaces.
pub fn section_heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("##")?;
    let after = lstrip(rest);
    if after.len() == rest.len() || after.is_empty() {
        return None;
    }
    if !after.starts_with('#') {
        return Some(after);
    }
    // `\s+` gives its last space back to the name when a `#` follows it.
    let spaces = &rest[..rest.len() - after.len()];
    let last = spaces.chars().last()?;
    (spaces.len() > last.len_utf8()).then(|| &rest[spaces.len() - last.len_utf8()..])
}

/// `FIELD_RE.match(line)` (`^-\s+\*\*([A-Za-z ]+)\*\*\s*:\s?(.*)$`): the stripped field
/// name and the value as written.
pub fn field_line(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix('-')?;
    let after = lstrip(rest);
    if after.len() == rest.len() {
        return None;
    }
    let after = after.strip_prefix("**")?;
    let end = after
        .find(|c: char| !(c.is_ascii_alphabetic() || c == ' '))
        .unwrap_or(after.len());
    if end == 0 {
        return None;
    }
    let name = strip(&after[..end]).to_string();
    let rest = lstrip(after[end..].strip_prefix("**")?).strip_prefix(':')?;
    let value = match rest.chars().next() {
        Some(c) if is_space(c) => &rest[c.len_utf8()..],
        _ => rest,
    };
    Some((name, value))
}

/// `RELEASE_RE.match(heading)` (`^(v\d+\.\d+\.\d+)\s*:\s*(.+?)\s*$`).
fn release_heading(heading: &str) -> Option<(&str, &str)> {
    let end = version_end(heading)?;
    let rest = lstrip(&heading[end..]).strip_prefix(':')?;
    let name = lstrip(rest);
    if name.is_empty() {
        return None;
    }
    Some((&heading[..end], name))
}

/// The end of `v\d+\.\d+\.\d+` at the start of `s`.
fn version_end(s: &str) -> Option<usize> {
    let mut pos = s.strip_prefix('v').map(|_| 1)?;
    for part in 0..3 {
        if part > 0 {
            if !s[pos..].starts_with('.') {
                return None;
            }
            pos += 1;
        }
        let digits = s[pos..]
            .char_indices()
            .find(|(_, c)| decimal_value(*c).is_none())
            .map_or(s.len() - pos, |(i, _)| i);
        if digits == 0 {
            return None;
        }
        pos += digits;
    }
    Some(pos)
}

/// `version_key(version)`: the three numbers, as keys that order like them.
fn version_key(version: &str) -> Result<Vec<(usize, String)>, RoadmapError> {
    let full = version_end(version).filter(|&e| matches!(&version[e..], "" | "\n"));
    let Some(end) = full else {
        return error(format!("'{version}' is not a version like v1.2.0"));
    };
    Ok(version[1..end]
        .split('.')
        .map(|digits| {
            let ascii: String = digits
                .chars()
                .map(|c| char::from(b'0' + decimal_value(c).unwrap_or(0) as u8))
                .collect();
            let trimmed = ascii.trim_start_matches('0').to_string();
            (trimmed.len(), trimmed)
        })
        .collect())
}

/// `ITEM_ID_RE.match(item_id)`.
fn is_item_id(item_id: &str) -> bool {
    id_match(item_id).is_some()
}

/// `name: [first line value, continuation lines as written]`, in file order.
type Fields = Vec<(String, Vec<String>)>;

fn field_get(fields: &Fields, name: &str) -> String {
    let Some((_, lines)) = fields.iter().find(|(n, _)| n == name) else {
        return String::new();
    };
    let parts: Vec<&str> = lines
        .iter()
        .map(|l| strip(l))
        .filter(|l| !l.is_empty())
        .collect();
    parts.join(" ")
}

pub struct Release {
    pub version: String,
    pub name: String,
    pub line: Option<usize>,
    fields: Fields,
}

impl Release {
    fn new(version: &str, name: &str, line: Option<usize>) -> Self {
        Release {
            version: version.to_string(),
            name: name.to_string(),
            line,
            fields: Vec::new(),
        }
    }

    pub fn get(&self, name: &str) -> String {
        field_get(&self.fields, name)
    }

    fn set(&mut self, name: &str, value: &str) {
        let text = value.replace("\r\n", "\n");
        let mut lines = text.split('\n');
        let mut out = vec![strip(lines.next().unwrap_or("")).to_string()];
        out.extend(lines.map(|l| {
            let s = strip(l);
            if s.is_empty() {
                String::new()
            } else {
                format!("  {s}")
            }
        }));
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some((_, lines)) => *lines = out,
            None => self.fields.push((name.to_string(), out)),
        }
    }

    fn pop(&mut self, name: &str) {
        self.fields.retain(|(n, _)| n != name);
    }

    pub fn status(&self) -> String {
        self.get("Status")
    }

    pub fn items(&self) -> Vec<String> {
        let value = self.get("Items");
        if value.is_empty() || value == NO_ITEMS {
            return Vec::new();
        }
        value
            .split(',')
            .map(strip)
            .filter(|p| !p.is_empty())
            .map(upper)
            .collect()
    }

    fn set_items(&mut self, ids: &[String]) {
        let value = if ids.is_empty() {
            NO_ITEMS.to_string()
        } else {
            ids.join(", ")
        };
        self.set("Items", &value);
    }

    pub fn to_dict(&self) -> Dict {
        let or_none = |v: String| {
            if v.is_empty() {
                Value::Null
            } else {
                Value::Str(v)
            }
        };
        let mut d = Dict::new();
        d.insert("version", Value::str(&self.version));
        d.insert("name", Value::str(&self.name));
        d.insert("status", Value::Str(self.status()));
        d.insert("target", or_none(self.get("Target")));
        d.insert("released", or_none(self.get("Released")));
        d.insert("goal", Value::Str(self.get("Goal")));
        d.insert(
            "items",
            Value::List(self.items().into_iter().map(Value::Str).collect()),
        );
        d
    }
}

pub struct Roadmap {
    newline: &'static str,
    header: Vec<String>,
    pub releases: Vec<Release>,
    pub errors: Vec<String>,
}

impl Roadmap {
    pub fn parse(text: &str) -> Self {
        let mut roadmap = Roadmap {
            newline: if text.contains("\r\n") { "\r\n" } else { "\n" },
            header: Vec::new(),
            releases: Vec::new(),
            errors: Vec::new(),
        };
        let text = text.replace("\r\n", "\n");
        // Index of the current release in `releases`, and the field being read.
        let mut release: Option<usize> = None;
        let mut field: Option<String> = None;
        for (i, line) in text.split('\n').enumerate() {
            let line = rstrip(line);
            if let Some(heading) = section_heading(line) {
                field = None;
                match release_heading(heading) {
                    Some((version, name)) => {
                        roadmap.releases.push(Release::new(version, name, Some(i)));
                        release = Some(roadmap.releases.len() - 1);
                    }
                    None => {
                        roadmap.errors.push(format!(
                            "line {}: release heading must be '## vX.Y.Z: <name>'",
                            i + 1
                        ));
                        release = None;
                    }
                }
                continue;
            }
            let Some(r) = release else {
                if roadmap.releases.is_empty() && roadmap.errors.is_empty() {
                    roadmap.header.push(line.to_string());
                } else if !strip(line).is_empty() {
                    roadmap
                        .errors
                        .push(format!("line {}: text outside a release", i + 1));
                }
                continue;
            };
            let rel = &mut roadmap.releases[r];
            if let Some((name, value)) = field_line(line) {
                let value = vec![strip(value).to_string()];
                match rel.fields.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, lines)) => {
                        roadmap.errors.push(format!(
                            "line {}: {} repeats the field '{name}'",
                            i + 1,
                            rel.version
                        ));
                        *lines = value;
                    }
                    None => rel.fields.push((name.clone(), value)),
                }
                field = Some(name);
                continue;
            }
            if let Some(name) = &field {
                if let Some((_, lines)) = rel.fields.iter_mut().find(|(n, _)| n == name) {
                    lines.push(line.to_string());
                }
            } else if !strip(line).is_empty() {
                roadmap.errors.push(format!(
                    "line {}: text in {} outside a '- **Field**:' line",
                    i + 1,
                    rel.version
                ));
            }
        }
        roadmap
    }

    fn find(&self, version: &str) -> Result<usize, RoadmapError> {
        match self.releases.iter().position(|r| r.version == version) {
            Some(i) => Ok(i),
            None => error(format!("{version}: no such release in the roadmap")),
        }
    }

    /// `release_of(item_id)`: the index of the first release that holds the item.
    pub fn release_of(&self, item_id: &str) -> Option<usize> {
        let want = upper(item_id);
        self.releases.iter().position(|r| r.items().contains(&want))
    }

    /// The first release that is not released yet, in roadmap order.
    pub fn current(&self) -> Option<&Release> {
        self.releases.iter().find(|r| r.status() != "released")
    }

    pub fn assigned(&self) -> Vec<String> {
        self.releases.iter().flat_map(Release::items).collect()
    }

    fn add_release(
        &mut self,
        version: &str,
        name: &str,
        goal: &str,
        target: Option<&str>,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<usize, RoadmapError> {
        version_key(version)?;
        if self.releases.iter().any(|r| r.version == version) {
            return error(format!("{version} is already in the roadmap"));
        }
        if strip(name).is_empty() {
            return error("the release name must not be empty");
        }
        if strip(goal).is_empty() {
            return error("the release goal must not be empty");
        }
        let mut release = Release::new(version, strip(name), None);
        release.set("Status", "planned");
        if let Some(t) = target.filter(|t| !t.is_empty()) {
            release.set("Target", t);
        }
        release.set("Goal", strip(goal));
        release.set_items(&[]);
        self.insert(release, before, after)
    }

    fn insert(
        &mut self,
        release: Release,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<usize, RoadmapError> {
        let before = before.filter(|b| !b.is_empty());
        let after = after.filter(|a| !a.is_empty());
        if before.is_some() && after.is_some() {
            return error("use --before or --after, not both");
        }
        let at = match (before, after) {
            (Some(b), _) => self.find(b)?,
            (_, Some(a)) => self.find(a)? + 1,
            _ => self.releases.len(),
        };
        self.releases.insert(at, release);
        Ok(at)
    }

    fn move_release(
        &mut self,
        version: &str,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<(), RoadmapError> {
        if before.is_none_or(str::is_empty) && after.is_none_or(str::is_empty) {
            return error("say where: --before <version> or --after <version>");
        }
        let i = self.find(version)?;
        if before == Some(version) || after == Some(version) {
            return error("a release cannot move relative to itself");
        }
        let release = self.releases.remove(i);
        self.insert(release, before, after)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn update_release(
        &mut self,
        version: &str,
        name: Option<&str>,
        goal: Option<&str>,
        target: Option<&str>,
        status: Option<&str>,
        released_on: Option<String>,
    ) -> Result<usize, RoadmapError> {
        let i = self.find(version)?;
        let release = &mut self.releases[i];
        if let Some(name) = name {
            if strip(name).is_empty() {
                return error("the release name must not be empty");
            }
            release.name = strip(name).to_string();
        }
        if let Some(goal) = goal {
            if strip(goal).is_empty() {
                return error("the release goal must not be empty");
            }
            release.set("Goal", strip(goal));
        }
        if let Some(target) = target {
            if strip(target).is_empty() {
                release.pop("Target");
            } else {
                release.set("Target", strip(target));
            }
        }
        if let Some(status) = status {
            release.set("Status", status);
            if status == "released" {
                release.set("Released", &released_on.unwrap_or_default());
            } else {
                release.pop("Released");
            }
        }
        Ok(i)
    }

    /// Adds items to a release. An item in another release moves only with `--move`.
    fn assign(
        &mut self,
        ids: &[String],
        version: &str,
        move_items: bool,
    ) -> Result<Vec<String>, RoadmapError> {
        let r = self.find(version)?;
        if self.releases[r].status() == "released" {
            return error(format!(
                "{version} is released; plan the items in a later release"
            ));
        }
        let mut added = Vec::new();
        for item_id in ids.iter().map(|i| upper(i)) {
            if !is_item_id(&item_id) {
                return error(format!("'{item_id}' is not a backlog id"));
            }
            let current = self.release_of(&item_id);
            if current == Some(r) {
                continue;
            }
            if let Some(c) = current {
                let other = &mut self.releases[c];
                if !move_items {
                    return error(format!(
                        "{item_id} is already in {}; use --move to move it",
                        other.version
                    ));
                }
                if other.status() == "released" {
                    return error(format!(
                        "{item_id} was delivered in the released {}",
                        other.version
                    ));
                }
                let kept: Vec<String> = other
                    .items()
                    .into_iter()
                    .filter(|i| *i != item_id)
                    .collect();
                other.set_items(&kept);
            }
            let release = &mut self.releases[r];
            let mut items = release.items();
            items.push(item_id.clone());
            release.set_items(&items);
            added.push(item_id);
        }
        Ok(added)
    }

    fn unassign(&mut self, ids: &[String]) -> Result<Vec<String>, RoadmapError> {
        let mut removed = Vec::new();
        for item_id in ids.iter().map(|i| upper(i)) {
            let Some(r) = self.release_of(&item_id) else {
                return error(format!("{item_id} is not in any release"));
            };
            let release = &mut self.releases[r];
            if release.status() == "released" {
                return error(format!(
                    "{item_id} was delivered in the released {}",
                    release.version
                ));
            }
            let kept: Vec<String> = release
                .items()
                .into_iter()
                .filter(|i| *i != item_id)
                .collect();
            release.set_items(&kept);
            removed.push(item_id);
        }
        Ok(removed)
    }

    /// Problems of the roadmap; with a backlog, also checks the ids against it.
    pub fn validate(&self, backlog: Option<&Backlog>) -> Vec<String> {
        let mut problems = self.errors.clone();
        let mut seen_versions: Vec<&str> = Vec::new();
        let mut seen_items: Vec<(String, String)> = Vec::new();
        let mut released_after_open = false;
        let mut open_seen = false;
        for release in &self.releases {
            let place = match release.line {
                Some(l) => format!("line {}: {}", l + 1, release.version),
                None => release.version.clone(),
            };
            if seen_versions.contains(&release.version.as_str()) {
                problems.push(format!("{place}: duplicate release"));
            }
            seen_versions.push(&release.version);
            for name in ["Status", "Goal", "Items"] {
                if release.get(name).is_empty() {
                    problems.push(format!("{place}: missing '- **{name}**:'"));
                }
            }
            let status = release.status();
            if !status.is_empty() && !STATUSES.contains(&status.as_str()) {
                problems.push(format!(
                    "{place}: status must be one of: {}",
                    STATUSES.join(", ")
                ));
            }
            let released = !release.get("Released").is_empty();
            if status == "released" && !released {
                problems.push(format!(
                    "{place}: a released release needs '- **Released**: <date>'"
                ));
            }
            if status != "released" && released {
                problems.push(format!(
                    "{place}: '- **Released**:' only belongs to a released release"
                ));
            }
            if status == "released" && open_seen {
                released_after_open = true;
            }
            if status != "released" {
                open_seen = true;
            }
            for item_id in release.items() {
                if !is_item_id(&item_id) {
                    problems.push(format!("{place}: '{item_id}' is not a backlog id"));
                    continue;
                }
                if let Some((_, v)) = seen_items.iter().find(|(i, _)| *i == item_id) {
                    problems.push(format!("{place}: {item_id} is also in {v}"));
                } else {
                    seen_items.push((item_id.clone(), release.version.clone()));
                }
                if let Some(backlog) = backlog {
                    match backlog.find(&item_id) {
                        Err(_) => {
                            problems.push(format!("{place}: {item_id} is not in the backlog"));
                            continue;
                        }
                        Ok(item) if item.kind() == Some("epic") => {
                            problems.push(format!(
                                "{place}: {item_id} is an epic; assign its items instead"
                            ));
                        }
                        Ok(_) => {}
                    }
                }
            }
        }
        if released_after_open {
            problems.push(
                "a released release comes after one that is not released; order the roadmap by delivery"
                    .to_string(),
            );
        }
        let versions: Vec<&str> = self.releases.iter().map(|r| r.version.as_str()).collect();
        let mut sorted = versions.clone();
        sorted.sort_by_cached_key(|v| version_key(v).unwrap_or_default());
        if versions != sorted {
            problems.push(format!(
                "releases are not in version order: {}",
                versions.join(", ")
            ));
        }
        problems
    }

    pub fn render(&self) -> String {
        let mut out: Vec<String> = self.header.clone();
        while out.last().is_some_and(|l| strip(l).is_empty()) {
            out.pop();
        }
        for release in &self.releases {
            out.push(String::new());
            out.push(format!("## {}: {}", release.version, release.name));
            let mut names: Vec<&str> = FIELD_ORDER
                .iter()
                .copied()
                .filter(|n| release.fields.iter().any(|(f, _)| f == n))
                .collect();
            names.extend(
                release
                    .fields
                    .iter()
                    .map(|(f, _)| f.as_str())
                    .filter(|f| !FIELD_ORDER.contains(f)),
            );
            for field in names {
                let Some((_, lines)) = release.fields.iter().find(|(f, _)| f == field) else {
                    continue;
                };
                let mut lines = lines.clone();
                while lines.len() > 1 && lines.last().is_some_and(|l| strip(l).is_empty()) {
                    lines.pop();
                }
                let first = if lines[0].is_empty() {
                    format!("- **{field}**:")
                } else {
                    format!("- **{field}**: {}", lines[0])
                };
                out.push(first);
                out.extend(lines.into_iter().skip(1));
            }
        }
        format!("{}{}", out.join(self.newline), self.newline)
    }
}

/// The roadmap file of one project.
pub struct RoadmapFile {
    root: String,
    pub path: String,
}

impl RoadmapFile {
    pub fn new(start_dir: Option<&str>) -> Result<Self, Crash> {
        let root = repo_root(start_dir);
        let config = load_project_config(Some(&root)).map_err(ConfigError::into_crash)?;
        let root = config.root.to_string_lossy().into_owned();
        let file = config.section("roadmap").get_or_none("file");
        let default = Value::Str(join("docs", "ROADMAP.md"));
        let path = join_value(&root, if file.truthy() { file } else { &default })?;
        Ok(RoadmapFile { root, path })
    }

    pub fn exists(&self) -> bool {
        isfile(&self.path)
    }

    pub fn rel(&self) -> Result<String, Crash> {
        Ok(relpath(&self.path, &self.root)?.replace('\\', "/"))
    }

    pub fn load(&self, create: bool) -> Result<Roadmap, RoadmapError> {
        if self.exists() {
            return Ok(Roadmap::parse(&read_exact(&self.path)?));
        }
        if !create {
            return error(format!(
                "no roadmap at {}; the first 'add-release' creates it",
                self.rel()?
            ));
        }
        Ok(Roadmap::parse(&universal_newlines(TEMPLATE)))
    }

    /// Atomic write; refuses a file whose structure it could not read, so nothing is lost.
    fn save(&self, roadmap: &Roadmap) -> Result<(), RoadmapError> {
        if !roadmap.errors.is_empty() {
            return error(format!(
                "{} has structural problems; fix them by hand first:\n  - {}",
                self.rel()?,
                roadmap.errors.join("\n  - ")
            ));
        }
        makedirs(&dirname(&self.path))?;
        let tmp = format!("{}.tmp", self.path);
        write_text(Path::new(&tmp), &roadmap.render())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| {
            let c = io_crash(&e, Path::new(&tmp));
            Crash::new(format!("{} -> {}", c.0, repr_str(&self.path)))
        })?;
        Ok(())
    }
}

fn describe(backlog: Option<&Backlog>, item_id: &str) -> Dict {
    let mut d = Dict::new();
    let Some(backlog) = backlog else {
        d.insert("id", Value::str(item_id));
        for key in ["title", "status", "type", "rank"] {
            d.insert(key, Value::Null);
        }
        return d;
    };
    let item: &Item = match backlog.find(item_id) {
        Ok(item) => item,
        Err(_) => {
            d.insert("id", Value::str(item_id));
            d.insert("title", Value::Null);
            d.insert("status", Value::str("missing"));
            d.insert("type", Value::Null);
            d.insert("rank", Value::Null);
            return d;
        }
    };
    d.insert("id", Value::str(&item.id));
    d.insert("title", Value::str(&item.title));
    d.insert("status", Value::Str(item.status()));
    d.insert("type", item.kind().map_or(Value::Null, Value::str));
    d.insert("rank", Value::Str(item.rank()));
    d.insert("closed", Value::Bool(item.closed()));
    d.insert("epic", item.epic().map_or(Value::Null, Value::Str));
    d
}

fn text_of<'a>(d: &'a Dict, key: &str) -> &'a str {
    d.get(key).and_then(Value::as_str).unwrap_or("")
}

fn is_closed(d: &Dict) -> bool {
    d.get("closed").is_some_and(Value::truthy)
}

fn item_line(d: &Dict) -> String {
    let mark = if is_closed(d) { "✓" } else { "·" };
    let status = text_of(d, "status");
    let title = if status == "missing" {
        "(not in the backlog)"
    } else {
        text_of(d, "title")
    };
    let or_q = |s: &str| {
        if s.is_empty() {
            "?".to_string()
        } else {
            s.to_string()
        }
    };
    let id = text_of(d, "id");
    let pad = " ".repeat(9usize.saturating_sub(id.chars().count()));
    format!(
        "{mark} {id}{pad} [{}] ({}) {title}",
        or_q(status),
        or_q(text_of(d, "rank"))
    )
}

/// Backlog ids to assign: an epic becomes its open items (those not in another release,
/// unless `move`).
fn expand(
    backlog: Option<&Backlog>,
    ids: &[String],
    roadmap: &Roadmap,
    move_items: bool,
) -> Result<Vec<String>, RoadmapError> {
    let Some(backlog) = backlog else {
        return error("there is no backlog; capture the items first (backlog.py add)");
    };
    let mut out: Vec<String> = Vec::new();
    for item_id in ids {
        let item = backlog.find(item_id)?;
        if item.kind() == Some("epic") {
            let members: Vec<&Item> = backlog
                .epic_items(&item.id, false)
                .into_iter()
                .filter(|i| move_items || roadmap.release_of(&i.id).is_none())
                .collect();
            if members.is_empty() {
                println(&format!("ℹ️ {} has no open item to assign.", item.id));
            }
            out.extend(members.iter().map(|i| i.id.clone()));
            continue;
        }
        if item.closed() {
            return error(format!("{} is closed ({})", item.id, item.status()));
        }
        out.push(item.id.clone());
    }
    let mut unique: Vec<String> = Vec::new();
    for id in out {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    Ok(unique)
}

/// `current_data(roadmap, backlog)`: the current release, its open items and the open
/// items in no release.
pub fn current_data(roadmap: &Roadmap, backlog: Option<&Backlog>) -> Dict {
    let release = roadmap.current();
    let mut data = Dict::new();
    data.insert(
        "release",
        release.map_or(Value::Null, |r| Value::Dict(r.to_dict())),
    );
    data.insert("open_items", Value::List(Vec::new()));
    data.insert("unscheduled", Value::List(Vec::new()));
    let Some(backlog) = backlog else {
        return data;
    };
    let open = backlog.open_items();
    let order = |id: &str| open.iter().rposition(|i| i.id == id).unwrap_or(open.len());
    if let Some(release) = release {
        let mut items: Vec<Dict> = release
            .items()
            .iter()
            .map(|i| describe(Some(backlog), i))
            .filter(|d| !is_closed(d) && text_of(d, "status") != "missing")
            .collect();
        items.sort_by_cached_key(|d| order(text_of(d, "id")));
        data.insert(
            "open_items",
            Value::List(items.into_iter().map(Value::Dict).collect()),
        );
    }
    let assigned = roadmap.assigned();
    let unscheduled: Vec<Value> = open
        .iter()
        .filter(|i| i.kind() != Some("epic") && !assigned.contains(&i.id))
        .map(|i| Value::str(&i.id))
        .collect();
    data.insert("unscheduled", Value::List(unscheduled));
    data
}

fn warnings(roadmap: &Roadmap, backlog: Option<&Backlog>) -> Vec<String> {
    let mut notes = Vec::new();
    let Some(backlog) = backlog else {
        return notes;
    };
    for release in &roadmap.releases {
        let released = release.status() == "released";
        for item_id in release.items() {
            let d = describe(Some(backlog), &item_id);
            let status = text_of(&d, "status");
            if released && OPEN.contains(&status) {
                notes.push(format!(
                    "{} is released but {item_id} is still {status}",
                    release.version
                ));
            }
            if !released && matches!(status, "dropped" | "duplicate") {
                notes.push(format!(
                    "{item_id} in {} was closed as {status}; unassign it",
                    release.version
                ));
            }
            if !released && status == "superseded" {
                let replacements: Vec<String> = backlog
                    .find(&item_id)
                    .map(Item::superseded_by)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|r| roadmap.release_of(r).is_none())
                    .collect();
                let hint = if replacements.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; assign {} (roadmap.py assign {} --release {})",
                        replacements.join(", "),
                        replacements.join(" "),
                        release.version
                    )
                };
                notes.push(format!(
                    "{item_id} in {} was superseded; unassign it{hint}",
                    release.version
                ));
            }
        }
    }
    notes
}

fn dumps(value: &Value) -> Result<String, Crash> {
    json::dumps(value, Some(2))
}

fn cmd(args: &Parsed) -> Result<u8, RoadmapError> {
    let command = args.value("command").unwrap_or("");
    let dir = args.value("dir");
    let rf = RoadmapFile::new(dir)?;

    if command == "add-release" {
        let mut roadmap = rf.load(true)?;
        let i = roadmap.add_release(
            args.value("version").unwrap_or(""),
            args.value("name").unwrap_or(""),
            args.value("goal").unwrap_or(""),
            args.value("target"),
            args.value("before"),
            args.value("after"),
        )?;
        rf.save(&roadmap)?;
        let release = &roadmap.releases[i];
        println(&format!(
            "✅ {}: {} added to {}",
            release.version,
            release.name,
            rf.rel()?
        ));
        return Ok(0);
    }

    if !rf.exists() {
        if matches!(command, "show" | "current" | "validate") {
            if args.flag("json") {
                println(if command == "show" {
                    "[]"
                } else {
                    r#"{"release": null, "open_items": [], "unscheduled": []}"#
                });
            } else {
                println(&format!("ℹ️ No roadmap at {} yet.", rf.rel()?));
            }
            return Ok(0);
        }
        return error(format!(
            "no roadmap at {}; the first 'add-release' creates it",
            rf.rel()?
        ));
    }
    let mut roadmap = rf.load(false)?;
    let backlog = load_backlog(dir)?;
    let backlog = backlog.as_ref();

    match command {
        "validate" => {
            let problems = roadmap.validate(backlog);
            for p in &problems {
                println(&format!("❌ {p}"));
            }
            for w in warnings(&roadmap, backlog) {
                println(&format!("⚠️ {w}"));
            }
            if backlog.is_none() && !roadmap.assigned().is_empty() {
                println("⚠️ No backlog file: item ids not checked.");
            }
            println(&format!(
                "\n{} roadmap: {} release(s), {} problem(s).",
                if problems.is_empty() { "✅" } else { "❌" },
                roadmap.releases.len(),
                problems.len()
            ));
            return Ok(u8::from(!problems.is_empty()));
        }
        "show" => {
            let data: Vec<Dict> = roadmap
                .releases
                .iter()
                .map(|r| {
                    let mut d = r.to_dict();
                    let items = r
                        .items()
                        .iter()
                        .map(|i| Value::Dict(describe(backlog, i)))
                        .collect();
                    d.insert("items", Value::List(items));
                    d
                })
                .collect();
            if args.flag("json") {
                let value = Value::List(data.into_iter().map(Value::Dict).collect());
                println(&dumps(&value)?);
                return Ok(0);
            }
            println(&format!("🗺️ Roadmap: {}", rf.rel()?));
            for r in &data {
                let items: Vec<&Dict> = r
                    .get("items")
                    .and_then(Value::as_list)
                    .map(|l| l.iter().filter_map(Value::as_dict).collect())
                    .unwrap_or_default();
                let done = items.iter().filter(|d| is_closed(d)).count();
                let released = text_of(r, "released");
                let target = text_of(r, "target");
                let when = if !released.is_empty() {
                    format!(", released {released}")
                } else if !target.is_empty() {
                    format!(", target {target}")
                } else {
                    String::new()
                };
                println(&format!(
                    "\n{}: {} [{}{when}] ({done}/{} items closed)",
                    text_of(r, "version"),
                    text_of(r, "name"),
                    text_of(r, "status"),
                    items.len()
                ));
                println(&format!("   Goal: {}", text_of(r, "goal")));
                for d in items {
                    println(&format!("   {}", item_line(d)));
                }
            }
            return Ok(0);
        }
        "current" => {
            let data = current_data(&roadmap, backlog);
            if args.flag("json") {
                println(&dumps(&Value::Dict(data))?);
                return Ok(0);
            }
            let open_items: Vec<&Dict> = data
                .get("open_items")
                .and_then(Value::as_list)
                .map(|l| l.iter().filter_map(Value::as_dict).collect())
                .unwrap_or_default();
            match data.get("release").and_then(Value::as_dict) {
                None => println(
                    "ℹ️ Every release in the roadmap is released; add the next one (roadmap.py add-release).",
                ),
                Some(r) => {
                    println(&format!(
                        "🎯 Current release {}: {} [{}]",
                        text_of(r, "version"),
                        text_of(r, "name"),
                        text_of(r, "status")
                    ));
                    println(&format!("   Goal: {}", text_of(r, "goal")));
                    for d in &open_items {
                        println(&format!("   {}", item_line(d)));
                    }
                    if open_items.is_empty() {
                        println(
                            "   No open item left: the release is ready for its candidate (dev-cycle-protocol Phase 8).",
                        );
                    }
                }
            }
            let unscheduled: Vec<&str> = data
                .get("unscheduled")
                .and_then(Value::as_list)
                .map(|l| l.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            if !unscheduled.is_empty() {
                println(&format!(
                    "ℹ️ {} open backlog item(s) in no release: {}",
                    unscheduled.len(),
                    unscheduled.join(", ")
                ));
            }
            return Ok(0);
        }
        _ => {}
    }

    let message = match command {
        "set-release" => {
            let status = args.value("status");
            let released = status == Some("released");
            let i = roadmap.update_release(
                args.value("version").unwrap_or(""),
                args.value("name"),
                args.value("goal"),
                args.value("target"),
                status,
                released.then(today),
            )?;
            let release = &roadmap.releases[i];
            let mut message = format!(
                "✅ {}: {} [{}]",
                release.version,
                release.name,
                release.status()
            );
            if released {
                let still_open: Vec<String> = release
                    .items()
                    .into_iter()
                    .filter(|i| OPEN.contains(&text_of(&describe(backlog, i), "status")))
                    .collect();
                if !still_open.is_empty() {
                    message.push_str(&format!(
                        "\n⚠️ Still open in this release: {}; move them (assign --move) or close them.",
                        still_open.join(", ")
                    ));
                }
            }
            message
        }
        "move-release" => {
            let version = args.value("version").unwrap_or("");
            roadmap.move_release(version, args.value("before"), args.value("after"))?;
            format!("✅ {version} moved")
        }
        "assign" => {
            let release = args.value("release").unwrap_or("");
            let move_items = args.flag("move");
            let ids = expand(backlog, &args.list("ids"), &roadmap, move_items)?;
            let added = roadmap.assign(&ids, release, move_items)?;
            if added.is_empty() {
                format!("ℹ️ Nothing to add to {release}.")
            } else {
                format!("✅ {release}: added {}", added.join(", "))
            }
        }
        "unassign" => {
            let removed = roadmap.unassign(&args.list("ids"))?;
            format!("✅ Removed {} from the roadmap", removed.join(", "))
        }
        other => return error(format!("unknown command '{other}'")),
    };
    rf.save(&roadmap)?;
    println(&message);
    Ok(0)
}

const DIR_HELP: &str = "Project directory (default: current; the Git root is used)";

fn command_parser(name: &str) -> Parser {
    let p = Parser::new(&format!("roadmap.py {name}"), "").option("--dir", DIR_HELP);
    match name {
        "add-release" => p
            .positional("version")
            .required_option("--name", "Short name, for example MVP")
            .required_option("--goal", "What the release achieves for its users")
            .option("--target", "Target date or period (free text)")
            .option("--before", "")
            .option("--after", ""),
        "set-release" => p
            .positional("version")
            .option("--name", "")
            .option("--goal", "")
            .option("--target", "'' clears it")
            .choice_option("--status", &STATUSES, ""),
        "move-release" => p
            .positional("version")
            .option("--before", "")
            .option("--after", ""),
        "assign" => p
            .positional_plus("ids")
            .required_option("--release", "")
            .flag("--move", "Move items that are in another release"),
        "unassign" => p.positional_plus("ids"),
        "show" | "current" => p.flag("--json", ""),
        _ => p,
    }
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "roadmap.py",
        "Plan the releases of the product and the backlog items each one delivers",
    )
    .commands(
        "command",
        &[
            (
                "add-release",
                "Add a release (at the end, or --before / --after another)",
            ),
            ("set-release", "Change a release"),
            ("move-release", "Reorder a release"),
            (
                "assign",
                "Put backlog items (or an epic's open items) into a release",
            ),
            ("unassign", "Take items out of the roadmap"),
            ("show", "Every release with its items"),
            (
                "current",
                "The open items of the current release (the first one not released)",
            ),
            (
                "validate",
                "Check the roadmap format and its ids against the backlog",
            ),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    match cmd(&args) {
        Ok(code) => Ok(code),
        Err(RoadmapError::Error(e)) => {
            println(&format!("❌ {e}"));
            Ok(1)
        }
        Err(RoadmapError::Crash(c)) => Err(c),
    }
}
