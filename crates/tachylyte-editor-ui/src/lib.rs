//! A small, reusable Markdown editor view for GPUI.
//!
//! [`EditorState`] is deliberately independent of GPUI and owns all editing
//! behavior. [`MarkdownEditor`] adapts it to GPUI focus, keyboard and text
//! input. It never reads or writes files: consumers receive [`EditorEvent`]s.

use gpui::{
    div, prelude::*, App, Bounds, Context, EntityInputHandler, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, MouseButton, Pixels, Point, Render, UTF16Selection, Window,
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
    selection: Selection,
    mode: ViewMode,
    scroll_line: usize,
    find_query: String,
    events: Vec<EditorEvent>,
}
impl EditorState {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            document: EditorDocument::new(source),
            selection: Selection::default(),
            mode: ViewMode::Source,
            scroll_line: 0,
            find_query: String::new(),
            events: Vec::new(),
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
        self.document.is_dirty()
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
    pub fn set_mode(&mut self, mode: ViewMode) {
        if self.mode != mode {
            self.mode = mode;
            self.events.push(EditorEvent::ModeChanged(mode));
        }
    }
    pub fn mark_clean(&mut self) {
        self.document.mark_clean();
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
        if self.document.undo() {
            self.set_cursor(Cursor(self.cursor().0.min(self.source().len())));
            self.events.push(EditorEvent::Changed {
                revision: self.document.document().revision,
            });
        }
    }
    pub fn redo(&mut self) {
        if self.document.redo() {
            self.set_cursor(Cursor(self.cursor().0.min(self.source().len())));
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
        let lines = self
            .state
            .projection()
            .into_iter()
            .map(|l| format!("{:>4}  {}", l.number, l.text));
        div()
            .id("markdown-editor")
            .track_focus(&focus)
            .on_key_down(move |event, window, cx| {
                key_entity.update(cx, |editor, _| editor.key_down(event));
                mouse_focus.focus(window);
            })
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                focus.focus(window);
            })
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .child(div().children(lines))
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
        None
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if let Some(r) = range {
            self.state.set_selection(Selection {
                anchor: Cursor(utf16_to_byte(self.state.source(), r.start)),
                head: Cursor(utf16_to_byte(self.state.source(), r.end)),
            })
        }
        self.state.insert_text(text)
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        new_selected: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, text, w, cx);
        if let Some(r) = new_selected {
            let a = utf16_to_byte(self.state.source(), r.start);
            self.state.set_cursor(Cursor(a));
        }
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.state.cursor().0)
    }
}
fn utf16_to_byte(s: &str, n: usize) -> usize {
    s.char_indices()
        .scan(0, |u, (i, c)| {
            let out = (*u, i);
            *u += c.len_utf16();
            Some(out)
        })
        .find(|(u, _)| *u >= n)
        .map_or(s.len(), |(_, i)| i)
}
fn byte_to_utf16(s: &str, n: usize) -> usize {
    s[..n.min(s.len())].encode_utf16().count()
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
}
