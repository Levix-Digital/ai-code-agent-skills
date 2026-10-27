//! Python's `textwrap.wrap` and `textwrap.fill` with their default options (break on hyphens,
//! break long words, drop whitespace at line ends), as argparse uses them for help text.

const WS: [char; 6] = ['\t', '\n', '\u{b}', '\u{c}', '\r', ' '];

fn is_ws(c: char) -> bool {
    WS.contains(&c)
}

/// `[^\d\W]`: a word character that is not a digit.
fn is_letter(c: char) -> bool {
    super::is_word(c) && !is_decimal(c)
}

fn is_decimal(c: char) -> bool {
    // `\d`: Unicode decimal digits; `to_digit` covers ASCII, `is_numeric` the rest closely enough.
    c.is_ascii_digit() || (!c.is_ascii() && c.is_numeric() && !c.is_alphabetic())
}

/// `[\w!"'&.,?]`.
fn is_word_punct(c: char) -> bool {
    super::is_word(c) || matches!(c, '!' | '"' | '\'' | '&' | '.' | ',' | '?')
}

/// `TextWrapper.wordsep_re`: whitespace runs, em-dashes between words, and words split
/// after their hyphens.
fn chunks(text: &[char]) -> Vec<String> {
    let n = text.len();
    let at = |i: isize| -> Option<char> {
        if i < 0 || i as usize >= n {
            None
        } else {
            Some(text[i as usize])
        }
    };
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < n {
        if is_ws(text[i]) {
            let start = i;
            while i < n && is_ws(text[i]) {
                i += 1;
            }
            out.push(text[start..i].iter().collect());
            continue;
        }
        // An em-dash between words: (?<=wp) -{2,} (?=\w)
        if text[i] == '-' && at(i as isize - 1).is_some_and(is_word_punct) {
            let mut j = i;
            while j < n && text[j] == '-' {
                j += 1;
            }
            if j - i >= 2 && at(j as isize).is_some_and(super::is_word) {
                out.push(text[i..j].iter().collect());
                i = j;
                continue;
            }
        }
        // A word, possibly hyphenated: the shortest run of non-whitespace that ends the word.
        let start = i;
        let mut j = i + 1;
        let end = loop {
            if j > n || is_ws(text[j - 1]) {
                break j - 1;
            }
            let ji = j as isize;
            // hyphenated word: '-' at j, preceded by two letters or letter-hyphen-letter,
            // followed by letter, optional hyphen, letter.
            if at(ji) == Some('-') {
                let behind = (at(ji - 2).is_some_and(is_letter)
                    && at(ji - 1).is_some_and(is_letter))
                    || (at(ji - 3).is_some_and(is_letter)
                        && at(ji - 2) == Some('-')
                        && at(ji - 1).is_some_and(is_letter));
                let ahead = at(ji + 1).is_some_and(is_letter)
                    && (at(ji + 2).is_some_and(is_letter)
                        || (at(ji + 2) == Some('-') && at(ji + 3).is_some_and(is_letter)));
                if behind && ahead {
                    break j + 1;
                }
            }
            // end of word
            if j == n || is_ws(text[j]) {
                break j;
            }
            // em-dash: (?<=wp)(?=-{2,}\w)
            if at(ji - 1).is_some_and(is_word_punct)
                && at(ji) == Some('-')
                && at(ji + 1) == Some('-')
            {
                let mut k = j;
                while k < n && text[k] == '-' {
                    k += 1;
                }
                if at(k as isize).is_some_and(super::is_word) {
                    break j;
                }
            }
            j += 1;
        };
        out.push(text[start..end].iter().collect());
        i = end;
    }
    out
}

fn clen(s: &str) -> usize {
    s.chars().count()
}

/// `textwrap.wrap(text, width, initial_indent=..., subsequent_indent=...)`.
pub fn wrap_indented(text: &str, width: usize, initial: &str, subsequent: &str) -> Vec<String> {
    let expanded = expand_tabs(text);
    let replaced: Vec<char> = expanded
        .chars()
        .map(|c| if is_ws(c) { ' ' } else { c })
        .collect();
    let mut chunks = chunks(&replaced);
    chunks.reverse();
    let mut lines: Vec<String> = Vec::new();
    while !chunks.is_empty() {
        let mut cur: Vec<String> = Vec::new();
        let mut cur_len = 0usize;
        let indent = if lines.is_empty() {
            initial
        } else {
            subsequent
        };
        let w = width as isize - clen(indent) as isize;
        if chunks.last().is_some_and(|c| c.trim().is_empty()) && !lines.is_empty() {
            chunks.pop();
        }
        while let Some(last) = chunks.last() {
            let l = clen(last) as isize;
            if cur_len as isize + l <= w {
                cur_len += l as usize;
                cur.push(chunks.pop().unwrap_or_default());
            } else {
                break;
            }
        }
        if chunks.last().is_some_and(|c| clen(c) as isize > w) {
            handle_long_word(&mut chunks, &mut cur, cur_len, w);
        }
        if cur.last().is_some_and(|c| c.trim().is_empty()) {
            cur.pop();
        }
        if !cur.is_empty() {
            lines.push(format!("{indent}{}", cur.concat()));
        }
    }
    lines
}

fn handle_long_word(chunks: &mut [String], cur: &mut Vec<String>, cur_len: usize, width: isize) {
    let space_left = if width < 1 {
        1
    } else {
        (width - cur_len as isize) as usize
    };
    let Some(last) = chunks.last_mut() else {
        return;
    };
    let chars: Vec<char> = last.chars().collect();
    let mut end = space_left;
    if chars.len() > space_left {
        // chunk.rfind('-', 0, space_left)
        if let Some(h) = chars[..space_left].iter().rposition(|&c| c == '-')
            && h > 0
            && chars[..h].iter().any(|&c| c != '-')
        {
            end = h + 1;
        }
    }
    let end = end.min(chars.len());
    cur.push(chars[..end].iter().collect());
    *last = chars[end..].iter().collect();
}

/// `str.expandtabs()` with tab size 8.
fn expand_tabs(text: &str) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    let mut out = String::new();
    let mut col = 0;
    for c in text.chars() {
        match c {
            '\t' => {
                let n = 8 - col % 8;
                out.push_str(&" ".repeat(n));
                col += n;
            }
            '\n' | '\r' => {
                out.push(c);
                col = 0;
            }
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    wrap_indented(text, width, "", "")
}

/// `textwrap.fill`.
pub fn fill(text: &str, width: usize, indent: &str) -> String {
    wrap_indented(text, width, indent, indent).join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_like_python() {
        assert_eq!(
            wrap(
                "Show, validate or migrate the project's .dev-cycle.json",
                38
            ),
            vec!["Show, validate or migrate the", "project's .dev-cycle.json"]
        );
        assert_eq!(
            wrap("With --migrate: write the migrated file in place", 22),
            vec!["With --migrate: write", "the migrated file in", "place"]
        );
        assert_eq!(
            wrap("a well-known long-winded thing", 12),
            vec!["a well-known", "long-winded", "thing"]
        );
        assert_eq!(wrap("abcdefghijkl", 5), vec!["abcde", "fghij", "kl"]);
    }
}
