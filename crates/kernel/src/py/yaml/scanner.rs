//! `yaml.reader` and `yaml.scanner`: the text as tokens.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::{Mark, marked, repr_char};
use crate::py::{Crash, surrogate_char};

pub(crate) const BREAKZ_SPACE: &str = "\0 \t\r\n\u{85}\u{2028}\u{2029}";
const BREAKZ_SPACE_NO_TAB: &str = "\0 \r\n\u{85}\u{2028}\u{2029}";
const BREAKZ: &str = "\0\r\n\u{85}\u{2028}\u{2029}";
const BREAKS: &str = "\r\n\u{85}\u{2028}\u{2029}";
const BREAKS_SPACE: &str = " \r\n\u{85}\u{2028}\u{2029}";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    StreamStart,
    StreamEnd,
    /// `%YAML major.minor`, `%TAG handle prefix` or another directive (ignored).
    Directive(String, DirectiveValue),
    DocumentStart,
    DocumentEnd,
    BlockSequenceStart,
    BlockMappingStart,
    BlockEnd,
    FlowSequenceStart,
    FlowMappingStart,
    FlowSequenceEnd,
    FlowMappingEnd,
    Key,
    Value,
    BlockEntry,
    FlowEntry,
    Alias(String),
    Anchor(String),
    Tag(Option<String>, String),
    Scalar {
        value: String,
        plain: bool,
        style: Option<char>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DirectiveValue {
    None,
    Version(String, String),
    Tag(String, String),
}

impl Kind {
    /// The token's `id`, as parser errors print it.
    pub fn id(&self) -> &'static str {
        match self {
            Kind::StreamStart => "<stream start>",
            Kind::StreamEnd => "<stream end>",
            Kind::Directive(..) => "<directive>",
            Kind::DocumentStart => "<document start>",
            Kind::DocumentEnd => "<document end>",
            Kind::BlockSequenceStart => "<block sequence start>",
            Kind::BlockMappingStart => "<block mapping start>",
            Kind::BlockEnd => "<block end>",
            Kind::FlowSequenceStart => "[",
            Kind::FlowMappingStart => "{",
            Kind::FlowSequenceEnd => "]",
            Kind::FlowMappingEnd => "}",
            Kind::Key => "?",
            Kind::Value => ":",
            Kind::BlockEntry => "-",
            Kind::FlowEntry => ",",
            Kind::Alias(_) => "<alias>",
            Kind::Anchor(_) => "<anchor>",
            Kind::Tag(..) => "<tag>",
            Kind::Scalar { .. } => "<scalar>",
        }
    }
}

#[derive(Clone)]
pub(crate) struct Token {
    pub kind: Kind,
    pub start: Mark,
    pub end: Mark,
}

struct SimpleKey {
    token_number: usize,
    required: bool,
    index: usize,
    line: usize,
    column: usize,
    mark: Mark,
}

pub(crate) struct Scanner {
    buffer: Rc<[char]>,
    pointer: usize,
    line: usize,
    column: usize,
    done: bool,
    flow_level: usize,
    tokens: Vec<Token>,
    tokens_taken: usize,
    indent: isize,
    indents: Vec<isize>,
    allow_simple_key: bool,
    possible_simple_keys: BTreeMap<usize, SimpleKey>,
}

fn scanner_error(context: Option<&str>, cm: Option<&Mark>, problem: &str, pm: &Mark) -> Crash {
    marked("scanner.ScannerError", context, cm, problem, Some(pm))
}

/// `Reader.check_printable`.
fn check_printable(text: &[char]) -> Result<(), Crash> {
    let bad = text.iter().position(|&c| {
        let u = c as u32;
        !matches!(u, 0x09 | 0x0a | 0x0d | 0x20..=0x7e | 0x85 | 0xa0..=0xd7ff | 0xe000..=0xfffd | 0x10000..)
    });
    match bad {
        None => Ok(()),
        Some(pos) => {
            let code = text[pos] as u32;
            Err(Crash::new(format!(
                "yaml.reader.ReaderError: unacceptable character #x{code:04x}: special characters are not allowed\n  in \"<unicode string>\", position {pos}"
            )))
        }
    }
}

fn is_alnum(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn hex_value(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap_or(0)
}

impl Scanner {
    pub fn new(text: &str) -> Result<Self, Crash> {
        let mut chars: Vec<char> = text.chars().collect();
        check_printable(&chars)?;
        chars.push('\0');
        let mut s = Scanner {
            buffer: chars.into(),
            pointer: 0,
            line: 0,
            column: 0,
            done: false,
            flow_level: 0,
            tokens: Vec::new(),
            tokens_taken: 0,
            indent: -1,
            indents: Vec::new(),
            allow_simple_key: true,
            possible_simple_keys: BTreeMap::new(),
        };
        let mark = s.mark();
        s.tokens.push(Token {
            kind: Kind::StreamStart,
            start: mark.clone(),
            end: mark,
        });
        Ok(s)
    }

    // ------------------------------------------------------------- reader

    fn peek_at(&self, i: usize) -> char {
        self.buffer.get(self.pointer + i).copied().unwrap_or('\0')
    }

    fn peek(&self) -> char {
        self.peek_at(0)
    }

    fn prefix(&self, length: usize) -> String {
        let end = (self.pointer + length).min(self.buffer.len());
        self.buffer[self.pointer..end].iter().collect()
    }

    fn forward(&mut self, mut length: usize) {
        while length > 0 {
            let ch = self.buffer[self.pointer];
            self.pointer += 1;
            if "\n\u{85}\u{2028}\u{2029}".contains(ch) || (ch == '\r' && self.peek() != '\n') {
                self.line += 1;
                self.column = 0;
            } else if ch != '\u{feff}' {
                self.column += 1;
            }
            length -= 1;
        }
    }

    fn mark(&self) -> Mark {
        Mark {
            line: self.line,
            column: self.column,
            pointer: self.pointer,
            buffer: self.buffer.clone(),
        }
    }

    // ------------------------------------------------------------- public

    fn fill(&mut self) -> Result<(), Crash> {
        while self.need_more_tokens()? {
            self.fetch_more_tokens()?;
        }
        Ok(())
    }

    pub fn peek_token(&mut self) -> Result<Option<&Token>, Crash> {
        self.fill()?;
        Ok(self.tokens.first())
    }

    pub fn get_token(&mut self) -> Result<Option<Token>, Crash> {
        self.fill()?;
        if self.tokens.is_empty() {
            return Ok(None);
        }
        self.tokens_taken += 1;
        Ok(Some(self.tokens.remove(0)))
    }

    fn need_more_tokens(&mut self) -> Result<bool, Crash> {
        if self.done {
            return Ok(false);
        }
        if self.tokens.is_empty() {
            return Ok(true);
        }
        self.stale_possible_simple_keys()?;
        Ok(self.next_possible_simple_key() == Some(self.tokens_taken))
    }

    fn fetch_more_tokens(&mut self) -> Result<(), Crash> {
        self.scan_to_next_token();
        self.stale_possible_simple_keys()?;
        self.unwind_indent(self.column as isize);
        let ch = self.peek();
        match ch {
            '\0' => return self.fetch_stream_end(),
            '%' if self.column == 0 => return self.fetch_directive(),
            '-' if self.check_document_indicator("---") => {
                return self.fetch_document_indicator(Kind::DocumentStart);
            }
            '.' if self.check_document_indicator("...") => {
                return self.fetch_document_indicator(Kind::DocumentEnd);
            }
            '[' => return self.fetch_flow_collection_start(Kind::FlowSequenceStart),
            '{' => return self.fetch_flow_collection_start(Kind::FlowMappingStart),
            ']' => return self.fetch_flow_collection_end(Kind::FlowSequenceEnd),
            '}' => return self.fetch_flow_collection_end(Kind::FlowMappingEnd),
            ',' => return self.fetch_flow_entry(),
            _ => {}
        }
        let next_is_blank = BREAKZ_SPACE.contains(self.peek_at(1));
        match ch {
            '-' if next_is_blank => return self.fetch_block_entry(),
            '?' if self.flow_level > 0 || next_is_blank => return self.fetch_key(),
            ':' if self.flow_level > 0 || next_is_blank => return self.fetch_value(),
            '*' => return self.fetch_anchor(true),
            '&' => return self.fetch_anchor(false),
            '!' => return self.fetch_tag(),
            '|' if self.flow_level == 0 => return self.fetch_block_scalar(false),
            '>' if self.flow_level == 0 => return self.fetch_block_scalar(true),
            '\'' => return self.fetch_flow_scalar(false),
            '"' => return self.fetch_flow_scalar(true),
            _ => {}
        }
        if self.check_plain() {
            return self.fetch_plain();
        }
        Err(scanner_error(
            Some("while scanning for the next token"),
            None,
            &format!(
                "found character {} that cannot start any token",
                repr_char(ch)
            ),
            &self.mark(),
        ))
    }

    // ------------------------------------------------------------- simple keys

    fn next_possible_simple_key(&self) -> Option<usize> {
        self.possible_simple_keys
            .values()
            .map(|k| k.token_number)
            .min()
    }

    fn stale_possible_simple_keys(&mut self) -> Result<(), Crash> {
        let levels: Vec<usize> = self.possible_simple_keys.keys().copied().collect();
        for level in levels {
            let key = &self.possible_simple_keys[&level];
            if key.line != self.line || self.pointer - key.index > 1024 {
                if key.required {
                    return Err(scanner_error(
                        Some("while scanning a simple key"),
                        Some(&key.mark),
                        "could not find expected ':'",
                        &self.mark(),
                    ));
                }
                self.possible_simple_keys.remove(&level);
            }
        }
        Ok(())
    }

    fn save_possible_simple_key(&mut self) -> Result<(), Crash> {
        let required = self.flow_level == 0 && self.indent == self.column as isize;
        if self.allow_simple_key {
            self.remove_possible_simple_key()?;
            let key = SimpleKey {
                token_number: self.tokens_taken + self.tokens.len(),
                required,
                index: self.pointer,
                line: self.line,
                column: self.column,
                mark: self.mark(),
            };
            self.possible_simple_keys.insert(self.flow_level, key);
        }
        Ok(())
    }

    fn remove_possible_simple_key(&mut self) -> Result<(), Crash> {
        if let Some(key) = self.possible_simple_keys.get(&self.flow_level) {
            if key.required {
                return Err(scanner_error(
                    Some("while scanning a simple key"),
                    Some(&key.mark),
                    "could not find expected ':'",
                    &self.mark(),
                ));
            }
            self.possible_simple_keys.remove(&self.flow_level);
        }
        Ok(())
    }

    // ------------------------------------------------------------- indentation

    fn unwind_indent(&mut self, column: isize) {
        if self.flow_level > 0 {
            return;
        }
        while self.indent > column {
            let mark = self.mark();
            self.indent = self.indents.pop().unwrap_or(-1);
            self.push(Kind::BlockEnd, mark.clone(), mark);
        }
    }

    fn add_indent(&mut self, column: usize) -> bool {
        if self.indent < column as isize {
            self.indents.push(self.indent);
            self.indent = column as isize;
            return true;
        }
        false
    }

    fn push(&mut self, kind: Kind, start: Mark, end: Mark) {
        self.tokens.push(Token { kind, start, end });
    }

    /// The one-character token at the cursor.
    fn push_forward(&mut self, kind: Kind) {
        let start = self.mark();
        self.forward(1);
        let end = self.mark();
        self.push(kind, start, end);
    }

    // ------------------------------------------------------------- fetchers

    fn fetch_stream_end(&mut self) -> Result<(), Crash> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        self.possible_simple_keys.clear();
        let mark = self.mark();
        self.push(Kind::StreamEnd, mark.clone(), mark);
        self.done = true;
        Ok(())
    }

    fn fetch_directive(&mut self) -> Result<(), Crash> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let token = self.scan_directive()?;
        self.tokens.push(token);
        Ok(())
    }

    fn check_document_indicator(&self, indicator: &str) -> bool {
        self.column == 0 && self.prefix(3) == indicator && BREAKZ_SPACE.contains(self.peek_at(3))
    }

    fn fetch_document_indicator(&mut self, kind: Kind) -> Result<(), Crash> {
        self.unwind_indent(-1);
        self.remove_possible_simple_key()?;
        self.allow_simple_key = false;
        let start = self.mark();
        self.forward(3);
        let end = self.mark();
        self.push(kind, start, end);
        Ok(())
    }

    fn fetch_flow_collection_start(&mut self, kind: Kind) -> Result<(), Crash> {
        self.save_possible_simple_key()?;
        self.flow_level = self.flow_level.wrapping_add(1);
        self.allow_simple_key = true;
        self.push_forward(kind);
        Ok(())
    }

    fn fetch_flow_collection_end(&mut self, kind: Kind) -> Result<(), Crash> {
        self.remove_possible_simple_key()?;
        // Python's flow_level goes below zero on a stray `]`; only `!= 0` is ever tested
        // and the next `[` brings it back, so a saturating counter would differ: keep a
        // wrapping one.
        self.flow_level = self.flow_level.wrapping_sub(1);
        self.allow_simple_key = false;
        self.push_forward(kind);
        Ok(())
    }

    fn fetch_flow_entry(&mut self) -> Result<(), Crash> {
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        self.push_forward(Kind::FlowEntry);
        Ok(())
    }

    fn fetch_block_entry(&mut self) -> Result<(), Crash> {
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(scanner_error(
                    None,
                    None,
                    "sequence entries are not allowed here",
                    &self.mark(),
                ));
            }
            if self.add_indent(self.column) {
                let mark = self.mark();
                self.push(Kind::BlockSequenceStart, mark.clone(), mark);
            }
        }
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        self.push_forward(Kind::BlockEntry);
        Ok(())
    }

    fn fetch_key(&mut self) -> Result<(), Crash> {
        if self.flow_level == 0 {
            if !self.allow_simple_key {
                return Err(scanner_error(
                    None,
                    None,
                    "mapping keys are not allowed here",
                    &self.mark(),
                ));
            }
            if self.add_indent(self.column) {
                let mark = self.mark();
                self.push(Kind::BlockMappingStart, mark.clone(), mark);
            }
        }
        self.allow_simple_key = self.flow_level == 0;
        self.remove_possible_simple_key()?;
        self.push_forward(Kind::Key);
        Ok(())
    }

    fn fetch_value(&mut self) -> Result<(), Crash> {
        if let Some(key) = self.possible_simple_keys.remove(&self.flow_level) {
            let at = key.token_number - self.tokens_taken;
            self.tokens.insert(
                at,
                Token {
                    kind: Kind::Key,
                    start: key.mark.clone(),
                    end: key.mark.clone(),
                },
            );
            if self.flow_level == 0 && self.add_indent(key.column) {
                self.tokens.insert(
                    at,
                    Token {
                        kind: Kind::BlockMappingStart,
                        start: key.mark.clone(),
                        end: key.mark,
                    },
                );
            }
            self.allow_simple_key = false;
        } else {
            if self.flow_level == 0 {
                if !self.allow_simple_key {
                    return Err(scanner_error(
                        None,
                        None,
                        "mapping values are not allowed here",
                        &self.mark(),
                    ));
                }
                if self.add_indent(self.column) {
                    let mark = self.mark();
                    self.push(Kind::BlockMappingStart, mark.clone(), mark);
                }
            }
            self.allow_simple_key = self.flow_level == 0;
            self.remove_possible_simple_key()?;
        }
        self.push_forward(Kind::Value);
        Ok(())
    }

    fn fetch_anchor(&mut self, alias: bool) -> Result<(), Crash> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let token = self.scan_anchor(alias)?;
        self.tokens.push(token);
        Ok(())
    }

    fn fetch_tag(&mut self) -> Result<(), Crash> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let token = self.scan_tag()?;
        self.tokens.push(token);
        Ok(())
    }

    fn fetch_block_scalar(&mut self, folded: bool) -> Result<(), Crash> {
        self.allow_simple_key = true;
        self.remove_possible_simple_key()?;
        let token = self.scan_block_scalar(folded)?;
        self.tokens.push(token);
        Ok(())
    }

    fn fetch_flow_scalar(&mut self, double: bool) -> Result<(), Crash> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let token = self.scan_flow_scalar(double)?;
        self.tokens.push(token);
        Ok(())
    }

    fn fetch_plain(&mut self) -> Result<(), Crash> {
        self.save_possible_simple_key()?;
        self.allow_simple_key = false;
        let token = self.scan_plain()?;
        self.tokens.push(token);
        Ok(())
    }

    fn check_plain(&self) -> bool {
        let ch = self.peek();
        !"\0 \t\r\n\u{85}\u{2028}\u{2029}-?:,[]{}#&*!|>'\"%@`".contains(ch)
            || (!BREAKZ_SPACE.contains(self.peek_at(1))
                && (ch == '-' || (self.flow_level == 0 && "?:".contains(ch))))
    }

    // ------------------------------------------------------------- scanners

    fn scan_to_next_token(&mut self) {
        if self.pointer == 0 && self.peek() == '\u{feff}' {
            self.forward(1);
        }
        loop {
            while self.peek() == ' ' {
                self.forward(1);
            }
            if self.peek() == '#' {
                while !BREAKZ.contains(self.peek()) {
                    self.forward(1);
                }
            }
            if self.scan_line_break().is_empty() {
                break;
            }
            if self.flow_level == 0 {
                self.allow_simple_key = true;
            }
        }
    }

    fn directive_error(&self, start: &Mark, problem: String) -> Crash {
        scanner_error(
            Some("while scanning a directive"),
            Some(start),
            &problem,
            &self.mark(),
        )
    }

    fn scan_directive(&mut self) -> Result<Token, Crash> {
        let start = self.mark();
        self.forward(1);
        let name = self.scan_directive_name(&start)?;
        let value;
        let end;
        if name == "YAML" {
            value = self.scan_yaml_directive_value(&start)?;
            end = self.mark();
        } else if name == "TAG" {
            value = self.scan_tag_directive_value(&start)?;
            end = self.mark();
        } else {
            value = DirectiveValue::None;
            end = self.mark();
            while !BREAKZ.contains(self.peek()) {
                self.forward(1);
            }
        }
        self.scan_directive_ignored_line(&start)?;
        Ok(Token {
            kind: Kind::Directive(name, value),
            start,
            end,
        })
    }

    fn scan_directive_name(&mut self, start: &Mark) -> Result<String, Crash> {
        let mut length = 0;
        let mut ch = self.peek_at(length);
        while is_alnum(ch) {
            length += 1;
            ch = self.peek_at(length);
        }
        if length == 0 {
            return Err(self.directive_error(
                start,
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        let value = self.prefix(length);
        self.forward(length);
        let ch = self.peek();
        if !BREAKZ_SPACE_NO_TAB.contains(ch) {
            return Err(self.directive_error(
                start,
                format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        Ok(value)
    }

    fn scan_yaml_directive_value(&mut self, start: &Mark) -> Result<DirectiveValue, Crash> {
        while self.peek() == ' ' {
            self.forward(1);
        }
        let major = self.scan_yaml_directive_number(start)?;
        if self.peek() != '.' {
            return Err(self.directive_error(
                start,
                format!(
                    "expected a digit or '.', but found {}",
                    repr_char(self.peek())
                ),
            ));
        }
        self.forward(1);
        let minor = self.scan_yaml_directive_number(start)?;
        if !BREAKZ_SPACE_NO_TAB.contains(self.peek()) {
            return Err(self.directive_error(
                start,
                format!(
                    "expected a digit or ' ', but found {}",
                    repr_char(self.peek())
                ),
            ));
        }
        Ok(DirectiveValue::Version(major, minor))
    }

    /// The digits of a version number, as `int()` reads them (leading zeros dropped).
    fn scan_yaml_directive_number(&mut self, start: &Mark) -> Result<String, Crash> {
        let ch = self.peek();
        if !ch.is_ascii_digit() {
            return Err(self.directive_error(
                start,
                format!("expected a digit, but found {}", repr_char(ch)),
            ));
        }
        let mut length = 0;
        while self.peek_at(length).is_ascii_digit() {
            length += 1;
        }
        let text = self.prefix(length);
        self.forward(length);
        let trimmed = text.trim_start_matches('0');
        Ok(if trimmed.is_empty() {
            "0".into()
        } else {
            trimmed.into()
        })
    }

    fn scan_tag_directive_value(&mut self, start: &Mark) -> Result<DirectiveValue, Crash> {
        while self.peek() == ' ' {
            self.forward(1);
        }
        let handle = self.scan_tag_handle("directive", start)?;
        let ch = self.peek();
        if ch != ' ' {
            return Err(
                self.directive_error(start, format!("expected ' ', but found {}", repr_char(ch)))
            );
        }
        while self.peek() == ' ' {
            self.forward(1);
        }
        let prefix = self.scan_tag_uri("directive", start)?;
        let ch = self.peek();
        if !BREAKZ_SPACE_NO_TAB.contains(ch) {
            return Err(
                self.directive_error(start, format!("expected ' ', but found {}", repr_char(ch)))
            );
        }
        Ok(DirectiveValue::Tag(handle, prefix))
    }

    fn scan_directive_ignored_line(&mut self, start: &Mark) -> Result<(), Crash> {
        while self.peek() == ' ' {
            self.forward(1);
        }
        if self.peek() == '#' {
            while !BREAKZ.contains(self.peek()) {
                self.forward(1);
            }
        }
        let ch = self.peek();
        if !BREAKZ.contains(ch) {
            return Err(self.directive_error(
                start,
                format!(
                    "expected a comment or a line break, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        self.scan_line_break();
        Ok(())
    }

    fn scan_anchor(&mut self, alias: bool) -> Result<Token, Crash> {
        let start = self.mark();
        let name = if alias { "alias" } else { "anchor" };
        self.forward(1);
        let mut length = 0;
        let mut ch = self.peek_at(length);
        while is_alnum(ch) {
            length += 1;
            ch = self.peek_at(length);
        }
        let error = |s: &Self, ch: char| {
            scanner_error(
                Some(&format!("while scanning an {name}")),
                Some(&start),
                &format!(
                    "expected alphabetic or numeric character, but found {}",
                    repr_char(ch)
                ),
                &s.mark(),
            )
        };
        if length == 0 {
            return Err(error(self, ch));
        }
        let value = self.prefix(length);
        self.forward(length);
        let ch = self.peek();
        if !"\0 \t\r\n\u{85}\u{2028}\u{2029}?:,]}%@`".contains(ch) {
            return Err(error(self, ch));
        }
        let end = self.mark();
        let kind = if alias {
            Kind::Alias(value)
        } else {
            Kind::Anchor(value)
        };
        Ok(Token { kind, start, end })
    }

    fn scan_tag(&mut self) -> Result<Token, Crash> {
        let start = self.mark();
        let mut ch = self.peek_at(1);
        let handle;
        let suffix;
        if ch == '<' {
            handle = None;
            self.forward(2);
            suffix = self.scan_tag_uri("tag", &start)?;
            if self.peek() != '>' {
                return Err(scanner_error(
                    Some("while parsing a tag"),
                    Some(&start),
                    &format!("expected '>', but found {}", repr_char(self.peek())),
                    &self.mark(),
                ));
            }
            self.forward(1);
        } else if BREAKZ_SPACE.contains(ch) {
            handle = None;
            suffix = "!".to_string();
            self.forward(1);
        } else {
            let mut length = 1;
            let mut use_handle = false;
            while !BREAKZ_SPACE_NO_TAB.contains(ch) {
                if ch == '!' {
                    use_handle = true;
                    break;
                }
                length += 1;
                ch = self.peek_at(length);
            }
            if use_handle {
                handle = Some(self.scan_tag_handle("tag", &start)?);
            } else {
                handle = Some("!".to_string());
                self.forward(1);
            }
            suffix = self.scan_tag_uri("tag", &start)?;
        }
        let ch = self.peek();
        if !BREAKZ_SPACE_NO_TAB.contains(ch) {
            return Err(scanner_error(
                Some("while scanning a tag"),
                Some(&start),
                &format!("expected ' ', but found {}", repr_char(ch)),
                &self.mark(),
            ));
        }
        let end = self.mark();
        Ok(Token {
            kind: Kind::Tag(handle, suffix),
            start,
            end,
        })
    }

    fn block_scalar_error(&self, start: &Mark, problem: String) -> Crash {
        scanner_error(
            Some("while scanning a block scalar"),
            Some(start),
            &problem,
            &self.mark(),
        )
    }

    fn scan_block_scalar(&mut self, folded: bool) -> Result<Token, Crash> {
        let mut chunks = String::new();
        let start = self.mark();
        self.forward(1);
        let (chomping, increment) = self.scan_block_scalar_indicators(&start)?;
        self.scan_block_scalar_ignored_line(&start)?;
        let min_indent = (self.indent + 1).max(1) as usize;
        let (mut breaks, mut end, indent) = match increment {
            None => {
                let (breaks, max_indent, end) = self.scan_block_scalar_indentation();
                (breaks, end, min_indent.max(max_indent))
            }
            Some(inc) => {
                let indent = min_indent + inc - 1;
                let (breaks, end) = self.scan_block_scalar_breaks(indent);
                (breaks, end, indent)
            }
        };
        let mut line_break = String::new();
        while self.column == indent && self.peek() != '\0' {
            chunks.push_str(&breaks.concat());
            let leading_non_space = !" \t".contains(self.peek());
            let mut length = 0;
            while !BREAKZ.contains(self.peek_at(length)) {
                length += 1;
            }
            chunks.push_str(&self.prefix(length));
            self.forward(length);
            line_break = self.scan_line_break();
            let (b, e) = self.scan_block_scalar_breaks(indent);
            breaks = b;
            end = e;
            if self.column == indent && self.peek() != '\0' {
                if folded && line_break == "\n" && leading_non_space && !" \t".contains(self.peek())
                {
                    if breaks.is_empty() {
                        chunks.push(' ');
                    }
                } else {
                    chunks.push_str(&line_break);
                }
            } else {
                break;
            }
        }
        if chomping != Some(false) {
            chunks.push_str(&line_break);
        }
        if chomping == Some(true) {
            chunks.push_str(&breaks.concat());
        }
        Ok(Token {
            kind: Kind::Scalar {
                value: chunks,
                plain: false,
                style: Some(if folded { '>' } else { '|' }),
            },
            start,
            end,
        })
    }

    fn scan_block_scalar_indicators(
        &mut self,
        start: &Mark,
    ) -> Result<(Option<bool>, Option<usize>), Crash> {
        let mut chomping = None;
        let mut increment = None;
        let zero = "expected indentation indicator in the range 1-9, but found 0";
        let mut ch = self.peek();
        if ch == '+' || ch == '-' {
            chomping = Some(ch == '+');
            self.forward(1);
            ch = self.peek();
            if let Some(d) = ch.to_digit(10) {
                if d == 0 {
                    return Err(self.block_scalar_error(start, zero.into()));
                }
                increment = Some(d as usize);
                self.forward(1);
            }
        } else if let Some(d) = ch.to_digit(10) {
            if d == 0 {
                return Err(self.block_scalar_error(start, zero.into()));
            }
            increment = Some(d as usize);
            self.forward(1);
            ch = self.peek();
            if ch == '+' || ch == '-' {
                chomping = Some(ch == '+');
                self.forward(1);
            }
        }
        let ch = self.peek();
        if !BREAKZ_SPACE_NO_TAB.contains(ch) {
            return Err(self.block_scalar_error(
                start,
                format!(
                    "expected chomping or indentation indicators, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        Ok((chomping, increment))
    }

    fn scan_block_scalar_ignored_line(&mut self, start: &Mark) -> Result<(), Crash> {
        while self.peek() == ' ' {
            self.forward(1);
        }
        if self.peek() == '#' {
            while !BREAKZ.contains(self.peek()) {
                self.forward(1);
            }
        }
        let ch = self.peek();
        if !BREAKZ.contains(ch) {
            return Err(self.block_scalar_error(
                start,
                format!(
                    "expected a comment or a line break, but found {}",
                    repr_char(ch)
                ),
            ));
        }
        self.scan_line_break();
        Ok(())
    }

    fn scan_block_scalar_indentation(&mut self) -> (Vec<String>, usize, Mark) {
        let mut chunks = Vec::new();
        let mut max_indent = 0;
        let mut end = self.mark();
        while BREAKS_SPACE.contains(self.peek()) {
            if self.peek() != ' ' {
                chunks.push(self.scan_line_break());
                end = self.mark();
            } else {
                self.forward(1);
                max_indent = max_indent.max(self.column);
            }
        }
        (chunks, max_indent, end)
    }

    fn scan_block_scalar_breaks(&mut self, indent: usize) -> (Vec<String>, Mark) {
        let mut chunks = Vec::new();
        let mut end = self.mark();
        while self.column < indent && self.peek() == ' ' {
            self.forward(1);
        }
        while BREAKS.contains(self.peek()) {
            chunks.push(self.scan_line_break());
            end = self.mark();
            while self.column < indent && self.peek() == ' ' {
                self.forward(1);
            }
        }
        (chunks, end)
    }

    fn scan_flow_scalar(&mut self, double: bool) -> Result<Token, Crash> {
        let mut chunks = String::new();
        let start = self.mark();
        let quote = self.peek();
        self.forward(1);
        self.scan_flow_scalar_non_spaces(double, &start, &mut chunks)?;
        while self.peek() != quote {
            self.scan_flow_scalar_spaces(double, &start, &mut chunks)?;
            self.scan_flow_scalar_non_spaces(double, &start, &mut chunks)?;
        }
        self.forward(1);
        let end = self.mark();
        Ok(Token {
            kind: Kind::Scalar {
                value: chunks,
                plain: false,
                style: Some(if double { '"' } else { '\'' }),
            },
            start,
            end,
        })
    }

    fn scan_flow_scalar_non_spaces(
        &mut self,
        double: bool,
        start: &Mark,
        chunks: &mut String,
    ) -> Result<(), Crash> {
        loop {
            let mut length = 0;
            while !"'\"\\\0 \t\r\n\u{85}\u{2028}\u{2029}".contains(self.peek_at(length)) {
                length += 1;
            }
            if length > 0 {
                chunks.push_str(&self.prefix(length));
                self.forward(length);
            }
            let ch = self.peek();
            if !double && ch == '\'' && self.peek_at(1) == '\'' {
                chunks.push('\'');
                self.forward(2);
            } else if (double && ch == '\'') || (!double && (ch == '"' || ch == '\\')) {
                chunks.push(ch);
                self.forward(1);
            } else if double && ch == '\\' {
                self.forward(1);
                let ch = self.peek();
                if let Some(r) = escape_replacement(ch) {
                    chunks.push(r);
                    self.forward(1);
                } else if let Some(length) = escape_code_length(ch) {
                    self.forward(1);
                    for k in 0..length {
                        if !self.peek_at(k).is_ascii_hexdigit() {
                            return Err(scanner_error(
                                Some("while scanning a double-quoted scalar"),
                                Some(start),
                                &format!(
                                    "expected escape sequence of {length} hexadecimal numbers, but found {}",
                                    repr_char(self.peek_at(k))
                                ),
                                &self.mark(),
                            ));
                        }
                    }
                    let code = hex_value(&self.prefix(length));
                    chunks.push(chr(code)?);
                    self.forward(length);
                } else if BREAKS.contains(ch) {
                    self.scan_line_break();
                    self.scan_flow_scalar_breaks(start, chunks)?;
                } else {
                    return Err(scanner_error(
                        Some("while scanning a double-quoted scalar"),
                        Some(start),
                        &format!("found unknown escape character {}", repr_char(ch)),
                        &self.mark(),
                    ));
                }
            } else {
                return Ok(());
            }
        }
    }

    fn scan_flow_scalar_spaces(
        &mut self,
        _double: bool,
        start: &Mark,
        chunks: &mut String,
    ) -> Result<(), Crash> {
        let mut length = 0;
        while " \t".contains(self.peek_at(length)) {
            length += 1;
        }
        let whitespaces = self.prefix(length);
        self.forward(length);
        let ch = self.peek();
        if ch == '\0' {
            return Err(scanner_error(
                Some("while scanning a quoted scalar"),
                Some(start),
                "found unexpected end of stream",
                &self.mark(),
            ));
        } else if BREAKS.contains(ch) {
            let line_break = self.scan_line_break();
            let mut breaks = String::new();
            self.scan_flow_scalar_breaks(start, &mut breaks)?;
            if line_break != "\n" {
                chunks.push_str(&line_break);
            } else if breaks.is_empty() {
                chunks.push(' ');
            }
            chunks.push_str(&breaks);
        } else {
            chunks.push_str(&whitespaces);
        }
        Ok(())
    }

    fn scan_flow_scalar_breaks(&mut self, start: &Mark, chunks: &mut String) -> Result<(), Crash> {
        loop {
            let prefix = self.prefix(3);
            if (prefix == "---" || prefix == "...") && BREAKZ_SPACE.contains(self.peek_at(3)) {
                return Err(scanner_error(
                    Some("while scanning a quoted scalar"),
                    Some(start),
                    "found unexpected document separator",
                    &self.mark(),
                ));
            }
            while " \t".contains(self.peek()) {
                self.forward(1);
            }
            if BREAKS.contains(self.peek()) {
                chunks.push_str(&self.scan_line_break());
            } else {
                return Ok(());
            }
        }
    }

    fn scan_plain(&mut self) -> Result<Token, Crash> {
        let mut chunks = String::new();
        let start = self.mark();
        let mut end = start.clone();
        let indent = self.indent + 1;
        let mut spaces: Vec<String> = Vec::new();
        loop {
            let mut length = 0;
            if self.peek() == '#' {
                break;
            }
            loop {
                let ch = self.peek_at(length);
                if BREAKZ_SPACE.contains(ch)
                    || (ch == ':'
                        && (BREAKZ_SPACE.contains(self.peek_at(length + 1))
                            || (self.flow_level > 0 && ",[]{}".contains(self.peek_at(length + 1)))))
                    || (self.flow_level > 0 && ",?[]{}".contains(ch))
                {
                    break;
                }
                length += 1;
            }
            if length == 0 {
                break;
            }
            self.allow_simple_key = false;
            chunks.push_str(&spaces.concat());
            chunks.push_str(&self.prefix(length));
            self.forward(length);
            end = self.mark();
            spaces = self.scan_plain_spaces();
            if spaces.is_empty()
                || self.peek() == '#'
                || (self.flow_level == 0 && (self.column as isize) < indent)
            {
                break;
            }
        }
        Ok(Token {
            kind: Kind::Scalar {
                value: chunks,
                plain: true,
                style: None,
            },
            start,
            end,
        })
    }

    /// Python returns `None` (no more of the scalar) at a document separator: an empty list
    /// here, which the caller treats the same way.
    fn scan_plain_spaces(&mut self) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut length = 0;
        while self.peek_at(length) == ' ' {
            length += 1;
        }
        let whitespaces = self.prefix(length);
        self.forward(length);
        let ch = self.peek();
        let separator = |s: &Self| {
            let prefix = s.prefix(3);
            (prefix == "---" || prefix == "...") && BREAKZ_SPACE.contains(s.peek_at(3))
        };
        if BREAKS.contains(ch) {
            let line_break = self.scan_line_break();
            self.allow_simple_key = true;
            if separator(self) {
                return Vec::new();
            }
            let mut breaks = Vec::new();
            while BREAKS_SPACE.contains(self.peek()) {
                if self.peek() == ' ' {
                    self.forward(1);
                } else {
                    breaks.push(self.scan_line_break());
                    if separator(self) {
                        return Vec::new();
                    }
                }
            }
            if line_break != "\n" {
                chunks.push(line_break);
            } else if breaks.is_empty() {
                chunks.push(" ".into());
            }
            chunks.extend(breaks);
        } else if !whitespaces.is_empty() {
            chunks.push(whitespaces);
        }
        chunks
    }

    fn scan_tag_handle(&mut self, name: &str, start: &Mark) -> Result<String, Crash> {
        let error = |s: &Self, ch: char| {
            scanner_error(
                Some(&format!("while scanning a {name}")),
                Some(start),
                &format!("expected '!', but found {}", repr_char(ch)),
                &s.mark(),
            )
        };
        let ch = self.peek();
        if ch != '!' {
            return Err(error(self, ch));
        }
        let mut length = 1;
        let mut ch = self.peek_at(length);
        if ch != ' ' {
            while is_alnum(ch) {
                length += 1;
                ch = self.peek_at(length);
            }
            if ch != '!' {
                self.forward(length);
                return Err(error(self, ch));
            }
            length += 1;
        }
        let value = self.prefix(length);
        self.forward(length);
        Ok(value)
    }

    fn scan_tag_uri(&mut self, name: &str, start: &Mark) -> Result<String, Crash> {
        let mut chunks = String::new();
        let mut length = 0;
        let mut ch = self.peek_at(length);
        while ch.is_ascii_alphanumeric() || "-;/?:@&=+$,_.!~*'()[]%".contains(ch) {
            if ch == '%' {
                chunks.push_str(&self.prefix(length));
                self.forward(length);
                length = 0;
                chunks.push_str(&self.scan_uri_escapes(name, start)?);
            } else {
                length += 1;
            }
            ch = self.peek_at(length);
        }
        if length > 0 {
            chunks.push_str(&self.prefix(length));
            self.forward(length);
        }
        if chunks.is_empty() {
            return Err(scanner_error(
                Some(&format!("while parsing a {name}")),
                Some(start),
                &format!("expected URI, but found {}", repr_char(ch)),
                &self.mark(),
            ));
        }
        Ok(chunks)
    }

    fn scan_uri_escapes(&mut self, name: &str, start: &Mark) -> Result<String, Crash> {
        let mut codes = Vec::new();
        let mark = self.mark();
        while self.peek() == '%' {
            self.forward(1);
            for k in 0..2 {
                if !self.peek_at(k).is_ascii_hexdigit() {
                    return Err(scanner_error(
                        Some(&format!("while scanning a {name}")),
                        Some(start),
                        &format!(
                            "expected URI escape sequence of 2 hexadecimal numbers, but found {}",
                            repr_char(self.peek_at(k))
                        ),
                        &self.mark(),
                    ));
                }
            }
            codes.push(hex_value(&self.prefix(2)) as u8);
            self.forward(2);
        }
        match String::from_utf8(codes) {
            Ok(s) => Ok(s),
            Err(e) => Err(marked(
                "scanner.ScannerError",
                Some(&format!("while scanning a {name}")),
                Some(start),
                &crate::py::utf8_error_text(e.as_bytes(), &e.utf8_error()),
                Some(&mark),
            )),
        }
    }

    fn scan_line_break(&mut self) -> String {
        let ch = self.peek();
        if "\r\n\u{85}".contains(ch) {
            if self.prefix(2) == "\r\n" {
                self.forward(2);
            } else {
                self.forward(1);
            }
            "\n".into()
        } else if ch == '\u{2028}' || ch == '\u{2029}' {
            self.forward(1);
            ch.to_string()
        } else {
            String::new()
        }
    }
}

fn escape_replacement(c: char) -> Option<char> {
    Some(match c {
        '0' => '\0',
        'a' => '\x07',
        'b' => '\x08',
        't' | '\t' => '\t',
        'n' => '\n',
        'v' => '\x0b',
        'f' => '\x0c',
        'r' => '\r',
        'e' => '\x1b',
        ' ' => ' ',
        '"' => '"',
        '\\' => '\\',
        '/' => '/',
        'N' => '\u{85}',
        '_' => '\u{a0}',
        'L' => '\u{2028}',
        'P' => '\u{2029}',
        _ => return None,
    })
}

fn escape_code_length(c: char) -> Option<usize> {
    match c {
        'x' => Some(2),
        'u' => Some(4),
        'U' => Some(8),
        _ => None,
    }
}

/// `chr(code)`: a surrogate becomes the stand-in character, as JSON's do.
fn chr(code: u32) -> Result<char, Crash> {
    if (0xd800..0xe000).contains(&code) {
        return Ok(surrogate_char(code));
    }
    char::from_u32(code).ok_or_else(|| Crash::new("ValueError: chr() arg not in range(0x110000)"))
}
