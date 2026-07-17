//! Small, renderer-independent intents produced by interactions with Markdown.
//!
//! The scanner in this module only borrows its input.  It deliberately does not
//! try to be a Markdown parser; callers can use it as a conservative hit-test
//! helper and decide how to execute the returned intent.

use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub const fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub fn range(self) -> Range<usize> { self.start..self.end }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorIntent {
    Link { destination: String, span: SourceSpan },
    WikiLink { target: String, span: SourceSpan },
    Tag { name: String, span: SourceSpan },
    ToggleTask { checked: bool, span: SourceSpan },
    Embed { target: String, span: SourceSpan },
    NavigateBlock { reference: String, span: SourceSpan },
}

impl EditorIntent {
    /// Finds the innermost actionable construct containing `offset`.
    pub fn at(source: &str, offset: usize) -> Option<Self> {
        if !source.is_char_boundary(offset) { return None; }
        scan(source).into_iter().find(|intent| intent.span().start <= offset && offset <= intent.span().end)
    }

    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Link { span, .. } | Self::WikiLink { span, .. }
            | Self::Tag { span, .. } | Self::ToggleTask { span, .. }
            | Self::Embed { span, .. } | Self::NavigateBlock { span, .. } => *span,
        }
    }
}

pub fn scan(source: &str) -> Vec<EditorIntent> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if source[i..].starts_with("![[") {
            if let Some(end) = source[i + 3..].find("]]" ) {
                let e = i + 3 + end + 2;
                out.push(EditorIntent::Embed { target: source[i + 3..i + 3 + end].to_owned(), span: SourceSpan::new(i, e) });
                i = e; continue;
            }
        } else if source[i..].starts_with("[[") {
            if let Some(end) = source[i + 2..].find("]]" ) {
                let e = i + 2 + end + 2;
                let target = &source[i + 2..i + 2 + end];
                if let Some(block) = target.rfind("#^") {
                    out.push(EditorIntent::NavigateBlock { reference: target[block + 2..].to_owned(), span: SourceSpan::new(i, e) });
                } else {
                    out.push(EditorIntent::WikiLink { target: target.to_owned(), span: SourceSpan::new(i, e) });
                }
                i = e; continue;
            }
        } else if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] != b' ' {
            if let Some(close) = source[i + 1..].find(']') {
                let after = i + 1 + close + 1;
                if source[after..].starts_with('(') {
                    if let Some(end) = source[after + 1..].find(')') {
                        let e = after + 1 + end + 1;
                        out.push(EditorIntent::Link { destination: source[after + 1..after + 1 + end].trim().to_owned(), span: SourceSpan::new(i, e) });
                        i = e; continue;
                    }
                }
            }
        }
        i += source[i..].chars().next().map_or(1, char::len_utf8);
    }
    for (line_start, line) in source.split_inclusive('\n').scan(0, |pos, line| { let p = *pos; *pos += line.len(); Some((p, line)) }) {
        let content = line.trim_end_matches(['\n', '\r']);
        let lead = content.len() - content.trim_start().len();
        let rest = &content[lead..];
        if let Some(task) = rest.find("- [") {
            let marker = task + 3;
            if matches!(rest.as_bytes().get(marker), Some(b' ' | b'x' | b'X'))
                && rest.as_bytes().get(marker + 1) == Some(&b']')
            {
                let checked = rest.as_bytes()[marker].eq_ignore_ascii_case(&b'x');
                out.push(EditorIntent::ToggleTask {
                    checked,
                    span: SourceSpan::new(
                        line_start + lead + task + 2,
                        line_start + lead + task + 5,
                    ),
                });
            }
        }
        for (j, c) in content.char_indices() {
            if c == '#' && (j == 0 || content.as_bytes()[j - 1].is_ascii_whitespace()) {
                let end = content[j + 1..].find(char::is_whitespace).map_or(content.len(), |n| j + 1 + n);
                if end > j + 1 { out.push(EditorIntent::Tag { name: content[j + 1..end].to_owned(), span: SourceSpan::new(line_start + j, line_start + end) }); }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn scans_common_intents() {
        let got = scan("[x](url) [[Page]] ![[img.png]] - [x] done #tag [[N#^b]]");
        assert!(got.iter().any(|x| matches!(x, EditorIntent::Link { destination, .. } if destination == "url")));
        assert!(got.iter().any(|x| matches!(x, EditorIntent::Embed { target, .. } if target == "img.png")));
        assert!(got.iter().any(|x| matches!(x, EditorIntent::ToggleTask { checked: true, .. })));
        assert!(got.iter().any(|x| matches!(x, EditorIntent::NavigateBlock { reference, .. } if reference == "b")));
    }
    #[test] fn invalid_offsets_are_safe() { assert!(EditorIntent::at("é", 1).is_none()); assert!(scan("[[unterminated").is_empty()); }
}
