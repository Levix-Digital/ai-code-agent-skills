//! `extractors.py`: the text of a supported file, split into sections and chunks.

use std::path::Path;

use kernel::py::num::{is_space, strip};
use kernel::py::{splitlines, universal_newlines};

mod docx;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".docx", ".pdf", ".md", ".txt", ".csv", ".json", ".yaml", ".yml", ".xml", ".html", ".htm",
    ".log", ".py", ".js", ".ts", ".tsx", ".jsx", ".java", ".go", ".rs", ".sql", ".sh", ".bat",
    ".ps1", ".c", ".cpp", ".h", ".hpp", ".cs", ".rb", ".php",
];

pub const CLOUD_SHORTCUT_EXTENSIONS: &[&str] = &[".gdoc", ".gsheet", ".gslides"];

const IGNORE_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    ".folder_brain",
    ".chroma_db",
    ".next",
    "dist",
    "build",
    ".idea",
    ".vscode",
];

/// `is_ignored(path, root_path)`, given the parts of the path relative to the root.
pub fn is_ignored<S: AsRef<str>>(parts: &[S]) -> bool {
    parts.iter().any(|p| {
        let p = p.as_ref();
        (IGNORE_DIRS.contains(&p) || p.starts_with('.')) && p != "."
    })
}

/// A part of a document, with the heading it falls under.
#[derive(Debug, Clone)]
pub struct Section {
    pub heading: String,
    pub text: String,
    pub page: Option<usize>,
}

/// What `extract_text_from_file` returns for a document it could read.
#[derive(Debug, Clone)]
pub struct Document {
    pub content: String,
    pub sections: Vec<Section>,
    pub word_count: usize,
}

fn section(heading: &str, text: String) -> Section {
    Section {
        heading: heading.to_string(),
        text,
        page: None,
    }
}

/// `extract_text_from_file(path)` for a supported extension (`ext`, lowercase); `None`
/// when it returns an error.
pub fn extract_text_from_file(path: &Path, ext: &str) -> Option<Document> {
    let (content, mut sections) = match ext {
        ".docx" => docx::extract(path).ok()?,
        ".pdf" => pdf(path)?,
        _ => text(path)?,
    };
    let word_count = content.split(is_space).filter(|w| !w.is_empty()).count();
    if sections.is_empty() {
        sections.push(section("Geral", content.clone()));
    }
    Some(Document {
        content,
        sections,
        word_count,
    })
}

/// Text, Markdown, code, JSON, CSV: read as UTF-8 with invalid bytes dropped, sectioned by
/// `#` headings.
fn text(path: &Path) -> Option<(String, Vec<Section>)> {
    let bytes = std::fs::read(path).ok()?;
    let mut decoded = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        decoded.push_str(chunk.valid());
    }
    let content = universal_newlines(&decoded);
    let mut sections = Vec::new();
    let mut heading = "Geral".to_string();
    let mut buffer: Vec<&str> = Vec::new();
    for line in splitlines(&content) {
        if line.starts_with('#') {
            if !buffer.is_empty() {
                sections.push(section(&heading, buffer.join("\n")));
                buffer.clear();
            }
            heading = strip(line.trim_start_matches('#')).to_string();
        } else {
            buffer.push(line);
        }
    }
    if !buffer.is_empty() {
        sections.push(section(&heading, buffer.join("\n")));
    }
    Some((content, sections))
}

/// PDF: the text of each page that has any.
fn pdf(path: &Path) -> Option<(String, Vec<Section>)> {
    let path = path.to_path_buf();
    // The extraction library can panic on a malformed file; that is an error like any other,
    // and its message must not reach stderr.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let pages = std::panic::catch_unwind(move || pdf_extract::extract_text_by_pages(&path));
    std::panic::set_hook(hook);
    let pages = pages.ok()?.ok()?;
    let mut full_text = Vec::new();
    let mut sections = Vec::new();
    for (idx, page_text) in pages.iter().enumerate() {
        let text = strip(page_text);
        if !text.is_empty() {
            let n = idx + 1;
            full_text.push(format!("[Page {n}]\n{text}"));
            sections.push(Section {
                heading: format!("Page {n}"),
                text: text.to_string(),
                page: Some(n),
            });
        }
    }
    Some((full_text.join("\n\n"), sections))
}

/// A piece of a section, small enough to index.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub heading: String,
    pub text: String,
    pub page: Option<usize>,
}

const TARGET_CHUNK_SIZE: usize = 800;
const OVERLAP: usize = 150;

/// `str.rfind(sub, start, end)` over code points.
fn rfind(text: &[char], sub: &[char], start: usize, end: usize) -> Option<usize> {
    let end = end.min(text.len());
    if end < start + sub.len() {
        return None;
    }
    (start..=end - sub.len())
        .rev()
        .find(|&i| text[i..i + sub.len()] == *sub)
}

/// `chunk_document(doc_info)`: each section whole when short, else split with overlap at a
/// line or sentence break.
pub fn chunk_document(doc: &Document) -> Vec<Chunk> {
    if doc.content.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    for sec in &doc.sections {
        let text: Vec<char> = sec.text.chars().collect();
        let chunk = |t: String| Chunk {
            heading: sec.heading.clone(),
            text: t,
            page: sec.page,
        };
        if text.len() <= TARGET_CHUNK_SIZE {
            chunks.push(chunk(sec.text.clone()));
            continue;
        }
        let mut start = 0;
        while start < text.len() {
            let mut end = start + TARGET_CHUNK_SIZE;
            if end < text.len() {
                let half = start + TARGET_CHUNK_SIZE / 2;
                let mut bp = rfind(&text, &['\n'], start, end);
                if bp.is_none_or(|b| b <= half) {
                    bp = rfind(&text, &['.', ' '], start, end);
                }
                if let Some(b) = bp
                    && b > half
                {
                    end = b + 1;
                }
            }
            let piece: String = text[start..end.min(text.len())].iter().collect();
            let piece = strip(&piece);
            if !piece.is_empty() {
                chunks.push(chunk(piece.to_string()));
            }
            start = if end < text.len() {
                end - OVERLAP
            } else {
                text.len()
            };
        }
    }
    chunks
}
