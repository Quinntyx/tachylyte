//! A small, reusable Markdown editor view for GPUI.
//!
//! [`EditorState`] is deliberately independent of GPUI and owns all editing
//! behavior. [`MarkdownEditor`] adapts it to GPUI focus, keyboard and text
//! input. It never reads or writes files: consumers receive [`EditorEvent`]s.

use gpui::{
    div, point, prelude::*, px, rgb, size, AnyElement, App, Bounds, Context, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, FontWeight,
    GlobalElementId, InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton, Pixels,
    Point, Render, UTF16Selection, Window,
};
use std::ops::Range;
use tachylyte_markdown::{EditorDocument, Span, ViewMode};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub mod editor_intents;
pub mod editor_options;
pub mod render_surface;
pub mod search_bar;

use editor_intents::EditorIntent;
use editor_options::{EditorOptions, EditorOptionsEvent, LineWrap, MenuAction, ToolbarAction};

/// A UTF-8 byte cursor. All offsets are normalized to character boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cursor(pub usize);
/// A half-open UTF-8 byte selection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub anchor: Cursor,
    pub head: Cursor,
}
impl Selection {
    pub fn range(self) -> Range<usize> {
        let (a, b) = (self.anchor.0, self.head.0);
        a.min(b)..a.max(b)
    }
    pub fn collapsed(self) -> bool {
        self.anchor == self.head
    }
}

/// A syntax category used by renderers to choose their own theme colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxKind {
    Plain,
    Heading,
    Marker,
    Code,
    Link,
    Emphasis,
    Comment,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub span: Span,
    pub kind: SyntaxKind,
}
/// A line projection suitable for any renderer; it contains no GPUI elements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedLine {
    pub number: usize,
    pub span: Span,
    pub text: String,
    pub tokens: Vec<Token>,
}

/// Search result in source byte offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FindResult {
    pub span: Span,
}
/// Events for persistence and surrounding application state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorEvent {
    Changed { revision: u64 },
    DirtyChanged(bool),
    SaveRequested,
    ModeChanged(ViewMode),
    FindChanged,
    Intent(EditorIntent),
    Options(EditorOptionsEvent),
}

/// The complete editing model. It is safe to use without a window or filesystem.
#[derive(Clone, Debug)]
pub struct EditorState {
    document: EditorDocument,
    clean_source: String,
    selection: Selection,
    mode: ViewMode,
    scroll_line: usize,
    find_query: String,
    events: Vec<EditorEvent>,
    history: Vec<(String, Selection)>,
    redo_history: Vec<(String, Selection)>,
    marked: Option<Range<usize>>,
    find_active: Option<usize>,
    revision: u64,
}
impl EditorState {
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        Self {
            document: EditorDocument::new(source.clone()),
            clean_source: source,
            selection: Selection::default(),
            mode: ViewMode::Source,
            scroll_line: 0,
            find_query: String::new(),
            events: Vec::new(),
            history: Vec::new(),
            redo_history: Vec::new(),
            marked: None,
            find_active: None,
            revision: 0,
        }
    }
    pub fn source(&self) -> &str {
        self.document.source()
    }
    /// Replaces the document contents with externally supplied source.
    ///
    /// This is intended for synchronizing the editor with application state;
    /// unlike an edit, it starts a fresh undo history and keeps the current
    /// clean/dirty baseline unchanged.
    pub fn set_source(&mut self, source: impl Into<String>) {
        let source = source.into();
        if source == self.source() {
            return;
        }
        let was_dirty = self.is_dirty();
        self.revision = self.revision.saturating_add(1);
        self.document = restore_document(source, self.revision);
        self.selection = self.normalize(self.selection);
        self.history.clear();
        self.redo_history.clear();
        self.marked = None;
        self.events.push(EditorEvent::Changed {
            revision: self.revision,
        });
        if was_dirty != self.is_dirty() {
            self.events.push(EditorEvent::DirtyChanged(self.is_dirty()));
        }
    }
    pub fn document(&self) -> &tachylyte_markdown::Document {
        self.document.document()
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn cursor(&self) -> Cursor {
        Cursor(self.selection.head.0)
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn is_dirty(&self) -> bool {
        self.source() != self.clean_source
    }
    pub fn mode(&self) -> ViewMode {
        self.mode
    }
    pub fn scroll_line(&self) -> usize {
        self.scroll_line
    }
    pub fn set_scroll_line(&mut self, line: usize) {
        self.scroll_line = line;
    }
    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = self.normalize(selection);
    }
    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.selection = self.normalize(Selection {
            anchor: cursor,
            head: cursor,
        });
    }
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }
    pub fn set_marked_range(&mut self, range: Option<Range<usize>>) {
        self.marked = range.map(|r| {
            self.normalize(Selection {
                anchor: Cursor(r.start),
                head: Cursor(r.end),
            })
            .range()
        });
    }
    pub fn set_mode(&mut self, mode: ViewMode) {
        if self.mode != mode {
            self.mode = mode;
            self.events.push(EditorEvent::ModeChanged(mode));
        }
    }
    pub fn mark_clean(&mut self) {
        self.clean_source = self.source().to_owned();
    }
    pub fn take_events(&mut self) -> Vec<EditorEvent> {
        std::mem::take(&mut self.events)
    }
    fn normalize(&self, mut s: Selection) -> Selection {
        s.anchor.0 = prev_boundary(self.source(), s.anchor.0);
        s.head.0 = prev_boundary(self.source(), s.head.0);
        s
    }
    fn replace(&mut self, range: Range<usize>, text: &str) {
        let was_dirty = self.is_dirty();
        let before = self.source().to_owned();
        self.history.push((before, self.selection));
        self.redo_history.clear();
        if self
            .document
            .edit(Span::new(range.start, range.end), text)
            .is_ok()
        {
            self.revision = self.revision.saturating_add(1);
            let pos = range.start + text.len();
            self.set_cursor(Cursor(pos));
            self.events.push(EditorEvent::Changed {
                revision: self.revision,
            });
            if was_dirty != self.is_dirty() {
                self.events.push(EditorEvent::DirtyChanged(self.is_dirty()));
            }
        }
    }
    pub fn insert_text(&mut self, text: &str) {
        self.replace(self.selection.range(), text);
    }
    pub fn delete_backward(&mut self) {
        let r = self.selection.range();
        if !self.selection.collapsed() {
            self.replace(r, "")
        } else if r.start > 0 {
            let p = previous_boundary(self.source(), r.start);
            self.replace(p..r.start, "");
        }
    }
    pub fn delete_forward(&mut self) {
        let r = self.selection.range();
        if !self.selection.collapsed() {
            self.replace(r, "")
        } else if r.end < self.source().len() {
            let n = next_boundary(self.source(), r.end);
            self.replace(r.end..n, "");
        }
    }
    pub fn undo(&mut self) {
        if let Some((source, selection)) = self.history.pop() {
            self.redo_history
                .push((self.source().to_owned(), self.selection));
            self.revision = self.revision.saturating_add(1);
            self.document = restore_document(source, self.revision);
            self.selection = self.normalize(selection);
            self.events.push(EditorEvent::Changed {
                revision: self.revision,
            });
        }
    }
    pub fn redo(&mut self) {
        if let Some((source, selection)) = self.redo_history.pop() {
            self.history
                .push((self.source().to_owned(), self.selection));
            self.revision = self.revision.saturating_add(1);
            self.document = restore_document(source, self.revision);
            self.selection = self.normalize(selection);
            self.events.push(EditorEvent::Changed {
                revision: self.revision,
            });
        }
    }
    pub fn request_save(&mut self) {
        self.events.push(EditorEvent::SaveRequested);
    }
    pub fn set_find_query(&mut self, query: impl Into<String>) {
        self.find_query = query.into();
        self.find_active = None;
        self.events.push(EditorEvent::FindChanged);
    }
    pub fn find_query(&self) -> &str {
        &self.find_query
    }
    pub fn find_results(&self) -> Vec<FindResult> {
        if self.find_query.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut at = 0;
        while let Some(i) = self.source()[at..].find(&self.find_query) {
            let s = at + i;
            out.push(FindResult {
                span: Span::new(s, s + self.find_query.len()),
            });
            at = s + self.find_query.len();
            if at >= self.source().len() {
                break;
            }
        }
        out
    }

    /// Selects the next or previous find result without changing source text.
    pub fn move_find(&mut self, backwards: bool) -> Option<FindResult> {
        let results = self.find_results();
        if results.is_empty() {
            self.find_active = None;
            return None;
        }
        self.find_active = Some(match (self.find_active, backwards) {
            (None, true) => results.len() - 1,
            (None, false) => 0,
            (Some(i), true) => i.checked_sub(1).unwrap_or(results.len() - 1),
            (Some(i), false) => (i + 1) % results.len(),
        });
        results.get(self.find_active.unwrap()).copied()
    }

    pub fn find_active_index(&self) -> Option<usize> {
        self.find_active
    }

    pub fn replace_current_find(&mut self, replacement: &str) -> bool {
        let Some(index) = self.find_active else { return false };
        let Some(result) = self.find_results().get(index).copied() else { return false };
        self.replace(result.span.start..result.span.end, replacement);
        true
    }

    pub fn replace_all_find(&mut self, replacement: &str) -> usize {
        let results = self.find_results();
        let count = results.len();
        for result in results.into_iter().rev() {
            self.replace(result.span.start..result.span.end, replacement);
        }
        self.find_active = None;
        count
    }

    /// Emits an intent for the construct at a source offset. The source remains
    /// untouched; applications decide whether and how to execute the action.
    pub fn activate_intent_at(&mut self, offset: usize) -> Option<EditorIntent> {
        let intent = EditorIntent::at(self.source(), offset)?;
        self.events.push(EditorEvent::Intent(intent.clone()));
        Some(intent)
    }
    pub fn replace_marked_text(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let target = range
            .map(|r| utf16_to_byte_range(self.source(), r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.range());
        self.replace(target.clone(), text);
        let start = target.start;
        self.marked = Some(start..start + text.len());
        if let Some(r) = selected {
            let a = utf16_to_byte(self.source(), r.start);
            let b = utf16_to_byte(self.source(), r.end);
            self.set_selection(Selection {
                anchor: Cursor(a),
                head: Cursor(b),
            });
        }
    }
    pub fn unmark_text(&mut self) {
        self.marked = None;
    }
    /// Projects source into lines and lightweight syntax spans.
    pub fn projection(&self) -> Vec<ProjectedLine> {
        self.source()
            .split_inclusive('\n')
            .enumerate()
            .map(|(i, line)| {
                let text = line
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_owned();
                let start = self
                    .source()
                    .split_inclusive('\n')
                    .take(i)
                    .map(str::len)
                    .sum();
                let span = Span::new(start, start + text.len());
                ProjectedLine {
                    number: i + 1,
                    span,
                    text: text.clone(),
                    tokens: tokens(&text, start),
                }
            })
            .collect()
    }
}
fn prev_boundary(s: &str, p: usize) -> usize {
    let mut p = p.min(s.len());
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}
fn previous_boundary(s: &str, p: usize) -> usize {
    let p = prev_boundary(s, p);
    s[..p].char_indices().next_back().map_or(0, |(i, _)| i)
}
fn next_boundary(s: &str, p: usize) -> usize {
    let mut p = prev_boundary(s, p);
    if p < s.len() {
        p += s[p..].chars().next().unwrap().len_utf8();
    }
    p
}
fn tokens(text: &str, offset: usize) -> Vec<Token> {
    let line_kind = if text.trim_start().starts_with('#') {
        Some(SyntaxKind::Heading)
    } else if text.trim_start().starts_with("<!--") {
        Some(SyntaxKind::Comment)
    } else if text.trim_start().starts_with("```") {
        Some(SyntaxKind::Code)
    } else {
        None
    };
    if let Some(kind) = line_kind {
        return vec![Token {
            span: Span::new(offset, offset + text.len()),
            kind,
        }];
    }
    let mut out = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let rest = &text[cursor..];
        let (needle, kind) = if rest.starts_with('`') {
            ("`", SyntaxKind::Code)
        } else if rest.starts_with("**") {
            ("**", SyntaxKind::Emphasis)
        } else if rest.starts_with('*') || rest.starts_with('_') {
            (&rest[..1], SyntaxKind::Emphasis)
        } else if rest.starts_with('[') {
            ("[", SyntaxKind::Link)
        } else {
            cursor += rest.chars().next().unwrap().len_utf8();
            continue;
        };
        let end = text[cursor + needle.len()..]
            .find(match kind {
                SyntaxKind::Code => '`',
                SyntaxKind::Emphasis => {
                    if needle.len() == 2 {
                        '*'
                    } else {
                        needle.chars().next().unwrap()
                    }
                }
                SyntaxKind::Link => ']',
                _ => needle.chars().next().unwrap(),
            })
            .map(|p| cursor + needle.len() + p + 1);
        if let Some(end) = end {
            out.push(Token {
                span: Span::new(offset + cursor, offset + end),
                kind,
            });
            cursor = end;
        } else {
            cursor += needle.len();
        }
    }
    if out.is_empty() {
        out.push(Token {
            span: Span::new(offset, offset + text.len()),
            kind: SyntaxKind::Plain,
        });
    }
    out
}

// Keep the palette local to the editor so it remains usable in applications that do
// not install a global GPUI theme. These are deliberately close to Obsidian's light
// defaults: the violet is an accent, never the page background.
const PAPER: u32 = 0xffff_ffff;
const PANEL: u32 = 0xf6f6_f6ff;
const INK: u32 = 0x2222_22ff;
const MUTED: u32 = 0x5c5c_5cff;
const BORDER: u32 = 0xe0e0_e0ff;
const ACCENT: u32 = 0x7852_eeff;
// Keep secondary surfaces neutral; the accent is reserved for semantic text,
// active controls, and the caret rather than becoming a saturated background.
const CODE_BG: u32 = PANEL;
const SELECTION: u32 = BORDER;

fn mode_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Source => "Source",
        ViewMode::LivePreview => "Live preview",
        ViewMode::Reading => "Reading",
    }
}

fn inline_element(inline: &tachylyte_markdown::Inline) -> AnyElement {
    use tachylyte_markdown::Inline;
    match inline {
        Inline::Text { value, .. } => div()
            .text_color(rgb(INK))
            .child(value.clone())
            .into_any_element(),
        Inline::Emphasis { children, .. } | Inline::Highlight { children, .. } => div()
            .italic()
            .text_color(rgb(INK))
            .children(children.iter().map(inline_element))
            .into_any_element(),
        Inline::Strong { children, .. } => div()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(INK))
            .children(children.iter().map(inline_element))
            .into_any_element(),
        Inline::Code { value, .. } => div()
            .bg(rgb(CODE_BG))
            .text_color(rgb(ACCENT))
            .child(format!(" {} ", value))
            .into_any_element(),
        Inline::Link { label, .. } => div()
            .text_color(rgb(ACCENT))
            .underline()
            .child(label.clone())
            .into_any_element(),
        Inline::WikiLink {
            target,
            alias,
            block,
            ..
        } => div()
            .text_color(rgb(ACCENT))
            .underline()
            .child(match (alias, block) {
                (Some(alias), Some(block)) => format!("{} ↗ #^{}", alias, block),
                (Some(alias), None) => alias.clone(),
                (None, Some(block)) => format!("{} ↗ #^{}", target, block),
                (None, None) => target.clone(),
            })
            .into_any_element(),
        Inline::Tag { value, .. } => div()
            .text_color(rgb(ACCENT))
            .child(value.clone())
            .into_any_element(),
        Inline::Math { value, .. } => div()
            .bg(rgb(PANEL))
            .text_color(rgb(ACCENT))
            .child(format!("∑ {} [math placeholder]", value))
            .into_any_element(),
        Inline::FootnoteRef { label, .. } => div()
            .text_color(rgb(ACCENT))
            .child(format!("[{}]", label))
            .into_any_element(),
        Inline::Embed { target, alias, .. } => div()
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded(px(4.0))
            .px(px(5.0))
            .text_color(rgb(MUTED))
            .child(format!(
                "▧ {} [embed placeholder]",
                alias.clone().unwrap_or_else(|| target.clone())
            ))
            .into_any_element(),
    }
}

fn block_element(block: &tachylyte_markdown::Block) -> AnyElement {
    use tachylyte_markdown::Block;
    match block {
        Block::Heading {
            level, children, ..
        } => div()
            .text_color(rgb(INK))
            .font_weight(FontWeight::BOLD)
            .text_size(px(match level {
                1 => 28.0,
                2 => 23.0,
                3 => 20.0,
                _ => 17.0,
            }))
            .mb(px(10.0))
            .children(children.iter().map(inline_element))
            .into_any_element(),
        Block::Paragraph { children, .. } => div()
            .text_color(rgb(INK))
            .text_size(px(16.0))
            .mb(px(12.0))
            .children(children.iter().map(inline_element))
            .into_any_element(),
        Block::Code {
            language, value, ..
        } => div()
            .w_full()
            .bg(rgb(CODE_BG))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded(px(5.0))
            .p(px(12.0))
            .mb(px(12.0))
            .text_color(rgb(INK))
            .child(if matches!(
                language.as_deref(),
                Some("mermaid") | Some("diagram") | Some("plantuml")
            ) {
                format!(
                    "◇ {} diagram placeholder\n{}",
                    language.as_deref().unwrap_or("diagram"),
                    value
                )
            } else {
                format!(
                    "{}\n{}",
                    language.as_deref().unwrap_or("code"),
                    value
                )
            })
            .into_any_element(),
        Block::Quote { children, .. } => div()
            .border_l_2()
            .border_color(rgb(ACCENT))
            .pl(px(14.0))
            .mb(px(12.0))
            .children(children.iter().map(block_element))
            .into_any_element(),
        Block::Callout {
            kind,
            title,
            children,
            ..
        } => div()
            .w_full()
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded(px(6.0))
            .p(px(12.0))
            .mb(px(12.0))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ACCENT))
                    .child(title.clone().unwrap_or_else(|| kind.clone())),
            )
            .children(children.iter().map(block_element))
            .into_any_element(),
        Block::List { ordered, items, .. } => div()
            .mb(px(10.0))
            .children(items.iter().enumerate().map(|(index, item)| {
                let marker = if *ordered {
                    format!("{}. ", index + 1)
                } else if item.checked == Some(true) {
                    "☑ ".into()
                } else if item.checked == Some(false) {
                    "☐ ".into()
                } else {
                    "• ".into()
                };
                div()
                    .flex()
                    .child(div().text_color(rgb(MUTED)).child(marker))
                    .children(item.blocks.iter().map(block_element))
                    .into_any_element()
            }))
            .into_any_element(),
        Block::ThematicBreak { .. } => div()
            .w_full()
            .h(px(1.0))
            .bg(rgb(BORDER))
            .my(px(10.0))
            .into_any_element(),
        Block::Html { value, .. } | Block::Comment { value, .. } => div()
            .text_color(rgb(MUTED))
            .child(value.clone())
            .into_any_element(),
        Block::Table { headers, rows, .. } => div()
            .border_1()
            .border_color(rgb(BORDER))
            .p(px(8.0))
            .mb(px(12.0))
            .child(headers.join("  |  "))
            .children(rows.iter().map(|row| {
                div()
                    .text_color(rgb(INK))
                    .child(row.join("  |  "))
                    .into_any_element()
            }))
            .into_any_element(),
    }
}

/// A GPUI view over [`EditorState`]. Use `cx.new(|cx| MarkdownEditor::new(...))`.
pub struct MarkdownEditor {
    pub state: EditorState,
    /// Presentation-only controls; these never alter the Markdown source.
    pub options: EditorOptions,
    focus: FocusHandle,
    scroll: gpui::ScrollHandle,
    pub accessibility_label: String,
    last_element_bounds: Bounds<Pixels>,
}

/// Adds the GPUI 0.2.2 input protocol to an element during its paint phase.
/// GPUI 0.2.2 exposes this as `Window::handle_input` rather than a built-in
/// fluent method; this extension keeps the call site component-friendly.
pub trait InputHandlerElementExt {
    fn input_handler(self, focus: &FocusHandle, view: Entity<MarkdownEditor>) -> InputElement;
}
pub struct InputElement {
    child: AnyElement,
    focus: FocusHandle,
    view: Entity<MarkdownEditor>,
}
impl InputHandlerElementExt for gpui::Div {
    fn input_handler(self, focus: &FocusHandle, view: Entity<MarkdownEditor>) -> InputElement {
        InputElement {
            child: self.into_any_element(),
            focus: focus.clone(),
            view,
        }
    }
}
impl InputHandlerElementExt for gpui::Stateful<gpui::Div> {
    fn input_handler(self, focus: &FocusHandle, view: Entity<MarkdownEditor>) -> InputElement {
        InputElement {
            child: self.into_any_element(),
            focus: focus.clone(),
            view,
        }
    }
}
impl IntoElement for InputElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for InputElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Registration is the authoritative per-frame geometry source. In
        // particular, this replaces the previous frame before IME hit testing.
        self.view
            .update(cx, |editor, _| editor.last_element_bounds = bounds);
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        self.child.paint(window, cx);
    }
}
impl MarkdownEditor {
    pub fn new(source: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self::from_state(EditorState::new(source), cx)
    }

    /// Creates an editor view around an existing model.
    pub fn from_state(state: EditorState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            options: EditorOptions::default(),
            focus: cx.focus_handle(),
            scroll: Default::default(),
            accessibility_label: "Markdown editor".into(),
            last_element_bounds: Bounds::default(),
        }
    }

    pub fn state(&self) -> &EditorState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }

    /// Replaces the model while retaining the view's focus and scroll state.
    pub fn replace_state(&mut self, state: EditorState) {
        self.state = state;
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.state.set_source(source);
    }

    /// Changes the presentation mode and emits `EditorEvent::ModeChanged`.
    pub fn set_mode(&mut self, mode: ViewMode) {
        self.state.set_mode(mode);
    }

    pub fn source(&self) -> &str {
        self.state.source()
    }

    pub fn mode(&self) -> ViewMode {
        self.state.mode()
    }

    pub fn take_events(&mut self) -> Vec<EditorEvent> {
        self.state.take_events()
    }

    pub fn take_option_events(&mut self) -> Vec<EditorOptionsEvent> {
        self.options.take_events()
    }

    pub fn toolbar_action(&mut self, action: ToolbarAction) {
        self.options.toolbar(action);
    }

    pub fn menu_action(&mut self, action: MenuAction) {
        self.options.menu(action);
    }
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }
    pub fn scroll_handle(&self) -> gpui::ScrollHandle {
        self.scroll.clone()
    }
}
impl Focusable for MarkdownEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for MarkdownEditor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let entity = _cx.entity();
        let key_entity = entity.clone();
        let mouse_focus = focus.clone();
        let click_focus = focus.clone();
        let selection = self.state.selection();
        let cursor = self.state.cursor().0;
        let mode = self.state.mode();
        let line_wrap = self.options.line_wrap() == LineWrap::Wrap;
        let show_line_numbers = self.options.show_line_numbers();
        let source_lines = self.state.projection().into_iter().map(move |line| {
            let line_number = line.number;
            let mut children = Vec::new();
            let selected = selection.range();
            let line_end = line.span.end;
            for (offset, grapheme) in line.text.grapheme_indices(true) {
                let start = line.span.start + offset;
                if start == cursor {
                    children.push(div().text_color(rgb(ACCENT)).child("│").into_any_element());
                }
                let end = start + grapheme.len();
                let is_selected = start < selected.end && end > selected.start;
                let syntax = line
                    .tokens
                    .iter()
                    .find(|token| start < token.span.end && end > token.span.start)
                    .map_or(SyntaxKind::Plain, |token| token.kind);
                let style = if is_selected {
                    SELECTION
                } else if mode == ViewMode::LivePreview
                    && matches!(
                        syntax,
                        SyntaxKind::Heading | SyntaxKind::Link | SyntaxKind::Code
                    )
                {
                    ACCENT
                } else {
                    INK
                };
                let mut part = div()
                    .text_color(rgb(style))
                    .when(is_selected, |part| part.bg(rgb(SELECTION)))
                    .child(grapheme.to_owned());
                if mode == ViewMode::LivePreview
                    && (grapheme == "*" || grapheme == "_" || grapheme == "`")
                {
                    part = part.text_color(rgb(MUTED));
                }
                if mode == ViewMode::LivePreview && syntax == SyntaxKind::Emphasis {
                    part = part.italic();
                } else if mode == ViewMode::LivePreview && syntax == SyntaxKind::Link {
                    part = part.underline();
                }
                children.push(part.into_any_element());
            }
            if cursor == line_end {
                children.push(div().text_color(rgb(ACCENT)).child("│").into_any_element());
            }
            div()
                .flex()
                .when(line_wrap, |line| line.max_w(px(760.0)))
                .text_size(px(15.0))
                .when(show_line_numbers, |line| line.child(
                    div()
                        .w(px(42.0))
                        .text_color(rgb(MUTED))
                        .child(format!("{:>3}", line_number)),
                ))
                .children(children)
                .into_any_element()
        });
        // Build the reading projection from the source that is current for this
        // render.  Keeping an owned document here also prevents a deferred GPUI
        // child iterator from retaining a projection from an earlier frame.
        let reading_document = (mode != ViewMode::Source)
            .then(|| restore_document(self.state.source().to_owned(), self.state.revision()));
        let reading_children = reading_document.as_ref().map(|document| {
            let mut children = Vec::new();
            if let Some(frontmatter) = &document.document().frontmatter {
                children.push(
                    div()
                        .w_full()
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded(px(6.0))
                        .p(px(10.0))
                        .mb(px(14.0))
                        .child(div().text_color(rgb(MUTED)).child("Properties"))
                        .children(frontmatter.properties.iter().map(|property| {
                            div()
                                .flex()
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                                    "{}: ",
                                    property.key
                                )))
                                .child(property.value.clone())
                                .into_any_element()
                        }))
                        .into_any_element(),
                );
            }
            children.extend(document.document().blocks.iter().map(block_element));
            if !document.document().footnotes.is_empty() {
                children.push(
                    div()
                        .mt(px(18.0))
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .pt(px(10.0))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Footnotes"))
                        .children(document.document().footnotes.iter().map(|footnote| {
                            div()
                                .text_color(rgb(MUTED))
                                .child(format!("[{}] {}", footnote.label, footnote.definition))
                                .into_any_element()
                        }))
                        .into_any_element(),
                );
            }
            children
        });
        let toolbar_entity = entity.clone();
        let toolbar_focus = focus.clone();
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .p(px(8.0))
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .children(
                [ViewMode::Source, ViewMode::LivePreview, ViewMode::Reading]
                    .into_iter()
                    .map(move |button_mode| {
                        let button_entity = toolbar_entity.clone();
                        let button_focus = toolbar_focus.clone();
                        let active = mode == button_mode;
                        div()
                            .id(ElementId::Name(
                                format!("mode-{}", mode_name(button_mode)).into(),
                            ))
                            .cursor_pointer()
                            .px(px(10.0))
                            .py(px(5.0))
                            .rounded(px(4.0))
                            .bg(rgb(if active { ACCENT } else { PANEL }))
                            .text_color(rgb(if active { PAPER } else { MUTED }))
                            .child(mode_name(button_mode))
                            .on_click(move |_, window, cx| {
                                button_focus.focus(window);
                                button_entity
                                    .update(cx, |editor, _| editor.state.set_mode(button_mode));
                            })
                            .into_any_element()
                    }),
            );
        let option_entity = entity.clone();
        let option_focus = focus.clone();
        let option_button = move |label: String, action: ToolbarAction| {
            let option_entity = option_entity.clone();
            let option_focus = option_focus.clone();
            div()
                .id(ElementId::Name(format!("option-{label}").into()))
                .cursor_pointer()
                .px(px(7.0))
                .py(px(5.0))
                .rounded(px(4.0))
                .text_color(rgb(MUTED))
                .child(label)
                .on_click(move |_, window, cx| {
                    option_focus.focus(window);
                    option_entity.update(cx, |editor, _| editor.options.toolbar(action));
                })
                .into_any_element()
        };
        let find_entity = entity.clone();
        let find_focus = focus.clone();
        let find_bar = div()
            .flex()
            .items_center()
            .gap(px(3.0))
            .ml(px(8.0))
            .text_color(rgb(MUTED))
            .child(format!(
                "Find: {} ({})",
                if self.state.find_query().is_empty() {
                    "—"
                } else {
                    self.state.find_query()
                },
                self.state.find_results().len()
            ))
            .child(
                div()
                    .id("find-previous")
                    .cursor_pointer()
                    .child("‹")
                    .on_click({
                        let find_entity = find_entity.clone();
                        let find_focus = find_focus.clone();
                        move |_, window, cx| {
                            find_focus.focus(window);
                            find_entity.update(cx, |editor, _| {
                                editor.state.move_find(true);
                            });
                        }
                    }),
            )
            .child(
                div()
                    .id("find-next")
                    .cursor_pointer()
                    .child("›")
                    .on_click({
                        let find_entity = find_entity.clone();
                        let find_focus = find_focus.clone();
                        move |_, window, cx| {
                            find_focus.focus(window);
                            find_entity.update(cx, |editor, _| {
                                editor.state.move_find(false);
                            });
                        }
                    }),
            )
            .child("Replace: use source-safe API");
        let option_controls = div()
            .flex()
            .items_center()
            .children([
                option_button(
                    format!("Wrap: {:?}", self.options.line_wrap()),
                    ToolbarAction::ToggleLineWrap,
                ),
                option_button(
                    format!(
                        "Gutter: {}",
                        if self.options.show_line_numbers() { "on" } else { "off" }
                    ),
                    ToolbarAction::ToggleLineNumbers,
                ),
                option_button("Spellcheck: unavailable".into(), ToolbarAction::Spellcheck),
                option_button("Vim: unavailable".into(), ToolbarAction::VimMode),
            ])
            .child(find_bar);
        let toolbar = toolbar.child(option_controls);
        let reading_entity = entity.clone();
        let content = if mode != ViewMode::Source {
            div()
                .id("reading-content")
                .w_full()
                .max_w(px(820.0))
                .mx_auto()
                .p(px(32.0))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    // Reading-mode surfaces are intentionally non-editing. A click
                    // asks the model for an intent at the current source cursor;
                    // the host application can then open links or toggle tasks.
                    reading_entity.update(cx, |editor, _| {
                        editor.state.activate_intent_at(cursor);
                    });
                })
                .children(reading_children.expect("reading children are built for reading mode"))
                .into_any_element()
        } else {
            div().p(px(14.0)).children(source_lines).into_any_element()
        };
        div()
            .id("markdown-editor")
            .track_focus(&focus)
            .on_key_down(move |event, window, cx| {
                key_entity.update(cx, |editor, _| editor.key_down(event));
                mouse_focus.focus(window);
            })
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                click_focus.focus(window);
            })
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .bg(rgb(PAPER))
            .text_color(rgb(INK))
            .child(toolbar)
            .child(content)
            .input_handler(&focus, entity)
    }
}
impl MarkdownEditor {
    fn key_down(&mut self, event: &KeyDownEvent) {
        let k = event.keystroke.key.as_str();
        let m = &event.keystroke.modifiers;
        if (m.control || m.platform) && k.eq_ignore_ascii_case("z") {
            if m.shift {
                self.state.redo()
            } else {
                self.state.undo()
            }
        } else if (m.control || m.platform) && k.eq_ignore_ascii_case("y") {
            self.state.redo()
        } else if (m.control || m.platform) && k.eq_ignore_ascii_case("s") {
            self.state.request_save()
        } else if k == "backspace" {
            self.state.delete_backward()
        } else if k == "delete" {
            self.state.delete_forward()
        } else if k == "left" {
            let p = previous_boundary(self.state.source(), self.state.cursor().0);
            self.state.set_cursor(Cursor(p))
        } else if k == "right" {
            let p = next_boundary(self.state.source(), self.state.cursor().0);
            self.state.set_cursor(Cursor(p))
        } else if k == "home" {
            self.state.set_cursor(Cursor(
                self.state.source()[..self.state.cursor().0]
                    .rfind('\n')
                    .map_or(0, |p| p + 1),
            ))
        } else if k == "end" {
            let p = self.state.source()[self.state.cursor().0..]
                .find('\n')
                .map_or(self.state.source().len(), |p| self.state.cursor().0 + p);
            self.state.set_cursor(Cursor(p))
        } else if k == "enter" {
            self.state.insert_text("\n")
        } else if let Some(c) = &event.keystroke.key_char {
            if !m.control && !m.platform && !m.alt {
                self.state.insert_text(c)
            }
        }
    }
}
impl EntityInputHandler for MarkdownEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = utf16_to_byte_range(self.state.source(), range);
        *adjusted = Some(byte_to_utf16_range(self.state.source(), r.clone()));
        Some(self.state.source().get(r)?.into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let r = self.state.selection();
        Some(UTF16Selection {
            range: byte_to_utf16_range(self.state.source(), r.range()),
            reversed: r.head.0 < r.anchor.0,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.state
            .marked_range()
            .map(|r| byte_to_utf16_range(self.state.source(), r))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.state.unmark_text();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.state.replace_marked_text(range, text, None)
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        new_selected: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = (w, cx);
        self.state.replace_marked_text(range, text, new_selected)
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds_for_range_geometry(
            self.state.source(),
            range,
            element_bounds,
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(character_index_for_point_geometry(
            self.state.source(),
            point,
            self.last_element_bounds,
        ))
    }
}
fn utf16_to_byte(s: &str, n: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if n <= units {
            return i;
        }
        units += c.len_utf16();
        if n <= units {
            return i + c.len_utf8();
        }
    }
    s.len()
}
fn byte_to_utf16(s: &str, n: usize) -> usize {
    let end = prev_boundary(s, n);
    s[..end].encode_utf16().count()
}
fn display_columns(s: &str) -> usize {
    s.graphemes(true).map(UnicodeWidthStr::width).sum()
}
/// Return the UTF-16 character index at a point in an explicitly supplied
/// element rectangle. Horizontal overflow is clamped to that line's end.
pub fn character_index_for_point_geometry(
    source: &str,
    point: Point<Pixels>,
    bounds: Bounds<Pixels>,
) -> usize {
    if source.is_empty() {
        return 0;
    }
    let x =
        (f32::from(point.x) - f32::from(bounds.origin.x)).clamp(0.0, f32::from(bounds.size.width));
    let y = (f32::from(point.y) - f32::from(bounds.origin.y))
        .clamp(0.0, f32::from(bounds.size.height.max(px(18.0))));
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let line = ((y / 18.0) as usize).min(lines.len().saturating_sub(1));
    let text = lines[line].trim_end_matches(['\n', '\r']);
    let target = (x / 8.0) as usize;
    let mut column = 0;
    let mut byte = 0;
    for grapheme in text.graphemes(true) {
        let width = UnicodeWidthStr::width(grapheme);
        if target < column + width.max(1) {
            break;
        }
        column += width;
        byte += grapheme.len();
    }
    let line_start: usize = lines.iter().take(line).map(|line| line.len()).sum();
    byte_to_utf16(source, line_start + byte)
}
/// Return the caret/selection rectangle using the same grapheme display
/// columns as [`character_index_for_point_geometry`].
pub fn bounds_for_range_geometry(
    source: &str,
    range: Range<usize>,
    bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    let r = utf16_to_byte_range(source, range);
    let line = source[..r.start].bytes().filter(|b| *b == b'\n').count();
    let line_start = source[..r.start].rfind('\n').map_or(0, |p| p + 1);
    let column = display_columns(&source[line_start..r.start]);
    let width = display_columns(&source[r.start..r.end]).max(1);
    Bounds::new(
        point(
            bounds.origin.x + px(column as f32 * 8.0),
            bounds.origin.y + px(line as f32 * 18.0),
        ),
        size(px(width as f32 * 8.0), px(18.0)),
    )
}
fn restore_document(source: String, revision: u64) -> EditorDocument {
    if revision == 0 {
        return EditorDocument::new(source);
    }
    let mut document = EditorDocument::new(String::new());
    let _ = document.edit(Span::new(0, 0), &source);
    for _ in 1..revision {
        let end = document.source().len();
        let _ = document.edit(Span::new(end, end), "");
    }
    document
}
fn utf16_to_byte_range(s: &str, r: Range<usize>) -> Range<usize> {
    utf16_to_byte(s, r.start)..utf16_to_byte(s, r.end)
}
fn byte_to_utf16_range(s: &str, r: Range<usize>) -> Range<usize> {
    byte_to_utf16(s, r.start)..byte_to_utf16(s, r.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_editing() {
        let mut e = EditorState::new("a🙂c");
        e.set_cursor(Cursor(5));
        e.delete_backward();
        assert_eq!(e.source(), "ac");
    }
    #[test]
    fn selection_and_undo() {
        let mut e = EditorState::new("hello");
        e.set_selection(Selection {
            anchor: Cursor(0),
            head: Cursor(5),
        });
        e.insert_text("hi");
        assert_eq!(e.source(), "hi");
        e.undo();
        assert_eq!(e.source(), "hello");
    }
    #[test]
    fn modes_find_projection() {
        let mut e = EditorState::new("# A\nA");
        e.set_mode(ViewMode::Reading);
        e.set_find_query("A");
        assert_eq!(e.find_results().len(), 2);
        assert_eq!(e.projection().len(), 2);
        assert_eq!(e.mode(), ViewMode::Reading);
    }

    #[test]
    fn find_navigation_and_intents_are_non_destructive() {
        let mut e = EditorState::new("[[Page]] and #tag");
        e.set_find_query("Page");
        assert_eq!(
            e.move_find(false).map(|result| result.span),
            Some(Span::new(2, 6))
        );
        assert_eq!(e.source(), "[[Page]] and #tag");
        assert!(e.activate_intent_at(3).is_some());
        assert!(e
            .take_events()
            .iter()
            .any(|event| matches!(event, EditorEvent::Intent(_))));
    }
    #[test]
    fn astral_utf16_ranges_are_character_safe() {
        let source = "a🙂b";
        assert_eq!(utf16_to_byte(source, 1), 1);
        assert_eq!(utf16_to_byte(source, 3), 5);
        assert_eq!(byte_to_utf16(source, 5), 3);
        assert_eq!(utf16_to_byte_range(source, 1..3), 1..5);
    }
    #[test]
    fn ime_composition_tracks_and_unmarks() {
        let mut e = EditorState::new("ab");
        e.set_cursor(Cursor(1));
        e.replace_marked_text(None, "é", Some(1..2));
        assert_eq!(e.source(), "aéb");
        assert_eq!(e.marked_range(), Some(1..3));
        assert_eq!(
            e.selection(),
            Selection {
                anchor: Cursor(1),
                head: Cursor(3)
            }
        );
        e.unmark_text();
        assert_eq!(e.marked_range(), None);
    }
    #[test]
    fn undo_restores_selection_and_projection_spans() {
        let mut e = EditorState::new("one\ntwo");
        e.set_selection(Selection {
            anchor: Cursor(1),
            head: Cursor(3),
        });
        e.insert_text("X");
        e.undo();
        assert_eq!(
            e.selection(),
            Selection {
                anchor: Cursor(1),
                head: Cursor(3)
            }
        );
        let lines = e.projection();
        assert_eq!(lines[0].span, Span::new(0, 3));
        assert_eq!(lines[1].span, Span::new(4, 7));
    }
    #[test]
    fn cursor_is_clamped_to_utf8_boundaries() {
        let mut e = EditorState::new("a🙂c");
        e.set_cursor(Cursor(999));
        assert_eq!(e.cursor(), Cursor(6));
        e.set_cursor(Cursor(3));
        assert_eq!(e.cursor(), Cursor(1));
    }
    #[test]
    fn astral_geometry_uses_display_columns_and_utf16() {
        let source = "a🙂b";
        assert_eq!(display_columns(&source[..5]), 3);
        assert_eq!(byte_to_utf16(source, 5), 3);
        let origin = (40.0_f32, 24.0_f32);
        let point = (origin.0 + 2.0 * 8.0 + 1.0, origin.1 + 18.0 + 1.0);
        assert_eq!(((point.0 - origin.0) / 8.0) as usize, 2);
        assert_eq!(((point.1 - origin.1) / 18.0) as usize, 1);
    }
    #[test]
    fn edits_and_undo_have_monotonic_revisions() {
        let mut e = EditorState::new("a");
        let start = e.revision();
        e.insert_text("b");
        let edited = e.revision();
        e.undo();
        let undone = e.revision();
        assert_eq!(e.document().revision, undone);
        e.redo();
        assert_eq!(e.document().revision, e.revision());
        assert!(start < edited && edited < undone && undone < e.revision());
    }
    #[test]
    fn geometry_roundtrips_nonzero_origin_and_replaces_stale_frame() {
        let old = Bounds::new(point(px(10.0), px(20.0)), size(px(160.0), px(40.0)));
        let current = Bounds::new(point(px(100.0), px(200.0)), size(px(160.0), px(40.0)));
        let caret = bounds_for_range_geometry("a🙂b", 1..3, current);
        assert_eq!(
            character_index_for_point_geometry(
                "a🙂b",
                point(caret.origin.x + px(1.0), caret.origin.y + px(1.0)),
                current
            ),
            1
        );
        assert_eq!(
            character_index_for_point_geometry(
                "a🙂b",
                point(caret.origin.x + px(17.0), caret.origin.y + px(1.0)),
                current
            ),
            3
        );
        assert_eq!(
            character_index_for_point_geometry(
                "a🙂b",
                point(old.origin.x + px(1.0), old.origin.y + px(1.0)),
                current
            ),
            0
        );
    }
    #[test]
    fn geometry_clamps_multiline_overflow_and_handles_widths() {
        let bounds = Bounds::new(point(px(30.0), px(50.0)), size(px(80.0), px(60.0)));
        let source = "a\n界🙂e\ne\u{301}";
        assert_eq!(
            character_index_for_point_geometry(source, point(px(500.0), px(51.0)), bounds),
            1
        );
        assert_eq!(
            character_index_for_point_geometry(source, point(px(30.0 + 17.0), px(69.0)), bounds),
            3
        );
        assert_eq!(display_columns("界🙂e\u{301}"), 5);
        assert_eq!(
            bounds_for_range_geometry(source, 6..7, bounds).size.width,
            px(8.0)
        );
    }
}
