use tachylyte_markdown::{Document, EditorDocument, Span, ViewMode};

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
