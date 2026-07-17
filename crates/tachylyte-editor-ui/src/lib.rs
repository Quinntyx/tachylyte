//! A small, reusable Markdown editor view for GPUI.
//!
//! [`EditorState`] is deliberately independent of GPUI and owns all editing
//! behavior. [`MarkdownEditor`] adapts it to GPUI focus, keyboard and text
//! input. It never reads or writes files: consumers receive [`EditorEvent`]s.

use gpui::{
    div, point, prelude::*, px, rgb, size, AnyElement, App, Bounds, Context, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton, Pixels, Point, Render,
    UTF16Selection, Window,
};
use std::ops::Range;
use tachylyte_markdown::{EditorDocument, Span, ViewMode};

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
        }
    }
    pub fn source(&self) -> &str {
        self.document.source()
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
        self.selection = Selection {
            anchor: cursor,
            head: cursor,
        };
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
            let pos = range.start + text.len();
            self.set_cursor(Cursor(pos));
            self.events.push(EditorEvent::Changed {
                revision: self.document.document().revision,
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
            self.document = EditorDocument::new(source);
            if self.document.source() == self.document.source()
                && self.document.source() != self.source()
            { /* keep revisioned model */
            }
            self.selection = self.normalize(selection);
            self.events.push(EditorEvent::Changed {
                revision: self.document.document().revision,
            });
        }
    }
    pub fn redo(&mut self) {
        if let Some((source, selection)) = self.redo_history.pop() {
            self.history
                .push((self.source().to_owned(), self.selection));
            self.document = EditorDocument::new(source);
            self.selection = self.normalize(selection);
            self.events.push(EditorEvent::Changed {
                revision: self.document.document().revision,
            });
        }
    }
    pub fn request_save(&mut self) {
        self.events.push(EditorEvent::SaveRequested);
    }
    pub fn set_find_query(&mut self, query: impl Into<String>) {
        self.find_query = query.into();
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
    let kind = if text.trim_start().starts_with('#') {
        SyntaxKind::Heading
    } else if text.trim_start().starts_with("<!--") {
        SyntaxKind::Comment
    } else if text.trim_start().starts_with("```") {
        SyntaxKind::Code
    } else {
        SyntaxKind::Plain
    };
    vec![Token {
        span: Span::new(offset, offset + text.len()),
        kind,
    }]
}

/// A GPUI view over [`EditorState`]. Use `cx.new(|cx| MarkdownEditor::new(...))`.
pub struct MarkdownEditor {
    pub state: EditorState,
    focus: FocusHandle,
    scroll: gpui::ScrollHandle,
    pub accessibility_label: String,
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
        Self {
            state: EditorState::new(source),
            focus: cx.focus_handle(),
            scroll: Default::default(),
            accessibility_label: "Markdown editor".into(),
        }
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
        let lines = self.state.projection().into_iter().map(move |line| {
            let mut children = Vec::new();
            let selected = selection.range();
            let line_end = line.span.end;
            for (offset, ch) in line.text.char_indices() {
                let start = line.span.start + offset;
                if start == cursor {
                    children.push(div().text_color(rgb(0x5f6b7aff)).child("│"));
                }
                let end = start + ch.len_utf8();
                let style = if start < selected.end && end > selected.start {
                    rgb(0x355070ff)
                } else {
                    rgb(0x20242bff)
                };
                children.push(div().bg(style).child(ch.to_string()));
            }
            if cursor == line_end {
                children.push(div().text_color(rgb(0x5f6b7aff)).child("│"));
            }
            div()
                .flex()
                .child(format!("{:>4}  ", line.number))
                .children(children)
        });
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
            .child(div().children(lines))
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
        let r = utf16_to_byte_range(self.state.source(), range);
        let line = self.state.source()[..r.start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
        let line_start = self.state.source()[..r.start]
            .rfind('\n')
            .map_or(0, |p| p + 1);
        let column = self.state.source()[line_start..r.start].chars().count();
        Some(Bounds::new(
            point(
                element_bounds.origin.x + px(column as f32 * 8.0),
                element_bounds.origin.y + px(line as f32 * 18.0),
            ),
            size(px(((r.end - r.start).max(1)) as f32 * 8.0), px(18.0)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let line = (f32::from(point.y) / 18.0).max(0.0) as usize;
        let line_start = self
            .state
            .source()
            .split_inclusive('\n')
            .take(line)
            .map(str::len)
            .sum::<usize>();
        let column = (f32::from(point.x) / 8.0).max(0.0) as usize;
        Some(
            line_start
                + self.state.source()[line_start..]
                    .chars()
                    .take(column)
                    .map(char::len_utf8)
                    .sum::<usize>(),
        )
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
}
