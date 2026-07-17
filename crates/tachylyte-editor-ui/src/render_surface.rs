//! Dependency-free Markdown surface classification.
//!
//! This is deliberately a syntax surface, rather than a renderer: consumers can
//! map the returned nodes to whatever view toolkit they use.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    FrontmatterProperty,
    Heading,
    Paragraph,
    Wikilink,
    Embed,
    Callout,
    TaskCheckbox,
    Table,
    FencedCode,
    Math,
    Diagram,
    Footnote,
    BlockReference,
    Tag,
    HorizontalRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceNode {
    pub kind: NodeKind,
    pub range: SourceRange,
    /// The source spelling (without a line ending), useful for later rendering.
    pub text: String,
    /// Optional classification detail, such as a frontmatter key or checkbox state.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderSurface {
    pub nodes: Vec<SurfaceNode>,
}

impl RenderSurface {
    pub fn parse(source: &str) -> Self {
        Self {
            nodes: classify(source),
        }
    }
}

pub fn render_surface(source: &str) -> RenderSurface {
    RenderSurface::parse(source)
}

fn node(
    kind: NodeKind,
    start: usize,
    end: usize,
    text: &str,
    detail: Option<String>,
) -> SurfaceNode {
    SurfaceNode {
        kind,
        range: SourceRange { start, end },
        text: text.to_owned(),
        detail,
    }
}

fn classify(source: &str) -> Vec<SurfaceNode> {
    let mut out = Vec::new();
    let lines = source
        .split_inclusive('\n')
        .map(|s| s.strip_suffix('\n').unwrap_or(s));
    let mut offset = 0;
    let mut frontmatter = false;
    let mut fence: Option<(usize, String)> = None;
    for line in lines {
        let start = offset;
        offset +=
            line.len() + usize::from(source.as_bytes().get(offset + line.len()) == Some(&b'\n'));
        let trimmed = line.trim();
        if start == 0 && trimmed == "---" {
            frontmatter = true;
            continue;
        }
        if frontmatter {
            if trimmed == "---" {
                frontmatter = false;
                continue;
            }
            if let Some(colon) = line.find(':') {
                let key = line[..colon].trim();
                if !key.is_empty() {
                    out.push(node(
                        NodeKind::FrontmatterProperty,
                        start,
                        start + line.len(),
                        line,
                        Some(key.to_owned()),
                    ));
                }
            }
            continue;
        }
        if let Some((fstart, marker)) = fence.clone() {
            if trimmed.starts_with(&marker) {
                out.push(node(
                    NodeKind::FencedCode,
                    fstart,
                    start + line.len(),
                    &source[fstart..start + line.len()],
                    Some(marker),
                ));
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some((start, trimmed[..3].to_owned()));
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push(node(
                NodeKind::HorizontalRule,
                start,
                start + line.len(),
                line,
                None,
            ));
            continue;
        }
        if trimmed.starts_with("$$") || trimmed.ends_with("$$") {
            out.push(node(NodeKind::Math, start, start + line.len(), line, None));
            continue;
        }
        if trimmed.starts_with("```mermaid") || trimmed.starts_with("```diagram") {
            out.push(node(
                NodeKind::Diagram,
                start,
                start + line.len(),
                line,
                None,
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> [!") {
            if let Some(end) = rest.find(']') {
                out.push(node(
                    NodeKind::Callout,
                    start,
                    start + line.len(),
                    line,
                    Some(rest[..end].to_owned()),
                ));
                continue;
            }
        }
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            out.push(node(NodeKind::Table, start, start + line.len(), line, None));
            continue;
        }
        if trimmed.starts_with("[^") {
            out.push(node(
                NodeKind::Footnote,
                start,
                start + line.len(),
                line,
                None,
            ));
            continue;
        }
        if trimmed.starts_with("^[]") || trimmed.starts_with("^(") {
            out.push(node(
                NodeKind::BlockReference,
                start,
                start + line.len(),
                line,
                None,
            ));
            continue;
        }
        if trimmed.starts_with('#') {
            let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
            if hashes > 0 && trimmed.as_bytes().get(hashes) == Some(&b' ') {
                out.push(node(
                    NodeKind::Heading,
                    start,
                    start + line.len(),
                    line,
                    Some(hashes.to_string()),
                ));
            }
        }
        if let Some(pos) = trimmed.find("- [") {
            if trimmed
                .as_bytes()
                .get(pos + 3)
                .is_some_and(|b| *b == b' ' || *b == b'x' || *b == b'X')
            {
                let state = trimmed[pos + 3..].chars().next().unwrap_or(' ');
                out.push(node(
                    NodeKind::TaskCheckbox,
                    start,
                    start + line.len(),
                    line,
                    Some((state == 'x' || state == 'X').to_string()),
                ));
            }
        }
        inline_nodes(source, start, line, &mut out);
        if !trimmed.is_empty()
            && !out.iter().any(|n| {
                n.range
                    == SourceRange {
                        start,
                        end: start + line.len(),
                    }
            })
        {
            out.push(node(
                NodeKind::Paragraph,
                start,
                start + line.len(),
                line,
                None,
            ));
        }
    }
    out
}

fn inline_nodes(source: &str, base: usize, line: &str, out: &mut Vec<SurfaceNode>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let (open, close, kind) = if bytes[i..].starts_with(b"![[") {
            (3, "]]", NodeKind::Embed)
        } else if bytes[i..].starts_with(b"[[") {
            (2, "]]", NodeKind::Wikilink)
        } else if bytes[i] == b'#' {
            (1, "", NodeKind::Tag)
        } else {
            i += 1;
            continue;
        };
        if kind == NodeKind::Tag {
            if i + 1 < bytes.len() && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
                let e = bytes[i + 1..]
                    .iter()
                    .position(|b| b.is_ascii_whitespace())
                    .map_or(bytes.len(), |p| i + 1 + p);
                out.push(node(kind, base + i, base + e, &line[i..e], None));
                i = e;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(p) = line[i + open..].find(close) {
            let e = i + open + p + close.len();
            out.push(node(kind, base + i, base + e, &line[i..e], None));
            i = e;
        } else {
            i += open;
        }
    }
    let _ = source;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_surface_features() {
        let s="---\ntitle: Demo\n---\n> [!NOTE] hi\n- [x] done [[page]] ![[img.png]] #tag\n| a | b |\n---\n";
        let r = render_surface(s);
        assert!(r
            .nodes
            .iter()
            .any(|n| n.kind == NodeKind::FrontmatterProperty));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::Callout));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::Wikilink));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::Embed));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::TaskCheckbox));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::Table));
        assert!(r.nodes.iter().any(|n| n.kind == NodeKind::HorizontalRule));
    }
    #[test]
    fn ranges_are_source_offsets() {
        let s = "[[x]]";
        let n = &render_surface(s).nodes[0];
        assert_eq!(&s[n.range.start..n.range.end], "[[x]]");
    }
}
