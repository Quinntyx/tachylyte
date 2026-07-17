use tachylyte_markdown::{Block, Document, EditorDocument, Inline, Span, ViewMode, MAX_HISTORY};

fn assert_span(source: &str, span: Span) {
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
    assert!(span.text(source).is_some());
}
fn walk_inline(source: &str, nodes: &[Inline]) {
    for node in nodes {
        let span = match node {
            Inline::Text { span, .. }
            | Inline::Code { span, .. }
            | Inline::Link { span, .. }
            | Inline::WikiLink { span, .. }
            | Inline::Embed { span, .. }
            | Inline::Tag { span, .. }
            | Inline::Math { span, .. }
            | Inline::FootnoteRef { span, .. } => *span,
            Inline::Emphasis { span, children }
            | Inline::Strong { span, children }
            | Inline::Highlight { span, children } => {
                assert_span(source, *span);
                walk_inline(source, children);
                *span
            }
        };
        assert_span(source, span);
    }
}
fn walk_blocks(source: &str, blocks: &[Block]) {
    for block in blocks {
        match block {
            Block::Paragraph { children, span } | Block::Heading { children, span, .. } => {
                assert_span(source, *span);
                walk_inline(source, children);
            }
            Block::Quote { children, span } | Block::Callout { children, span, .. } => {
                assert_span(source, *span);
                walk_blocks(source, children);
            }
            Block::List { items, span, .. } => {
                assert_span(source, *span);
                for item in items {
                    assert_span(source, item.span);
                    walk_blocks(source, &item.blocks);
                }
            }
            Block::Code { span, .. }
            | Block::Table { span, .. }
            | Block::ThematicBreak { span }
            | Block::Html { span, .. }
            | Block::Comment { span, .. } => assert_span(source, *span),
        }
    }
}

#[test]
fn extracts_obsidian_semantics_and_preserves_yaml() {
    let d = Document::parse("---\ntitle: Café\ncustom: [one, two]\n---\n# Hello World\nSee [[Notes#Part one|alias]] and ![[image.png|image]]. #tag ==hi== $x$");
    assert_eq!(d.properties()[0].key, "title");
    assert_eq!(d.properties()[0].value, "Café");
    assert_eq!(d.outline().headings[0].slug, "hello-world");
    assert_eq!(d.wikilinks()[0].target, "Notes");
    assert_eq!(d.embeds()[0].target, "image.png");
    assert_eq!(d.tags(), vec!["tag"]);
    assert!(d.character_count() > d.word_count());
}

#[test]
fn unicode_edits_are_boundary_safe_and_revisioned() {
    let mut d = EditorDocument::new("naïve café");
    assert!(d.edit(Span::new(0, 6), "🌱").is_ok());
    assert_eq!(d.source(), "🌱 café");
    assert!(d.is_dirty());
    assert!(d.undo());
    assert_eq!(d.source(), "naïve café");
    assert!(d.redo());
    d.mark_clean();
    assert!(!d.is_dirty());
    assert_eq!(
        d.document().with_mode(ViewMode::Reading).mode(),
        ViewMode::Reading
    );
}

#[test]
fn malformed_markup_remains_text() {
    let d = Document::parse("[[unfinished\n[bad](target");
    assert_eq!(d.links().len(), 0);
    assert!(d.source().contains("unfinished"));
}

#[test]
fn blocks_tasks_callouts_tables_and_footnotes() {
    let d = Document::parse("> [!WARNING]- Check\n> body\n\n- [x] done\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n[^n]: note\nSee [^n]");
    assert!(d.blocks.len() >= 4);
    assert_eq!(d.footnotes[0].label, "n");
}

#[test]
fn every_unicode_crlf_span_is_a_source_span() {
    let source = "---\r\ntitle: café\r\n---\r\n> [!NOTE]+ Ün\r\n> [[文書#章|表示]] #tag\r\n\r\n- [x] naïve ^block\r\n\r\n[^a]: résumé\r\n";
    let d = Document::parse(source);
    walk_blocks(source, &d.blocks);
    if let Some(front) = &d.frontmatter {
        assert_span(source, front.span);
        for p in &front.properties {
            assert_span(source, p.span);
        }
    }
    for f in &d.footnotes {
        assert_span(source, f.span);
    }
    assert_eq!(
        d.resolve_block("block").unwrap().text(source),
        Some("- [x] naïve ^block")
    );
}

#[test]
fn frontmatter_requires_delimiter_lines_and_malformed_syntax_is_text() {
    let d = Document::parse("---\r\nkey: value\r\nnot --- a delimiter\r\n---x\r\nbody");
    assert!(d.frontmatter.is_none());
    let d = Document::parse("---\r\nkey: value\r\n---\r\nbody");
    assert_eq!(d.properties()[0].value, "value");
    let d = Document::parse("[bad](\n#not!\n#");
    assert!(d.links().is_empty() && d.tags().is_empty());
}

#[test]
fn history_is_bounded_and_new_edits_branch() {
    let mut editor = EditorDocument::new("x");
    for _ in 0..(MAX_HISTORY + 10) {
        editor.edit(Span::new(0, 1), "x").unwrap();
    }
    let mut undo_count = 0;
    while editor.undo() {
        undo_count += 1;
    }
    assert_eq!(undo_count, MAX_HISTORY);
    editor.edit(Span::new(0, 1), "y").unwrap();
    assert!(!editor.redo());
    assert_eq!(editor.source(), "y");
}
