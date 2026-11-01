//! The part of Python's `re` the scripts' patterns use, matched the way `sre` matches them: a
//! backtracking search over characters, with Python's `\w`, `\s`, `\d` and `\b`, lookahead and
//! fixed-width lookbehind, lazy and greedy repeats, and `IGNORECASE` as `sre` applies it (simple
//! lower-case mappings plus its table of equivalent letters). The `regex` crate has neither
//! lookaround nor these character definitions.

use super::num::{decimal_value, is_space};
use super::unicode::{is_word, lower_char, upper_char};

#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    pub ignorecase: bool,
    pub dotall: bool,
    pub multiline: bool,
}

#[derive(Debug, Clone)]
enum Item {
    Char(char),
    Range(char, char),
    Word(bool),
    Space(bool),
    Digit(bool),
}

#[derive(Debug, Clone)]
struct Class {
    negate: bool,
    items: Vec<Item>,
    /// `IGNORECASE` lowers the character first only for sets that hold a cased letter.
    cased: bool,
}

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class(Class),
    Start,
    End,
    /// `\A` and `\Z`: the very start and end of the text.
    TextStart,
    TextEnd,
    Boundary(bool),
    Group(Box<Node>, Option<usize>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
    Look {
        node: Box<Node>,
        behind: Option<usize>,
        negate: bool,
    },
}

/// A compiled pattern.
#[derive(Debug, Clone)]
pub struct Regex {
    node: Node,
    groups: usize,
    flags: Flags,
}

/// Where a match and its groups are, in characters.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub groups: Vec<Option<(usize, usize)>>,
}

impl Match {
    /// `m.group(i)` as text (`None` for a group that did not take part).
    pub fn group(&self, text: &[char], i: usize) -> Option<String> {
        let (a, b) = if i == 0 {
            (self.start, self.end)
        } else {
            self.groups.get(i - 1).copied().flatten()?
        };
        Some(text[a..b].iter().collect())
    }
}

/// `sre`'s table of letters that match each other under `IGNORECASE` beyond lower-casing.
const EQUIVALENT: [&[u32]; 16] = [
    &[0x69, 0x131],
    &[0x73, 0x17f],
    &[0xb5, 0x3bc],
    &[0x345, 0x3b9, 0x1fbe],
    &[0x390, 0x1fd3],
    &[0x3b0, 0x1fe3],
    &[0x3b2, 0x3d0],
    &[0x3b5, 0x3f5],
    &[0x3b8, 0x3d1],
    &[0x3ba, 0x3f0],
    &[0x3c0, 0x3d6],
    &[0x3c1, 0x3f1],
    &[0x3c2, 0x3c3],
    &[0x3c6, 0x3d5],
    &[0x1e61, 0x1e9b],
    &[0xfb05, 0xfb06],
];

fn is_cased(c: char) -> bool {
    lower_char(c) != c || upper_char(c) != c
}

/// Whether the lowered text character `lc` matches the pattern character `p`.
fn same_letter(lc: char, p: char) -> bool {
    let lp = lower_char(p);
    lc == lp
        || EQUIVALENT
            .iter()
            .any(|set| set.contains(&(lp as u32)) && set.contains(&(lc as u32)))
}

impl Class {
    fn raw(&self, c: char) -> bool {
        let hit = self.items.iter().any(|item| match *item {
            Item::Char(x) => x == c,
            Item::Range(a, b) => a <= c && c <= b,
            Item::Word(yes) => is_word(c) == yes,
            Item::Space(yes) => is_space(c) == yes,
            Item::Digit(yes) => decimal_value(c).is_some() == yes,
        });
        hit != self.negate
    }

    fn matches(&self, c: char, flags: Flags) -> bool {
        if !(flags.ignorecase && self.cased) {
            return self.raw(c);
        }
        let lc = lower_char(c);
        let hit = self.items.iter().any(|item| match *item {
            Item::Char(x) => same_letter(lc, x),
            Item::Range(a, b) => {
                let inside = |x: char| a <= x && x <= b;
                inside(lc)
                    || inside(upper_char(lc))
                    || EQUIVALENT.iter().any(|set| {
                        set.contains(&(lc as u32))
                            && set
                                .iter()
                                .filter_map(|&u| char::from_u32(u))
                                .any(|x| inside(x) || inside(upper_char(x)))
                    })
            }
            Item::Word(yes) => is_word(lc) == yes,
            Item::Space(yes) => is_space(lc) == yes,
            Item::Digit(yes) => decimal_value(lc).is_some() == yes,
        });
        hit != self.negate
    }
}

// ---------------------------------------------------------------- parser

struct Parser<'a> {
    p: &'a [char],
    i: usize,
    groups: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.p.get(self.i).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.concat()?];
        while self.eat('|') {
            alts.push(self.concat()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap_or(Node::Concat(Vec::new()))
        } else {
            Node::Alt(alts)
        })
    }

    fn concat(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quantified(atom)?);
        }
        Ok(Node::Concat(items))
    }

    fn number(&mut self) -> Option<usize> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        (self.i > start).then(|| {
            self.p[start..self.i]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(usize::MAX)
        })
    }

    fn quantified(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let lo = self.number();
                let bounds = if self.eat(',') {
                    let hi = self.number();
                    Some((lo.unwrap_or(0), hi))
                } else {
                    lo.map(|n| (n, Some(n)))
                };
                match bounds {
                    Some(b) if self.peek() == Some('}') => {
                        self.i += 1;
                        let greedy = !self.eat('?');
                        return Ok(Node::Repeat {
                            node: Box::new(atom),
                            min: b.0,
                            max: b.1,
                            greedy,
                        });
                    }
                    _ => {
                        // Not a repeat: `{` is a literal.
                        self.i = save;
                        return Ok(atom);
                    }
                }
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        let greedy = !self.eat('?');
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    fn escape(&mut self, in_class: bool) -> Result<Item, String> {
        let c = self.peek().ok_or("bad escape (end of pattern)")?;
        self.i += 1;
        Ok(match c {
            'w' => Item::Word(true),
            'W' => Item::Word(false),
            's' => Item::Space(true),
            'S' => Item::Space(false),
            'd' => Item::Digit(true),
            'D' => Item::Digit(false),
            'n' => Item::Char('\n'),
            't' => Item::Char('\t'),
            'r' => Item::Char('\r'),
            'f' => Item::Char('\u{c}'),
            'v' => Item::Char('\u{b}'),
            'a' => Item::Char('\u{7}'),
            'b' if in_class => Item::Char('\u{8}'),
            c if c.is_ascii_alphanumeric() => return Err(format!("bad escape \\{c}")),
            c => Item::Char(c),
        })
    }

    fn class(&mut self) -> Result<Node, String> {
        let negate = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or("unterminated character set")?;
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            self.i += 1;
            let item = if c == '\\' {
                self.escape(true)?
            } else {
                Item::Char(c)
            };
            if let Item::Char(lo) = item
                && self.peek() == Some('-')
                && self.p.get(self.i + 1).is_some_and(|&n| n != ']')
            {
                self.i += 1;
                let n = self.peek().ok_or("unterminated character set")?;
                self.i += 1;
                let hi = if n == '\\' {
                    match self.escape(true)? {
                        Item::Char(h) => h,
                        _ => return Err("bad character range".into()),
                    }
                } else {
                    n
                };
                items.push(Item::Range(lo, hi));
                continue;
            }
            items.push(item);
        }
        let cased = items.iter().any(|item| match *item {
            Item::Char(c) => is_cased(c),
            Item::Range(a, b) => (a..=b).any(is_cased),
            _ => false,
        });
        Ok(Node::Class(Class {
            negate,
            items,
            cased,
        }))
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end of pattern")?;
        self.i += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '[' => self.class()?,
            '(' => {
                let node = if self.eat('?') {
                    if self.eat(':') {
                        Node::Group(Box::new(self.alt()?), None)
                    } else if self.eat('=') || self.peek() == Some('!') {
                        let negate = self.eat('!');
                        Node::Look {
                            node: Box::new(self.alt()?),
                            behind: None,
                            negate,
                        }
                    } else if self.eat('<') {
                        let negate = if self.eat('!') {
                            true
                        } else if self.eat('=') {
                            false
                        } else {
                            return Err("unknown extension ?<".into());
                        };
                        let inner = self.alt()?;
                        let width = fixed_width(&inner)
                            .ok_or("look-behind requires fixed-width pattern")?;
                        Node::Look {
                            node: Box::new(inner),
                            behind: Some(width),
                            negate,
                        }
                    } else {
                        return Err("unknown extension".into());
                    }
                } else {
                    self.groups += 1;
                    let index = self.groups - 1;
                    Node::Group(Box::new(self.alt()?), Some(index))
                };
                if !self.eat(')') {
                    return Err("missing ), unterminated subpattern".into());
                }
                node
            }
            '\\' => match self.peek() {
                Some('b') => {
                    self.i += 1;
                    Node::Boundary(true)
                }
                Some('B') => {
                    self.i += 1;
                    Node::Boundary(false)
                }
                Some('A') => {
                    self.i += 1;
                    Node::TextStart
                }
                Some('Z') => {
                    self.i += 1;
                    Node::TextEnd
                }
                _ => match self.escape(false)? {
                    Item::Char(x) => Node::Char(x),
                    item => Node::Class(Class {
                        negate: false,
                        items: vec![item],
                        cased: false,
                    }),
                },
            },
            '*' | '+' | '?' => return Err("nothing to repeat".into()),
            c => Node::Char(c),
        })
    }
}

fn fixed_width(node: &Node) -> Option<usize> {
    match node {
        Node::Char(_) | Node::Any | Node::Class(_) => Some(1),
        Node::Start
        | Node::End
        | Node::TextStart
        | Node::TextEnd
        | Node::Boundary(_)
        | Node::Look { .. } => Some(0),
        Node::Group(n, _) => fixed_width(n),
        Node::Concat(items) => items.iter().map(fixed_width).sum(),
        Node::Alt(alts) => {
            let w = fixed_width(alts.first()?)?;
            alts.iter().all(|a| fixed_width(a) == Some(w)).then_some(w)
        }
        Node::Repeat { node, min, max, .. } => {
            (Some(*min) == *max).then(|| fixed_width(node).map(|w| w * min))?
        }
    }
}

// ---------------------------------------------------------------- matcher

type Caps = Vec<Option<(usize, usize)>>;

struct Matcher<'a> {
    t: &'a [char],
    flags: Flags,
}

impl Matcher<'_> {
    /// One character at `pos` matched by a single-character node.
    fn one(&self, node: &Node, pos: usize) -> bool {
        let Some(&c) = self.t.get(pos) else {
            return false;
        };
        match node {
            Node::Char(p) => {
                if self.flags.ignorecase && is_cased(*p) {
                    same_letter(lower_char(c), *p)
                } else {
                    c == *p
                }
            }
            Node::Any => self.flags.dotall || c != '\n',
            Node::Class(class) => class.matches(c, self.flags),
            _ => false,
        }
    }

    fn m(
        &self,
        node: &Node,
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        let len = self.t.len();
        match node {
            Node::Char(_) | Node::Any | Node::Class(_) => self.one(node, pos) && k(pos + 1, caps),
            Node::Start => {
                (pos == 0 || (self.flags.multiline && self.t[pos - 1] == '\n')) && k(pos, caps)
            }
            Node::End => {
                let at_end =
                    pos == len || (self.t[pos] == '\n' && (self.flags.multiline || pos + 1 == len));
                at_end && k(pos, caps)
            }
            Node::TextStart => pos == 0 && k(pos, caps),
            Node::TextEnd => pos == len && k(pos, caps),
            Node::Boundary(want) => {
                let before = pos > 0 && is_word(self.t[pos - 1]);
                let after = pos < len && is_word(self.t[pos]);
                ((before != after) == *want) && k(pos, caps)
            }
            Node::Group(inner, index) => match index {
                None => self.m(inner, pos, caps, k),
                Some(i) => {
                    let i = *i;
                    self.m(inner, pos, caps, &mut |p, c: &mut Caps| {
                        let old = c[i];
                        c[i] = Some((pos, p));
                        if k(p, c) {
                            return true;
                        }
                        c[i] = old;
                        false
                    })
                }
            },
            Node::Concat(items) => self.seq(items, pos, caps, k),
            Node::Alt(alts) => {
                for alt in alts {
                    let saved = caps.clone();
                    if self.m(alt, pos, caps, k) {
                        return true;
                    }
                    *caps = saved;
                }
                false
            }
            Node::Repeat {
                node: inner,
                min,
                max,
                greedy,
            } => {
                if matches!(**inner, Node::Char(_) | Node::Any | Node::Class(_)) {
                    let limit = max.unwrap_or(usize::MAX);
                    let mut n = 0;
                    while n < limit && self.one(inner, pos + n) {
                        n += 1;
                    }
                    if n < *min {
                        return false;
                    }
                    if *greedy {
                        (*min..=n).rev().any(|j| k(pos + j, caps))
                    } else {
                        (*min..=n).any(|j| k(pos + j, caps))
                    }
                } else {
                    self.rep(inner, *min, *max, *greedy, 0, pos, caps, k)
                }
            }
            Node::Look {
                node: inner,
                behind,
                negate,
            } => {
                let mut trial = caps.clone();
                let found = match behind {
                    None => self.m(inner, pos, &mut trial, &mut |_, _| true),
                    Some(w) => {
                        pos >= *w && self.m(inner, pos - w, &mut trial, &mut |p, _| p == pos)
                    }
                };
                if found == *negate {
                    return false;
                }
                if !*negate {
                    *caps = trial;
                }
                k(pos, caps)
            }
        }
    }

    fn seq(
        &self,
        items: &[Node],
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        match items.split_first() {
            None => k(pos, caps),
            Some((first, rest)) => self.m(first, pos, caps, &mut |p, c: &mut Caps| {
                self.seq(rest, p, c, k)
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rep(
        &self,
        inner: &Node,
        min: usize,
        max: Option<usize>,
        greedy: bool,
        count: usize,
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        let more = max.is_none_or(|m| count < m);
        if greedy {
            if more {
                let saved = caps.clone();
                if self.iterate(inner, min, max, greedy, count, pos, caps, k) {
                    return true;
                }
                *caps = saved;
            }
            count >= min && k(pos, caps)
        } else {
            if count >= min {
                let saved = caps.clone();
                if k(pos, caps) {
                    return true;
                }
                *caps = saved;
            }
            more && self.iterate(inner, min, max, greedy, count, pos, caps, k)
        }
    }

    /// One more iteration of a repeat, then the rest of it.
    #[allow(clippy::too_many_arguments)]
    fn iterate(
        &self,
        inner: &Node,
        min: usize,
        max: Option<usize>,
        greedy: bool,
        count: usize,
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        self.m(inner, pos, caps, &mut |p, c: &mut Caps| {
            if p == pos {
                // An empty iteration ends the repeat, as `sre` does.
                return count + 1 >= min && k(p, c);
            }
            self.rep(inner, min, max, greedy, count + 1, p, c, k)
        })
    }
}

impl Regex {
    /// `re.compile(pattern, flags)`; `flags` holds the letters of `re.I`, `re.S` and `re.M`.
    pub fn new(pattern: &str, flags: &str) -> Result<Regex, String> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut parser = Parser {
            p: &chars,
            i: 0,
            groups: 0,
        };
        let node = parser.alt()?;
        if parser.i != chars.len() {
            return Err("unbalanced parenthesis".into());
        }
        Ok(Regex {
            node,
            groups: parser.groups,
            flags: Flags {
                ignorecase: flags.contains('i'),
                dotall: flags.contains('s'),
                multiline: flags.contains('m'),
            },
        })
    }

    /// A pattern written in the code: it is known to compile.
    pub fn fixed(pattern: &str, flags: &str) -> Regex {
        Regex::new(pattern, flags).unwrap_or_else(|e| panic!("pattern {pattern:?}: {e}"))
    }

    /// The match starting exactly at `pos`; with `nonempty`, an empty one is refused.
    fn at(&self, t: &[char], pos: usize, nonempty: bool) -> Option<Match> {
        let matcher = Matcher {
            t,
            flags: self.flags,
        };
        let mut caps: Caps = vec![None; self.groups];
        let mut end = None;
        let mut groups = None;
        let found = matcher.m(&self.node, pos, &mut caps, &mut |p, c: &mut Caps| {
            if nonempty && p == pos {
                return false;
            }
            end = Some(p);
            groups = Some(c.clone());
            true
        });
        found.then(|| Match {
            start: pos,
            end: end.unwrap_or(pos),
            groups: groups.unwrap_or_default(),
        })
    }

    /// `pattern.match(text)`.
    pub fn match_start(&self, t: &[char]) -> Option<Match> {
        self.at(t, 0, false)
    }

    fn search_from(&self, t: &[char], start: usize, must_advance: bool) -> Option<Match> {
        (start..=t.len()).find_map(|pos| self.at(t, pos, must_advance && pos == start))
    }

    /// `pattern.search(text)`.
    pub fn search(&self, t: &[char]) -> Option<Match> {
        self.search_from(t, 0, false)
    }

    /// `pattern.finditer(text)`.
    pub fn find_iter(&self, t: &[char]) -> Vec<Match> {
        let mut out = Vec::new();
        let mut pos = 0;
        let mut must_advance = false;
        while pos <= t.len() {
            let Some(m) = self.search_from(t, pos, must_advance) else {
                break;
            };
            must_advance = m.end == m.start;
            pos = m.end;
            out.push(m);
        }
        out
    }

    /// `pattern.sub(repl, text)` with `repl` a function of the match.
    pub fn sub(&self, text: &str, mut repl: impl FnMut(&Match, &[char]) -> String) -> String {
        let t: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut last = 0;
        for m in self.find_iter(&t) {
            out.extend(&t[last..m.start]);
            out.push_str(&repl(&m, &t));
            last = m.end;
        }
        out.extend(&t[last..]);
        out
    }

    /// `pattern.sub(literal, text)`.
    pub fn replace(&self, text: &str, with: &str) -> String {
        self.sub(text, |_, _| with.to_string())
    }

    /// `bool(pattern.search(text))`.
    pub fn is_found(&self, text: &str) -> bool {
        let t: Vec<char> = text.chars().collect();
        self.search(&t).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(p: &str, flags: &str, text: &str) -> Vec<String> {
        let t: Vec<char> = text.chars().collect();
        Regex::fixed(p, flags)
            .find_iter(&t)
            .iter()
            .map(|m| m.group(&t, 0).unwrap_or_default())
            .collect()
    }

    #[test]
    fn matches_like_sre() {
        assert_eq!(
            all(r"(?<![^\W_])_+|_+(?![^\W_])", "", "a__b _x_ snake_case"),
            ["_", "_", "_", "_"]
        );
        assert_eq!(
            all(r"claude(?:\s+code)?\b", "i", "Claude codex, CLAUDE  Code."),
            ["Claude", "CLAUDE  Code"]
        );
        assert_eq!(
            all(
                r"claude\.ai/code(?!/session)\b",
                "",
                "claude.ai/code/session claude.ai/code/x"
            ),
            ["claude.ai/code"]
        );
        assert_eq!(
            all(
                r"co-authored-by:[^\n]*(?:bot\b|gpt)",
                "i",
                "Co-Authored-By: gpt and a bot\nx"
            ),
            ["Co-Authored-By: gpt and a bot"]
        );
        assert_eq!(
            all(r"```.*?```", "s", "a```x\n```b```y```"),
            ["```x\n```", "```y```"]
        );
        assert_eq!(
            all(
                r"(?<![\w/])(?:\.?[\w.-]+/)+[\w.-]+|(?<!\w)\.[\w.-]+\b",
                "",
                "see a/b/ and .env- x/.y"
            ),
            ["a/b", ".env", "x/.y"]
        );
        assert_eq!(all(r"s", "i", "ſS"), ["ſ", "S"]);
        assert_eq!(all(r"i", "i", "İıI"), ["İ", "ı", "I"]);
        assert_eq!(all(r"x*", "", "axx"), ["", "xx", ""]);
        let t: Vec<char> = "git@host:org/repo.git".chars().collect();
        let m = Regex::fixed(r"^(?:ssh://)?git@([^:/]+)[:/](.+?)(?:\.git)?/?$", "")
            .match_start(&t)
            .unwrap();
        assert_eq!(
            (m.group(&t, 1).unwrap(), m.group(&t, 2).unwrap()),
            ("host".into(), "org/repo".into())
        );
        assert_eq!(
            Regex::fixed(r"\[([^\]\n]*)\]\([^)\n]*\)", "")
                .sub("[a](b) [c](d)", |m, t| m.group(t, 1).unwrap()),
            "a c"
        );
    }
}
