//! `yaml.emitter`: events as text.

use crate::py::as_surrogate;

#[derive(Clone, Debug)]
pub(crate) enum Event {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Scalar {
        tag: Option<String>,
        implicit: (bool, bool),
        value: String,
        style: Option<char>,
    },
    SequenceStart {
        tag: Option<String>,
        implicit: bool,
        flow: bool,
    },
    SequenceEnd,
    MappingStart {
        tag: Option<String>,
        implicit: bool,
        flow: bool,
    },
    MappingEnd,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    StreamStart,
    FirstDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentRoot,
    Nothing,
    FirstFlowSequenceItem,
    FlowSequenceItem,
    FirstFlowMappingKey,
    FlowMappingKey,
    FlowMappingSimpleValue,
    FlowMappingValue,
    FirstBlockSequenceItem,
    BlockSequenceItem,
    FirstBlockMappingKey,
    BlockMappingKey,
    BlockMappingSimpleValue,
    BlockMappingValue,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Single,
    Double,
    Literal,
    Folded,
}

struct Analysis {
    scalar: Vec<char>,
    empty: bool,
    multiline: bool,
    allow_flow_plain: bool,
    allow_block_plain: bool,
    allow_single_quoted: bool,
    allow_block: bool,
}

const BREAKS: &str = "\n\u{85}\u{2028}\u{2029}";
const BLANK_OR_BREAK: &str = "\0 \t\r\n\u{85}\u{2028}\u{2029}";

/// The code point Python sees: a stand-in character is its lone surrogate.
fn code(c: char) -> u32 {
    as_surrogate(c).unwrap_or(c as u32)
}

pub(crate) struct Emitter {
    out: String,
    events: Vec<Event>,
    pos: usize,
    states: Vec<State>,
    state: State,
    indents: Vec<Option<usize>>,
    indent: Option<usize>,
    flow_level: usize,
    root_context: bool,
    sequence_context: bool,
    mapping_context: bool,
    simple_key_context: bool,
    column: usize,
    whitespace: bool,
    indention: bool,
    open_ended: bool,
    allow_unicode: bool,
    best_indent: usize,
    best_width: usize,
    prepared_tag: Option<String>,
    analysis: Option<Analysis>,
    style: Option<Style>,
}

impl Emitter {
    pub fn new(allow_unicode: bool, width: Option<usize>) -> Self {
        let best_indent = 2;
        Emitter {
            out: String::new(),
            events: Vec::new(),
            pos: 0,
            states: Vec::new(),
            state: State::StreamStart,
            indents: Vec::new(),
            indent: None,
            flow_level: 0,
            root_context: false,
            sequence_context: false,
            mapping_context: false,
            simple_key_context: false,
            column: 0,
            whitespace: true,
            indention: true,
            open_ended: false,
            allow_unicode,
            best_indent,
            best_width: width.filter(|&w| w > best_indent * 2).unwrap_or(80),
            prepared_tag: None,
            analysis: None,
            style: None,
        }
    }

    /// Writes all the events; the serializer produces them before the emitter runs, which
    /// only changes when the text is written, not what it is.
    pub fn emit_all(mut self, events: Vec<Event>) -> String {
        self.events = events;
        while self.pos < self.events.len() {
            self.run();
            self.pos += 1;
        }
        self.out
    }

    fn event(&self) -> &Event {
        &self.events[self.pos]
    }

    fn next_event(&self) -> Option<&Event> {
        self.events.get(self.pos + 1)
    }

    fn pop_state(&mut self) -> State {
        self.states.pop().unwrap_or(State::Nothing)
    }

    fn increase_indent(&mut self, flow: bool, indentless: bool) {
        self.indents.push(self.indent);
        match self.indent {
            None => self.indent = Some(if flow { self.best_indent } else { 0 }),
            Some(i) if !indentless => self.indent = Some(i + self.best_indent),
            _ => {}
        }
    }

    fn pop_indent(&mut self) {
        self.indent = self.indents.pop().unwrap_or(None);
    }

    fn run(&mut self) {
        match self.state {
            State::StreamStart => self.state = State::FirstDocumentStart,
            State::FirstDocumentStart => self.expect_document_start(true),
            State::DocumentStart => self.expect_document_start(false),
            State::DocumentEnd => {
                self.write_indent();
                self.state = State::DocumentStart;
            }
            State::DocumentRoot => {
                self.states.push(State::DocumentEnd);
                self.expect_node(true, false, false, false);
            }
            State::Nothing => {}
            State::FirstFlowSequenceItem | State::FlowSequenceItem => {
                let first = self.state == State::FirstFlowSequenceItem;
                if matches!(self.event(), Event::SequenceEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    self.write_indicator("]", false, false, false);
                    self.state = self.pop_state();
                } else {
                    if !first {
                        self.write_indicator(",", false, false, false);
                    }
                    if self.column > self.best_width {
                        self.write_indent();
                    }
                    self.states.push(State::FlowSequenceItem);
                    self.expect_node(false, true, false, false);
                }
            }
            State::FirstFlowMappingKey | State::FlowMappingKey => {
                let first = self.state == State::FirstFlowMappingKey;
                if matches!(self.event(), Event::MappingEnd) {
                    self.pop_indent();
                    self.flow_level -= 1;
                    self.write_indicator("}", false, false, false);
                    self.state = self.pop_state();
                } else {
                    if !first {
                        self.write_indicator(",", false, false, false);
                    }
                    if self.column > self.best_width {
                        self.write_indent();
                    }
                    if self.check_simple_key() {
                        self.states.push(State::FlowMappingSimpleValue);
                        self.expect_node(false, false, true, true);
                    } else {
                        self.write_indicator("?", true, false, false);
                        self.states.push(State::FlowMappingValue);
                        self.expect_node(false, false, true, false);
                    }
                }
            }
            State::FlowMappingSimpleValue => {
                self.write_indicator(":", false, false, false);
                self.states.push(State::FlowMappingKey);
                self.expect_node(false, false, true, false);
            }
            State::FlowMappingValue => {
                if self.column > self.best_width {
                    self.write_indent();
                }
                self.write_indicator(":", true, false, false);
                self.states.push(State::FlowMappingKey);
                self.expect_node(false, false, true, false);
            }
            State::FirstBlockSequenceItem | State::BlockSequenceItem => {
                let first = self.state == State::FirstBlockSequenceItem;
                if !first && matches!(self.event(), Event::SequenceEnd) {
                    self.pop_indent();
                    self.state = self.pop_state();
                } else {
                    self.write_indent();
                    self.write_indicator("-", true, false, true);
                    self.states.push(State::BlockSequenceItem);
                    self.expect_node(false, true, false, false);
                }
            }
            State::FirstBlockMappingKey | State::BlockMappingKey => {
                let first = self.state == State::FirstBlockMappingKey;
                if !first && matches!(self.event(), Event::MappingEnd) {
                    self.pop_indent();
                    self.state = self.pop_state();
                } else {
                    self.write_indent();
                    if self.check_simple_key() {
                        self.states.push(State::BlockMappingSimpleValue);
                        self.expect_node(false, false, true, true);
                    } else {
                        self.write_indicator("?", true, false, true);
                        self.states.push(State::BlockMappingValue);
                        self.expect_node(false, false, true, false);
                    }
                }
            }
            State::BlockMappingSimpleValue => {
                self.write_indicator(":", false, false, false);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, false, true, false);
            }
            State::BlockMappingValue => {
                self.write_indent();
                self.write_indicator(":", true, false, true);
                self.states.push(State::BlockMappingKey);
                self.expect_node(false, false, true, false);
            }
        }
    }

    fn expect_document_start(&mut self, first: bool) {
        match self.event() {
            Event::DocumentStart => {
                let implicit = first && !self.check_empty_document();
                if !implicit {
                    self.write_indent();
                    self.write_indicator("---", true, false, false);
                }
                self.state = State::DocumentRoot;
            }
            Event::StreamEnd => {
                if self.open_ended {
                    self.write_indicator("...", true, false, false);
                    self.write_indent();
                }
                self.state = State::Nothing;
            }
            _ => {}
        }
    }

    fn expect_node(&mut self, root: bool, sequence: bool, mapping: bool, simple_key: bool) {
        self.root_context = root;
        self.sequence_context = sequence;
        self.mapping_context = mapping;
        self.simple_key_context = simple_key;
        self.process_tag();
        match self.event().clone() {
            Event::Scalar { .. } => {
                self.increase_indent(true, false);
                self.process_scalar();
                self.pop_indent();
                self.state = self.pop_state();
            }
            Event::SequenceStart { flow, .. } => {
                if self.flow_level > 0 || flow || self.check_empty_sequence() {
                    self.write_indicator("[", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    self.state = State::FirstFlowSequenceItem;
                } else {
                    let indentless = self.mapping_context && !self.indention;
                    self.increase_indent(false, indentless);
                    self.state = State::FirstBlockSequenceItem;
                }
            }
            Event::MappingStart { flow, .. } => {
                if self.flow_level > 0 || flow || self.check_empty_mapping() {
                    self.write_indicator("{", true, true, false);
                    self.flow_level += 1;
                    self.increase_indent(true, false);
                    self.state = State::FirstFlowMappingKey;
                } else {
                    self.increase_indent(false, false);
                    self.state = State::FirstBlockMappingKey;
                }
            }
            _ => {}
        }
    }

    fn check_empty_sequence(&self) -> bool {
        matches!(self.event(), Event::SequenceStart { .. })
            && matches!(self.next_event(), Some(Event::SequenceEnd))
    }

    fn check_empty_mapping(&self) -> bool {
        matches!(self.event(), Event::MappingStart { .. })
            && matches!(self.next_event(), Some(Event::MappingEnd))
    }

    fn check_empty_document(&self) -> bool {
        matches!(
            self.next_event(),
            Some(Event::Scalar { tag: None, implicit, value, .. })
                if (implicit.0 || implicit.1) && value.is_empty()
        )
    }

    fn check_simple_key(&mut self) -> bool {
        let mut length = 0;
        let tag = match self.event() {
            Event::Scalar { tag, .. }
            | Event::SequenceStart { tag, .. }
            | Event::MappingStart { tag, .. } => tag.clone(),
            _ => None,
        };
        if let Some(tag) = tag {
            if self.prepared_tag.is_none() {
                self.prepared_tag = Some(self.prepare_tag(&tag));
            }
            length += self
                .prepared_tag
                .as_deref()
                .map_or(0, |t| t.chars().count());
        }
        if let Event::Scalar { value, .. } = self.event() {
            if self.analysis.is_none() {
                self.analysis = Some(self.analyze_scalar(value));
            }
            length += self.analysis.as_ref().map_or(0, |a| a.scalar.len());
        }
        length < 128
            && (match self.event() {
                Event::Scalar { .. } => self
                    .analysis
                    .as_ref()
                    .is_some_and(|a| !a.empty && !a.multiline),
                _ => false,
            } || self.check_empty_sequence()
                || self.check_empty_mapping())
    }

    fn process_tag(&mut self) {
        let mut tag = match self.event().clone() {
            Event::Scalar { tag, implicit, .. } => {
                if self.style.is_none() {
                    self.style = Some(self.choose_scalar_style());
                }
                let plain = self.style == Some(Style::Plain);
                if (plain && implicit.0) || (!plain && implicit.1) {
                    self.prepared_tag = None;
                    return;
                }
                if implicit.0 && tag.is_none() {
                    self.prepared_tag = None;
                    Some("!".to_string())
                } else {
                    tag
                }
            }
            Event::SequenceStart { tag, implicit, .. }
            | Event::MappingStart { tag, implicit, .. } => {
                if implicit {
                    self.prepared_tag = None;
                    return;
                }
                tag
            }
            _ => return,
        };
        let Some(tag) = tag.take() else {
            return;
        };
        if self.prepared_tag.is_none() {
            self.prepared_tag = Some(self.prepare_tag(&tag));
        }
        if let Some(t) = self.prepared_tag.take()
            && !t.is_empty()
        {
            self.write_indicator(&t, true, false, false);
        }
    }

    fn choose_scalar_style(&mut self) -> Style {
        let Event::Scalar {
            value,
            implicit,
            style,
            ..
        } = self.event().clone()
        else {
            return Style::Double;
        };
        if self.analysis.is_none() {
            self.analysis = Some(self.analyze_scalar(&value));
        }
        let a = self.analysis.as_ref().expect("analysis set above");
        if style == Some('"') {
            return Style::Double;
        }
        if style.is_none()
            && implicit.0
            && !(self.simple_key_context && (a.empty || a.multiline))
            && ((self.flow_level > 0 && a.allow_flow_plain)
                || (self.flow_level == 0 && a.allow_block_plain))
        {
            return Style::Plain;
        }
        if let Some(s @ ('|' | '>')) = style
            && self.flow_level == 0
            && !self.simple_key_context
            && a.allow_block
        {
            return if s == '|' {
                Style::Literal
            } else {
                Style::Folded
            };
        }
        if (style.is_none() || style == Some('\''))
            && a.allow_single_quoted
            && !(self.simple_key_context && a.multiline)
        {
            return Style::Single;
        }
        Style::Double
    }

    fn process_scalar(&mut self) {
        if self.analysis.is_none()
            && let Event::Scalar { value, .. } = self.event()
        {
            self.analysis = Some(self.analyze_scalar(value));
        }
        if self.style.is_none() {
            self.style = Some(self.choose_scalar_style());
        }
        let text = self.analysis.take().map(|a| a.scalar).unwrap_or_default();
        let split = !self.simple_key_context;
        match self.style.take().unwrap_or(Style::Double) {
            Style::Double => self.write_double_quoted(&text, split),
            Style::Single => self.write_single_quoted(&text, split),
            Style::Folded => self.write_folded(&text),
            Style::Literal => self.write_literal(&text),
            Style::Plain => self.write_plain(&text, split),
        }
    }

    fn prepare_tag(&self, tag: &str) -> String {
        if tag == "!" {
            return tag.into();
        }
        let mut handle = None;
        let mut suffix = tag;
        for (prefix, h) in [("!", "!"), ("tag:yaml.org,2002:", "!!")] {
            if tag.starts_with(prefix) && (prefix == "!" || prefix.len() < tag.len()) {
                handle = Some(h);
                suffix = &tag[prefix.len()..];
            }
        }
        let mut text = String::new();
        for ch in suffix.chars() {
            if ch.is_ascii_alphanumeric()
                || "-;/?:@&=+$,_.~*'()[]".contains(ch)
                || (ch == '!' && handle != Some("!"))
            {
                text.push(ch);
            } else {
                let mut buf = [0u8; 4];
                for b in ch.encode_utf8(&mut buf).bytes() {
                    text.push_str(&format!("%{b:02X}"));
                }
            }
        }
        match handle {
            Some(h) => format!("{h}{text}"),
            None => format!("!<{text}>"),
        }
    }

    fn analyze_scalar(&self, value: &str) -> Analysis {
        let scalar: Vec<char> = value.chars().collect();
        if scalar.is_empty() {
            return Analysis {
                scalar,
                empty: true,
                multiline: false,
                allow_flow_plain: false,
                allow_block_plain: true,
                allow_single_quoted: true,
                allow_block: false,
            };
        }
        let mut block_indicators = false;
        let mut flow_indicators = false;
        let mut line_breaks = false;
        let mut special_characters = false;
        let mut leading_space = false;
        let mut leading_break = false;
        let mut trailing_space = false;
        let mut trailing_break = false;
        let mut break_space = false;
        let mut space_break = false;
        if value.starts_with("---") || value.starts_with("...") {
            block_indicators = true;
            flow_indicators = true;
        }
        let n = scalar.len();
        let mut preceded_by_whitespace = true;
        let mut followed_by_whitespace = n == 1 || BLANK_OR_BREAK.contains(scalar[1]);
        let mut previous_space = false;
        let mut previous_break = false;
        for (index, &ch) in scalar.iter().enumerate() {
            if index == 0 {
                if "#,[]{}&*!|>'\"%@`".contains(ch) {
                    flow_indicators = true;
                    block_indicators = true;
                }
                if ch == '?' || ch == ':' {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                if ch == '-' && followed_by_whitespace {
                    flow_indicators = true;
                    block_indicators = true;
                }
            } else {
                if ",?[]{}".contains(ch) {
                    flow_indicators = true;
                }
                if ch == ':' {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                if ch == '#' && preceded_by_whitespace {
                    flow_indicators = true;
                    block_indicators = true;
                }
            }
            if BREAKS.contains(ch) {
                line_breaks = true;
            }
            let u = code(ch);
            if !(ch == '\n' || (0x20..=0x7e).contains(&u)) {
                if (u == 0x85
                    || (0xa0..=0xd7ff).contains(&u)
                    || (0xe000..=0xfffd).contains(&u)
                    || (0x10000..0x10ffff).contains(&u))
                    && u != 0xfeff
                {
                    if !self.allow_unicode {
                        special_characters = true;
                    }
                } else {
                    special_characters = true;
                }
            }
            if ch == ' ' {
                if index == 0 {
                    leading_space = true;
                }
                if index == n - 1 {
                    trailing_space = true;
                }
                if previous_break {
                    break_space = true;
                }
                previous_space = true;
                previous_break = false;
            } else if BREAKS.contains(ch) {
                if index == 0 {
                    leading_break = true;
                }
                if index == n - 1 {
                    trailing_break = true;
                }
                if previous_space {
                    space_break = true;
                }
                previous_space = false;
                previous_break = true;
            } else {
                previous_space = false;
                previous_break = false;
            }
            preceded_by_whitespace = BLANK_OR_BREAK.contains(ch);
            followed_by_whitespace = index + 2 >= n || BLANK_OR_BREAK.contains(scalar[index + 2]);
        }
        let mut allow_flow_plain = true;
        let mut allow_block_plain = true;
        let mut allow_single_quoted = true;
        let mut allow_block = true;
        if leading_space || leading_break || trailing_space || trailing_break {
            allow_flow_plain = false;
            allow_block_plain = false;
        }
        if trailing_space {
            allow_block = false;
        }
        if break_space {
            allow_flow_plain = false;
            allow_block_plain = false;
            allow_single_quoted = false;
        }
        if space_break || special_characters {
            allow_flow_plain = false;
            allow_block_plain = false;
            allow_single_quoted = false;
            allow_block = false;
        }
        if line_breaks {
            allow_flow_plain = false;
            allow_block_plain = false;
        }
        if flow_indicators {
            allow_flow_plain = false;
        }
        if block_indicators {
            allow_block_plain = false;
        }
        Analysis {
            scalar,
            empty: false,
            multiline: line_breaks,
            allow_flow_plain,
            allow_block_plain,
            allow_single_quoted,
            allow_block,
        }
    }

    // ------------------------------------------------------------- writers

    fn write(&mut self, data: &str) {
        self.out.push_str(data);
    }

    fn write_chars(&mut self, data: &[char]) {
        self.column += data.len();
        self.out.extend(data.iter());
    }

    fn write_indicator(
        &mut self,
        indicator: &str,
        need_whitespace: bool,
        whitespace: bool,
        indention: bool,
    ) {
        let data = if self.whitespace || !need_whitespace {
            indicator.to_string()
        } else {
            format!(" {indicator}")
        };
        self.whitespace = whitespace;
        self.indention = self.indention && indention;
        self.column += data.chars().count();
        self.open_ended = false;
        self.write(&data);
    }

    fn write_indent(&mut self) {
        let indent = self.indent.unwrap_or(0);
        if !self.indention || self.column > indent || (self.column == indent && !self.whitespace) {
            self.write_line_break(None);
        }
        if self.column < indent {
            self.whitespace = true;
            let data = " ".repeat(indent - self.column);
            self.column = indent;
            self.write(&data);
        }
    }

    fn write_line_break(&mut self, data: Option<char>) {
        self.whitespace = true;
        self.indention = true;
        self.column = 0;
        let mut buf = [0u8; 4];
        let text = data.unwrap_or('\n').encode_utf8(&mut buf).to_string();
        self.write(&text);
    }

    fn write_breaks(&mut self, breaks: &[char]) {
        for &br in breaks {
            self.write_line_break(if br == '\n' { None } else { Some(br) });
        }
    }

    fn write_single_quoted(&mut self, text: &[char], split: bool) {
        self.write_indicator("'", true, false, false);
        let mut spaces = false;
        let mut breaks = false;
        let (mut start, mut end) = (0usize, 0usize);
        let n = text.len();
        while end <= n {
            let ch = text.get(end).copied();
            if spaces {
                if ch != Some(' ') {
                    if start + 1 == end
                        && self.column > self.best_width
                        && split
                        && start != 0
                        && end != n
                    {
                        self.write_indent();
                    } else {
                        self.write_chars(&text[start..end]);
                    }
                    start = end;
                }
            } else if breaks {
                if ch.is_none_or(|c| !BREAKS.contains(c)) {
                    if text[start] == '\n' {
                        self.write_line_break(None);
                    }
                    self.write_breaks(&text[start..end]);
                    self.write_indent();
                    start = end;
                }
            } else if ch.is_none_or(|c| " \n\u{85}\u{2028}\u{2029}'".contains(c)) && start < end {
                self.write_chars(&text[start..end]);
                start = end;
            }
            if ch == Some('\'') {
                self.column += 2;
                self.write("''");
                start = end + 1;
            }
            if let Some(c) = ch {
                spaces = c == ' ';
                breaks = BREAKS.contains(c);
            }
            end += 1;
        }
        self.write_indicator("'", false, false, false);
    }

    fn write_double_quoted(&mut self, text: &[char], split: bool) {
        self.write_indicator("\"", true, false, false);
        let (mut start, mut end) = (0usize, 0usize);
        let n = text.len();
        while end <= n {
            let ch = text.get(end).copied();
            let escape = match ch {
                None => true,
                Some(c) => {
                    let u = code(c);
                    "\"\\\u{85}\u{2028}\u{2029}\u{feff}".contains(c)
                        || !((0x20..=0x7e).contains(&u)
                            || (self.allow_unicode
                                && ((0xa0..=0xd7ff).contains(&u)
                                    || (0xe000..=0xfffd).contains(&u))))
                }
            };
            if escape {
                if start < end {
                    self.write_chars(&text[start..end]);
                    start = end;
                }
                if let Some(c) = ch {
                    let u = code(c);
                    let data = match escape_replacement(c) {
                        Some(r) => format!("\\{r}"),
                        None if u <= 0xff => format!("\\x{u:02X}"),
                        None if u <= 0xffff => format!("\\u{u:04X}"),
                        None => format!("\\U{u:08X}"),
                    };
                    self.column += data.chars().count();
                    self.write(&data);
                    start = end + 1;
                }
            }
            if 0 < end
                && end + 1 < n
                && (ch == Some(' ') || start >= end)
                && self.column as isize + end as isize - start as isize > self.best_width as isize
                && split
            {
                let mut data: String = text[start.min(end)..end].iter().collect();
                data.push('\\');
                if start < end {
                    start = end;
                }
                self.column += data.chars().count();
                self.write(&data);
                self.write_indent();
                self.whitespace = false;
                self.indention = false;
                if text[start] == ' ' {
                    self.column += 1;
                    self.write("\\");
                }
            }
            end += 1;
        }
        self.write_indicator("\"", false, false, false);
    }

    fn block_hints(&self, text: &[char]) -> String {
        let mut hints = String::new();
        if let (Some(&first), Some(&last)) = (text.first(), text.last()) {
            if " \n\u{85}\u{2028}\u{2029}".contains(first) {
                hints.push_str(&self.best_indent.to_string());
            }
            if !BREAKS.contains(last) {
                hints.push('-');
            } else if text.len() == 1 || BREAKS.contains(text[text.len() - 2]) {
                hints.push('+');
            }
        }
        hints
    }

    fn write_folded(&mut self, text: &[char]) {
        let hints = self.block_hints(text);
        self.write_indicator(&format!(">{hints}"), true, false, false);
        if hints.ends_with('+') {
            self.open_ended = true;
        }
        self.write_line_break(None);
        let mut leading_space = true;
        let mut spaces = false;
        let mut breaks = true;
        let (mut start, mut end) = (0usize, 0usize);
        let n = text.len();
        while end <= n {
            let ch = text.get(end).copied();
            if breaks {
                if ch.is_none_or(|c| !BREAKS.contains(c)) {
                    if !leading_space && ch.is_some_and(|c| c != ' ') && text[start] == '\n' {
                        self.write_line_break(None);
                    }
                    leading_space = ch == Some(' ');
                    self.write_breaks(&text[start..end]);
                    if ch.is_some() {
                        self.write_indent();
                    }
                    start = end;
                }
            } else if spaces {
                if ch != Some(' ') {
                    if start + 1 == end && self.column > self.best_width {
                        self.write_indent();
                    } else {
                        self.write_chars(&text[start..end]);
                    }
                    start = end;
                }
            } else if ch.is_none_or(|c| " \n\u{85}\u{2028}\u{2029}".contains(c)) {
                self.write_chars(&text[start..end]);
                if ch.is_none() {
                    self.write_line_break(None);
                }
                start = end;
            }
            if let Some(c) = ch {
                breaks = BREAKS.contains(c);
                spaces = c == ' ';
            }
            end += 1;
        }
    }

    fn write_literal(&mut self, text: &[char]) {
        let hints = self.block_hints(text);
        self.write_indicator(&format!("|{hints}"), true, false, false);
        if hints.ends_with('+') {
            self.open_ended = true;
        }
        self.write_line_break(None);
        let mut breaks = true;
        let (mut start, mut end) = (0usize, 0usize);
        let n = text.len();
        while end <= n {
            let ch = text.get(end).copied();
            if breaks {
                if ch.is_none_or(|c| !BREAKS.contains(c)) {
                    self.write_breaks(&text[start..end]);
                    if ch.is_some() {
                        self.write_indent();
                    }
                    start = end;
                }
            } else if ch.is_none_or(|c| BREAKS.contains(c)) {
                // Python writes these characters without counting them in the column.
                let data: String = text[start..end].iter().collect();
                self.write(&data);
                if ch.is_none() {
                    self.write_line_break(None);
                }
                start = end;
            }
            if let Some(c) = ch {
                breaks = BREAKS.contains(c);
            }
            end += 1;
        }
    }

    fn write_plain(&mut self, text: &[char], split: bool) {
        if self.root_context {
            self.open_ended = true;
        }
        if text.is_empty() {
            return;
        }
        if !self.whitespace {
            self.column += 1;
            self.write(" ");
        }
        self.whitespace = false;
        self.indention = false;
        let mut spaces = false;
        let mut breaks = false;
        let (mut start, mut end) = (0usize, 0usize);
        let n = text.len();
        while end <= n {
            let ch = text.get(end).copied();
            if spaces {
                if ch != Some(' ') {
                    if start + 1 == end && self.column > self.best_width && split {
                        self.write_indent();
                        self.whitespace = false;
                        self.indention = false;
                    } else {
                        self.write_chars(&text[start..end]);
                    }
                    start = end;
                }
            } else if breaks {
                if ch.is_some_and(|c| !BREAKS.contains(c)) {
                    if text[start] == '\n' {
                        self.write_line_break(None);
                    }
                    self.write_breaks(&text[start..end]);
                    self.write_indent();
                    self.whitespace = false;
                    self.indention = false;
                    start = end;
                }
            } else if ch.is_none_or(|c| " \n\u{85}\u{2028}\u{2029}".contains(c)) {
                self.write_chars(&text[start..end]);
                start = end;
            }
            if let Some(c) = ch {
                spaces = c == ' ';
                breaks = BREAKS.contains(c);
            }
            end += 1;
        }
    }
}

fn escape_replacement(c: char) -> Option<char> {
    Some(match c {
        '\0' => '0',
        '\x07' => 'a',
        '\x08' => 'b',
        '\t' => 't',
        '\n' => 'n',
        '\x0b' => 'v',
        '\x0c' => 'f',
        '\r' => 'r',
        '\x1b' => 'e',
        '"' => '"',
        '\\' => '\\',
        '\u{85}' => 'N',
        '\u{a0}' => '_',
        '\u{2028}' => 'L',
        '\u{2029}' => 'P',
        _ => return None,
    })
}
