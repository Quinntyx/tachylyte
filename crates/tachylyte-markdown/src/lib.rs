//! A small, dependency-free semantic Markdown engine.
//!
//! The parser deliberately keeps the original source and treats malformed syntax as
//! ordinary text.  This makes it suitable for editors: parsing never destroys text.

#![allow(
    clippy::trim_split_whitespace,
    clippy::manual_pattern_char_comparison,
    clippy::manual_strip
)]

/// A half-open byte span. Spans always fall on UTF-8 boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    /// Returns the covered source text when this span is valid for `source`.
    pub fn text<'a>(&self, source: &'a str) -> Option<&'a str> {
        (self.start <= self.end
            && self.end <= source.len()
            && source.is_char_boundary(self.start)
            && source.is_char_boundary(self.end))
        .then(|| &source[self.start..self.end])
    }
}

/// The editor's presentation mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Source,
    LivePreview,
    Reading,
}

/// A render-neutral inline node.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text {
        value: String,
        span: Span,
    },
    Emphasis {
        children: Vec<Inline>,
        span: Span,
    },
    Strong {
        children: Vec<Inline>,
        span: Span,
    },
    Code {
        value: String,
        span: Span,
    },
    Link {
        label: String,
        target: String,
        span: Span,
    },
    WikiLink {
        target: String,
        alias: Option<String>,
        heading: Option<String>,
        block: Option<String>,
        span: Span,
    },
    Embed {
        target: String,
        alias: Option<String>,
        span: Span,
    },
    Tag {
        value: String,
        span: Span,
    },
    Highlight {
        children: Vec<Inline>,
        span: Span,
    },
    Math {
        value: String,
        display: bool,
        span: Span,
    },
    FootnoteRef {
        label: String,
        span: Span,
    },
}

/// A block-level semantic node.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph {
        children: Vec<Inline>,
        span: Span,
    },
    Heading {
        level: u8,
        text: String,
        slug: String,
        children: Vec<Inline>,
        span: Span,
    },
    Quote {
        children: Vec<Block>,
        span: Span,
    },
    List {
        ordered: bool,
        items: Vec<ListItem>,
        span: Span,
    },
    Code {
        language: Option<String>,
        value: String,
        span: Span,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        span: Span,
    },
    ThematicBreak {
        span: Span,
    },
    Html {
        value: String,
        span: Span,
    },
    Callout {
        kind: String,
        title: Option<String>,
        foldable: Option<bool>,
        children: Vec<Block>,
        span: Span,
    },
    Comment {
        value: String,
        span: Span,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub key: String,
    pub value: String,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Frontmatter {
    pub raw: String,
    pub properties: Vec<Property>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Footnote {
    pub label: String,
    pub definition: String,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub target: String,
    pub alias: Option<String>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EmbedRef {
    pub target: String,
    pub alias: Option<String>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HeadingInfo {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub headings: Vec<HeadingInfo>,
}

/// Parsed immutable snapshot of a Markdown document.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    source: String,
    pub blocks: Vec<Block>,
    pub frontmatter: Option<Frontmatter>,
    pub footnotes: Vec<Footnote>,
    pub revision: u64,
    pub mode: ViewMode,
}

impl Document {
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        parse_document(source, 0, ViewMode::Source)
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn outline(&self) -> Outline {
        let mut headings = Vec::new();
        collect_headings(&self.blocks, &mut headings);
        Outline { headings }
    }
    pub fn links(&self) -> Vec<Link> {
        let mut out = Vec::new();
        collect_inlines(&self.blocks, &mut |i| {
            if let Inline::Link {
                label,
                target,
                span,
            } = i
            {
                out.push(Link {
                    target: target.clone(),
                    alias: Some(label.clone()),
                    span: *span,
                });
            }
        });
        out
    }
    pub fn wikilinks(&self) -> Vec<Link> {
        let mut out = Vec::new();
        collect_inlines(&self.blocks, &mut |i| {
            if let Inline::WikiLink {
                target,
                alias,
                span,
                ..
            } = i
            {
                out.push(Link {
                    target: target.clone(),
                    alias: alias.clone(),
                    span: *span,
                });
            }
        });
        out
    }
    pub fn embeds(&self) -> Vec<EmbedRef> {
        let mut out = Vec::new();
        collect_inlines(&self.blocks, &mut |i| {
            if let Inline::Embed {
                target,
                alias,
                span,
            } = i
            {
                out.push(EmbedRef {
                    target: target.clone(),
                    alias: alias.clone(),
                    span: *span,
                });
            }
        });
        out
    }
    pub fn tags(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_inlines(&self.blocks, &mut |i| {
            if let Inline::Tag { value, .. } = i {
                if !out.contains(value) {
                    out.push(value.clone());
                }
            }
        });
        out
    }
    pub fn properties(&self) -> &[Property] {
        self.frontmatter
            .as_ref()
            .map_or(&[], |f| f.properties.as_slice())
    }
    pub fn word_count(&self) -> usize {
        self.source.split_whitespace().count()
    }
    pub fn character_count(&self) -> usize {
        self.source.chars().count()
    }
    pub fn mode(&self) -> ViewMode {
        self.mode
    }
    pub fn with_mode(&self, mode: ViewMode) -> Self {
        let mut d = self.clone();
        d.mode = mode;
        d
    }
    pub fn resolve_heading(&self, slug: &str) -> Option<HeadingInfo> {
        self.outline().headings.into_iter().find(|h| h.slug == slug)
    }
    pub fn resolve_block(&self, id: &str) -> Option<Span> {
        find_block_id(&self.source, id)
    }
}

/// A mutable revisioned editor model with bounded undo/redo history.
#[derive(Clone, Debug)]
pub struct EditorDocument {
    document: Document,
    undo: Vec<String>,
    redo: Vec<String>,
    clean_source: String,
}
/// Maximum number of snapshots retained in each undo/redo stack.
pub const MAX_HISTORY: usize = 100;
impl EditorDocument {
    pub fn new(source: impl Into<String>) -> Self {
        let d = Document::parse(source);
        Self {
            clean_source: d.source.clone(),
            document: d,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn source(&self) -> &str {
        self.document.source()
    }
    pub fn is_dirty(&self) -> bool {
        self.source() != self.clean_source
    }
    pub fn mark_clean(&mut self) {
        self.clean_source = self.source().to_owned();
    }
    pub fn edit(&mut self, span: Span, replacement: &str) -> Result<(), EditError> {
        if span.end > self.source().len()
            || span.start > span.end
            || !self.source().is_char_boundary(span.start)
            || !self.source().is_char_boundary(span.end)
        {
            return Err(EditError::InvalidSpan);
        }
        self.undo.push(self.source().to_owned());
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        let mut text = self.source().to_owned();
        text.replace_range(span.start..span.end, replacement);
        self.document = parse_document(
            text,
            self.document.revision.saturating_add(1),
            self.document.mode,
        );
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(source) = self.undo.pop() {
            self.redo.push(self.source().to_owned());
            if self.redo.len() > MAX_HISTORY {
                self.redo.remove(0);
            }
            self.document = parse_document(
                source,
                self.document.revision.saturating_add(1),
                self.document.mode,
            );
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(source) = self.redo.pop() {
            self.undo.push(self.source().to_owned());
            if self.undo.len() > MAX_HISTORY {
                self.undo.remove(0);
            }
            self.document = parse_document(
                source,
                self.document.revision.saturating_add(1),
                self.document.mode,
            );
            true
        } else {
            false
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditError {
    InvalidSpan,
}

fn parse_document(source: String, revision: u64, mode: ViewMode) -> Document {
    let (frontmatter, body_start) = parse_frontmatter(&source);
    let body = &source[body_start..];
    let mut blocks = Vec::new();
    let lines: Vec<(usize, &str)> = body
        .split_inclusive('\n')
        .scan(body_start, |pos, line| {
            let start = *pos;
            *pos += line.len();
            Some((start, line.trim_end_matches('\n').trim_end_matches('\r')))
        })
        .collect();
    let mut i = 0;
    let mut footnotes = Vec::new();
    while i < lines.len() {
        let (start, line) = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if line.starts_with("<!--") {
            let mut end = i;
            while end < lines.len() && !lines[end].1.contains("-->") {
                end += 1;
            }
            if end < lines.len() {
                end += 1;
            }
            let e = lines
                .get(end.saturating_sub(1))
                .map_or(start + line.len(), |x| x.0 + x.1.len());
            blocks.push(Block::Comment {
                value: source[start..e].to_owned(),
                span: Span::new(start, e),
            });
            i = end;
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            let marker = &line[..3];
            let language = line[3..]
                .trim()
                .split_whitespace()
                .next()
                .filter(|x| !x.is_empty())
                .map(str::to_owned);
            let mut j = i + 1;
            while j < lines.len() && !lines[j].1.starts_with(marker) {
                j += 1;
            }
            let e = lines.get(j).map_or(source.len(), |x| x.0 + x.1.len());
            let content_start =
                start + line.len() + newline_width_after(&source, start + line.len());
            let content_end = lines.get(j).map_or(e, |x| {
                x.0.saturating_sub(newline_width_before(&source, x.0))
            });
            blocks.push(Block::Code {
                language,
                value: source[content_start..content_end.min(source.len())].to_owned(),
                span: Span::new(start, e),
            });
            i = (j + 1).min(lines.len());
            continue;
        }
        if let Some((level, text, text_offset)) = heading(line) {
            let e = start + line.len();
            let span = Span::new(start, e);
            blocks.push(Block::Heading {
                level,
                text: text.to_owned(),
                slug: slugify(text),
                children: parse_inlines(text, start + text_offset),
                span,
            });
            i += 1;
            continue;
        }
        if line.starts_with('>') {
            let mut j = i;
            let mut content_lines: Vec<(usize, &str)> = Vec::new();
            while j < lines.len() && lines[j].1.starts_with('>') {
                let after = &lines[j].1[1..];
                let whitespace = after.len() - after.trim_start().len();
                content_lines.push((lines[j].0 + 1 + whitespace, after.trim_start()));
                j += 1;
            }
            let e = lines[j - 1].0 + lines[j - 1].1.len();
            let callout = content_lines
                .first()
                .and_then(|(_, text)| parse_callout(text));
            if let Some((kind, title, fold)) = callout {
                let body = &content_lines[1..];
                let child_start = body.first().map_or(e, |x| x.0);
                let child_end = body.last().map_or(child_start, |x| x.0 + x.1.len());
                blocks.push(Block::Callout {
                    kind,
                    title,
                    foldable: fold,
                    children: vec![Block::Paragraph {
                        children: body
                            .iter()
                            .flat_map(|(offset, text)| parse_inlines(text, *offset))
                            .collect(),
                        span: Span::new(child_start, child_end),
                    }],
                    span: Span::new(start, e),
                });
            } else {
                blocks.push(Block::Quote {
                    children: vec![Block::Paragraph {
                        children: content_lines
                            .iter()
                            .flat_map(|(offset, text)| parse_inlines(text, *offset))
                            .collect(),
                        span: Span::new(
                            content_lines.first().map_or(start, |x| x.0),
                            content_lines.last().map_or(e, |x| x.0 + x.1.len()),
                        ),
                    }],
                    span: Span::new(start, e),
                });
            }
            i = j;
            continue;
        }
        if let Some((ordered, _item, _)) = list_marker_at(line) {
            let mut items = Vec::new();
            let mut j = i;
            while j < lines.len() {
                if let Some((o, t, content_offset)) = list_marker_at(lines[j].1) {
                    if o != ordered {
                        break;
                    }
                    let checked = task_state(t);
                    let task_prefix = if t.starts_with("[ ] ")
                        || t.starts_with("[x] ")
                        || t.starts_with("[X] ")
                    {
                        4
                    } else {
                        0
                    };
                    let content = &t[task_prefix..];
                    let content_start = lines[j].0 + content_offset + task_prefix;
                    items.push(ListItem {
                        checked,
                        blocks: vec![Block::Paragraph {
                            children: parse_inlines(content, content_start),
                            span: Span::new(content_start, lines[j].0 + lines[j].1.len()),
                        }],
                        span: Span::new(lines[j].0, lines[j].0 + lines[j].1.len()),
                    });
                    j += 1;
                } else {
                    break;
                }
            }
            blocks.push(Block::List {
                ordered,
                items,
                span: Span::new(start, lines[j - 1].0 + lines[j - 1].1.len()),
            });
            i = j;
            continue;
        }
        if line.trim_start().starts_with('|')
            && i + 1 < lines.len()
            && lines[i + 1].1.contains("---")
        {
            let mut j = i + 2;
            while j < lines.len() && lines[j].1.trim_start().starts_with('|') {
                j += 1;
            }
            let cells = split_table(line);
            let rows = (i + 2..j).map(|k| split_table(lines[k].1)).collect();
            blocks.push(Block::Table {
                headers: cells,
                rows,
                span: Span::new(start, lines[j - 1].0 + lines[j - 1].1.len()),
            });
            i = j;
            continue;
        }
        if line
            .trim()
            .chars()
            .all(|c| c == '-' || c == '*' || c == '_')
            && line.trim().len() >= 3
        {
            blocks.push(Block::ThematicBreak {
                span: Span::new(start, start + line.len()),
            });
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && !lines[j].1.trim().is_empty() {
            j += 1;
        }
        let e = lines[j - 1].0 + lines[j - 1].1.len();
        let text = &source[start..e];
        for (label, definition, span) in extract_footnotes(text, start) {
            footnotes.push(Footnote {
                label,
                definition,
                span,
            });
        }
        blocks.push(Block::Paragraph {
            children: parse_inlines(text, start),
            span: Span::new(start, e),
        });
        i = j;
    }
    Document {
        source,
        blocks,
        frontmatter,
        footnotes,
        revision,
        mode,
    }
}

fn parse_frontmatter(s: &str) -> (Option<Frontmatter>, usize) {
    let mut lines = s.split_inclusive('\n').scan(0usize, |offset, raw| {
        let start = *offset;
        *offset += raw.len();
        Some((
            start,
            raw.trim_end_matches('\n').trim_end_matches('\r'),
            raw.len(),
        ))
    });
    let Some((_, first, first_len)) = lines.next() else {
        return (None, 0);
    };
    if first != "---" {
        return (None, 0);
    }
    let mut close = None;
    for (start, line, len) in lines {
        if line == "---" {
            close = Some((start, start + len));
            break;
        }
    }
    let Some((close_start, end)) = close else {
        return (None, 0);
    };
    // Exclude only the line break separating the final body line from the
    // closing delimiter; preserve every byte inside the YAML body.
    let raw_end = close_start.saturating_sub(newline_width_before(s, close_start));
    let raw = s[first_len..raw_end].to_owned();
    let mut properties = Vec::new();
    let mut at = first_len;
    for line in s[first_len..raw_end].split_inclusive('\n') {
        let content = line.trim_end_matches('\n').trim_end_matches('\r');
        let len = content.len();
        if let Some(k) = content.find(':') {
            let key = content[..k].trim();
            if !key.is_empty() {
                properties.push(Property {
                    key: key.to_owned(),
                    value: content[k + 1..].trim().to_owned(),
                    span: Span::new(at, at + len),
                });
            }
        }
        at += line.len();
    }
    (
        Some(Frontmatter {
            raw,
            properties,
            span: Span::new(0, end),
        }),
        end,
    )
}
fn newline_width_after(source: &str, at: usize) -> usize {
    match source.as_bytes().get(at..at + 2) {
        Some(b"\r\n") => 2,
        Some(b"\n\n") => 1,
        _ => 0,
    }
}
fn newline_width_before(source: &str, at: usize) -> usize {
    if at >= 2 && &source.as_bytes()[at - 2..at] == b"\r\n" {
        2
    } else if at >= 1 && source.as_bytes()[at - 1] == b'\n' {
        1
    } else {
        0
    }
}
fn heading(line: &str) -> Option<(u8, &str, usize)> {
    let n = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&n) && line.as_bytes().get(n) == Some(&b' ') {
        let rest = &line[n + 1..];
        let leading = rest.len() - rest.trim_start().len();
        Some((n as u8, rest.trim(), n + 1 + leading))
    } else {
        None
    }
}
fn slugify(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
        .map(|c| {
            if c.is_whitespace() {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
}
fn list_marker(s: &str) -> Option<(bool, &str)> {
    let t = s.trim_start();
    let n = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if n > 0 && t.as_bytes().get(n) == Some(&b'.') && t.as_bytes().get(n + 1) == Some(&b' ') {
        Some((true, &t[n + 2..]))
    } else if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        Some((false, &t[2..]))
    } else {
        None
    }
}
fn list_marker_at(s: &str) -> Option<(bool, &str, usize)> {
    let leading = s.len() - s.trim_start().len();
    let t = &s[leading..];
    let (ordered, content) = list_marker(s)?;
    Some((ordered, content, leading + t.len() - content.len()))
}
fn task_state(s: &str) -> Option<bool> {
    if s.starts_with("[ ] ") {
        Some(false)
    } else if s.starts_with("[x] ") || s.starts_with("[X] ") {
        Some(true)
    } else {
        None
    }
}
fn split_table(s: &str) -> Vec<String> {
    s.trim()
        .trim_matches('|')
        .split('|')
        .map(|x| x.trim().to_owned())
        .collect()
}
fn parse_callout(s: &str) -> Option<(String, Option<String>, Option<bool>)> {
    if !s.starts_with("[!") {
        return None;
    }
    let end = s.find(']')?;
    let head = &s[2..end];
    let (kind, fold) = if let Some(k) = head.strip_suffix('+') {
        (k.to_owned(), Some(true))
    } else if let Some(k) = head.strip_suffix('-') {
        (k.to_owned(), Some(false))
    } else {
        (head.to_owned(), None)
    };
    let title = s[end + 1..].trim();
    Some((kind, (!title.is_empty()).then(|| title.to_owned()), fold))
}
fn extract_footnotes(s: &str, base: usize) -> Vec<(String, String, Span)> {
    let mut offset = 0;
    s.split_inclusive('\n')
        .filter_map(|raw| {
            let l = raw.trim_end_matches('\n').trim_end_matches('\r');
            let line_start = base + offset;
            offset += raw.len();
            let rest = l.strip_prefix("[^")?;
            let k = rest.find("]: ")?;
            if rest[..k].is_empty() || rest[..k].contains(|c: char| c.is_whitespace() || c == ']') {
                return None;
            }
            let label = &rest[..k];
            Some((
                label.to_owned(),
                rest[k + 3..].to_owned(),
                Span::new(line_start, line_start + l.len()),
            ))
        })
        .collect()
}

fn parse_inlines(s: &str, base: usize) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut text_start = 0;
    while i < s.len() {
        let rest = &s[i..];
        let found = rest.find(|c| matches!(c, '[' | '!' | '`' | '#' | '~' | '$' | '<' | '*' | '_'));
        let Some(rel) = found else { break };
        let p = i + rel;
        if is_escaped(s, p) && matches!(s.as_bytes().get(p), Some(b'*' | b'_')) {
            i = p + 1;
            continue;
        }
        if p > text_start {
            out.push(Inline::Text {
                value: s[text_start..p].to_owned(),
                span: Span::new(base + text_start, base + p),
            });
        }
        let r = &s[p..];
        // Delimiter runs are parsed before links and other constructs. A
        // marker preceded by a backslash is literal, and an unclosed pair is
        // deliberately left in the text stream for editor-friendly recovery.
        let mut delimiter_parsed = false;
        for (marker, strong) in [("**", true), ("__", true), ("*", false), ("_", false)] {
            if r.starts_with(marker)
                && !is_escaped(s, p)
                && !(marker.len() == 1 && r.as_bytes().get(1) == Some(&marker.as_bytes()[0]))
                && (marker.len() == 2 || marker == "*" || !adjacent_word_marker(s, p, marker.len()))
            {
                if let Some(close) = find_closing_marker(s, p + marker.len(), marker) {
                    let inside = &s[p + marker.len()..close];
                    if !inside.is_empty() {
                        let span = Span::new(base + p, base + close + marker.len());
                        let children = parse_inlines(inside, base + p + marker.len());
                        if strong {
                            out.push(Inline::Strong { children, span });
                        } else {
                            out.push(Inline::Emphasis { children, span });
                        }
                        i = close + marker.len();
                        text_start = i;
                        delimiter_parsed = true;
                        break;
                    }
                }
            }
        }
        if delimiter_parsed {
            continue;
        }
        if r.starts_with("![[") {
            if let Some(e) = r[3..].find("]]") {
                let e = p + 3 + e;
                let (target, alias) = split_target(&s[p + 3..e]);
                if !target.trim().is_empty() {
                    out.push(Inline::Embed {
                        target,
                        alias,
                        span: Span::new(base + p, base + e + 2),
                    });
                }
                i = e + 2;
                text_start = i;
                continue;
            }
        }
        if r.starts_with("[[") {
            if let Some(e) = r[2..].find("]]") {
                let e = p + 2 + e;
                let raw = &s[p + 2..e];
                let (target, alias) = split_target(raw);
                let (target, heading, block) = split_wiki_target(&target);
                out.push(Inline::WikiLink {
                    target,
                    alias,
                    heading,
                    block,
                    span: Span::new(base + p, base + e + 2),
                });
                i = e + 2;
                text_start = i;
                continue;
            }
        }
        if r.starts_with('`') {
            if let Some(e) = r[1..].find('`') {
                let e = p + 1 + e;
                out.push(Inline::Code {
                    value: s[p + 1..e].to_owned(),
                    span: Span::new(base + p, base + e + 1),
                });
                i = e + 1;
                text_start = i;
                continue;
            }
        }
        if r.starts_with("==") {
            if let Some(e) = r[2..].find("==") {
                let e = p + 2 + e;
                out.push(Inline::Highlight {
                    children: vec![Inline::Text {
                        value: s[p + 2..e].to_owned(),
                        span: Span::new(base + p + 2, base + e),
                    }],
                    span: Span::new(base + p, base + e + 2),
                });
                i = e + 2;
                text_start = i;
                continue;
            }
        }
        if r.starts_with("$") {
            let display = r.starts_with("$$");
            let q = if display { "$$" } else { "$" };
            if let Some(e) = r[q.len()..].find(q) {
                let e = p + q.len() + e;
                out.push(Inline::Math {
                    value: s[p + q.len()..e].to_owned(),
                    display,
                    span: Span::new(base + p, base + e + q.len()),
                });
                i = e + q.len();
                text_start = i;
                continue;
            }
        }
        if r.starts_with("[^") {
            if let Some(e) = r.find(']') {
                let label = &r[2..e];
                if !label.is_empty() && !label.contains(|c: char| c.is_whitespace() || c == ']') {
                    out.push(Inline::FootnoteRef {
                        label: label.to_owned(),
                        span: Span::new(base + p, base + p + e + 1),
                    });
                    i = p + e + 1;
                    text_start = i;
                    continue;
                }
            }
        }
        if r.starts_with('[') {
            if let Some(mid) = r.find("](") {
                if let Some(e) = r[mid + 2..].find(')') {
                    let e = p + mid + 2 + e;
                    let label = &s[p + 1..p + mid];
                    let target = &s[p + mid + 2..e];
                    if !label.is_empty() && !target.is_empty() && !target.contains(['\n', '\r']) {
                        out.push(Inline::Link {
                            label: label.to_owned(),
                            target: target.to_owned(),
                            span: Span::new(base + p, base + e + 1),
                        });
                        i = e + 1;
                        text_start = i;
                        continue;
                    }
                }
            }
        }
        if r.starts_with('#') && (p == 0 || s.as_bytes()[p - 1].is_ascii_whitespace()) {
            let e = r.find(|c: char| c.is_whitespace()).unwrap_or(r.len());
            if e > 1 && valid_tag(&r[1..e]) {
                out.push(Inline::Tag {
                    value: r[1..e].to_owned(),
                    span: Span::new(base + p, base + p + e),
                });
                i = p + e;
                text_start = i;
                continue;
            }
        }
        let marker_width = if s[p..].starts_with("**") || s[p..].starts_with("__") {
            2
        } else {
            1
        };
        i = p + marker_width;
        text_start = p;
    }
    if text_start < s.len() {
        out.push(Inline::Text {
            value: s[text_start..].to_owned(),
            span: Span::new(base + text_start, base + s.len()),
        });
    }
    out
}
fn is_escaped(source: &str, at: usize) -> bool {
    let mut slashes = 0;
    let bytes = source.as_bytes();
    let mut i = at;
    while i > 0 && bytes[i - 1] == b'\\' {
        slashes += 1;
        i -= 1;
    }
    slashes % 2 == 1
}
fn adjacent_word_marker(source: &str, at: usize, width: usize) -> bool {
    let before = at.checked_sub(1).and_then(|i| source[i..].chars().next());
    let after = source[at + width..].chars().next();
    before.is_some_and(|c| c.is_alphanumeric()) || after.is_some_and(|c| c.is_alphanumeric())
}
fn find_closing_marker(source: &str, from: usize, marker: &str) -> Option<usize> {
    let mut at = from;
    while let Some(relative) = source[at..].find(marker) {
        let mut candidate = at + relative;
        if !is_escaped(source, candidate) {
            // A three-star closing run belongs to the outer `**` and the
            // inner `*`; use the final two bytes for the strong delimiter.
            if marker.len() == 2 {
                let byte = marker.as_bytes()[0];
                let mut end = candidate + marker.len();
                while source.as_bytes().get(end) == Some(&byte) {
                    end += 1;
                }
                candidate = end - marker.len();
            }
            return Some(candidate);
        }
        at = candidate + marker.len();
    }
    None
}
fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
}
fn split_target(s: &str) -> (String, Option<String>) {
    s.split_once('|').map_or((s.to_owned(), None), |(a, b)| {
        (a.to_owned(), Some(b.to_owned()))
    })
}
fn split_wiki_target(s: &str) -> (String, Option<String>, Option<String>) {
    let (without_block, block) = s
        .split_once('^')
        .map_or((s, None), |(a, b)| (a, Some(b.to_owned())));
    let (target, heading) = without_block
        .split_once('#')
        .map_or((without_block, None), |(a, b)| (a, Some(b.to_owned())));
    (target.to_owned(), heading, block)
}
fn collect_inlines<F: FnMut(&Inline)>(blocks: &[Block], f: &mut F) {
    for b in blocks {
        match b {
            Block::Paragraph { children, .. } | Block::Heading { children, .. } => {
                children.iter().for_each(&mut *f)
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_inlines(&item.blocks, f);
                }
            }
            Block::Quote { children, .. } | Block::Callout { children, .. } => {
                collect_inlines(children, f)
            }
            _ => {}
        }
    }
}
fn collect_headings(blocks: &[Block], out: &mut Vec<HeadingInfo>) {
    for b in blocks {
        match b {
            Block::Heading {
                level,
                text,
                slug,
                span,
                ..
            } => out.push(HeadingInfo {
                level: *level,
                text: text.clone(),
                slug: slug.clone(),
                span: *span,
            }),
            Block::Quote { children, .. } | Block::Callout { children, .. } => {
                collect_headings(children, out)
            }
            Block::List { items, .. } => {
                items.iter().for_each(|i| collect_headings(&i.blocks, out))
            }
            _ => {}
        }
    }
}
fn find_block_id(source: &str, id: &str) -> Option<Span> {
    if id.is_empty() {
        return None;
    }
    let needle = format!("^{}", id);
    let mut offset = 0;
    source.split_inclusive('\n').find_map(|raw| {
        let line = raw.trim_end_matches('\n').trim_end_matches('\r');
        let start = offset;
        offset += raw.len();
        line.split_whitespace()
            .any(|part| part == needle)
            .then_some(Span::new(start, start + line.len()))
    })
}
