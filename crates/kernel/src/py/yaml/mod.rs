//! PyYAML 6's pure-Python `safe_load` and `safe_dump`, translated stage by stage (reader and
//! scanner, parser, composer, resolver and constructor; representer, serializer and emitter)
//! so that documents load to the same values, dump to the same text and fail with the same
//! messages.
//!
//! Known gaps: an alias is a copy of its anchor's value (Python shares one object), so the
//! dumper never writes `&id001` anchors, and a document whose aliases make a value contain
//! itself fails as `RecursionError`.

mod constructor;
mod emitter;
mod parser;
mod representer;
mod scanner;

use std::fmt;
use std::rc::Rc;

use super::{Crash, Value, repr_str};

pub use representer::DumpOptions;

/// `yaml.safe_load(text)` for a `str`.
pub fn safe_load(text: &str) -> Result<Value, Crash> {
    constructor::load(text)
}

/// `yaml.safe_dump(value, **options)` to a `str`.
pub fn safe_dump(value: &Value, options: &DumpOptions) -> Result<String, Crash> {
    representer::dump(value, options)
}

/// `yaml.error.Mark`: a position in the loaded text, printed with a snippet of its line.
#[derive(Clone)]
pub(crate) struct Mark {
    pub line: usize,
    pub column: usize,
    pub pointer: usize,
    pub buffer: Rc<[char]>,
}

const SNIPPET_END: &str = "\0\r\n\u{85}\u{2028}\u{2029}";

impl Mark {
    fn snippet(&self) -> String {
        let (indent, max_half) = (4usize, 36usize);
        let buf = &self.buffer;
        let mut head = "";
        let mut start = self.pointer;
        while start > 0 && !SNIPPET_END.contains(buf[start - 1]) {
            start -= 1;
            if self.pointer - start > max_half {
                head = " ... ";
                start += 5;
                break;
            }
        }
        let mut tail = "";
        let mut end = self.pointer;
        while end < buf.len() && !SNIPPET_END.contains(buf[end]) {
            end += 1;
            if end - self.pointer > max_half {
                tail = " ... ";
                end -= 5;
                break;
            }
        }
        let snippet: String = buf[start..end].iter().collect();
        format!(
            "{}{head}{snippet}{tail}\n{}^",
            " ".repeat(indent),
            " ".repeat(indent + self.pointer - start + head.len())
        )
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "  in \"<unicode string>\", line {}, column {}:\n{}",
            self.line + 1,
            self.column + 1,
            self.snippet()
        )
    }
}

/// A `MarkedYAMLError` subclass, raised as the uncaught exception.
pub(crate) fn marked(
    class: &str,
    context: Option<&str>,
    context_mark: Option<&Mark>,
    problem: &str,
    problem_mark: Option<&Mark>,
) -> Crash {
    let mut lines: Vec<String> = Vec::new();
    if let Some(c) = context {
        lines.push(c.to_string());
    }
    if let Some(cm) = context_mark
        && problem_mark.is_none_or(|pm| cm.line != pm.line || cm.column != pm.column)
    {
        lines.push(cm.to_string());
    }
    lines.push(problem.to_string());
    if let Some(pm) = problem_mark {
        lines.push(pm.to_string());
    }
    Crash::new(format!("yaml.{class}: {}", lines.join("\n")))
}

/// `%r` of one character.
pub(crate) fn repr_char(c: char) -> String {
    repr_str(&c.to_string())
}
