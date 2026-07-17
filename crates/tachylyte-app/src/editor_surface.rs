//! App-local mounting adapter for the native Markdown editor surface.

use gpui::{AppContext, Context, Entity};
use tachylyte_editor_ui::{EditorEvent, MarkdownEditor};
use tachylyte_markdown::ViewMode;

/// Owns a mounted [`MarkdownEditor`] and exposes the lifecycle operations
/// needed by an application shell.
#[derive(Clone, Debug)]
pub struct EditorSurface {
    editor: Entity<MarkdownEditor>,
}

impl EditorSurface {
    /// Mount an editor initialized from source and presentation mode.
    pub fn mount<T>(source: impl Into<String>, mode: ViewMode, cx: &mut Context<T>) -> Self {
        let source = source.into();
        let editor = cx.new(|cx| {
            let mut editor = MarkdownEditor::new(source, cx);
            editor.set_mode(mode);
            editor
        });
        Self { editor }
    }

    /// Return the mounted editor entity for placement in a GPUI layout.
    pub fn entity(&self) -> Entity<MarkdownEditor> {
        self.editor.clone()
    }

    /// Synchronize externally owned source and presentation mode.
    pub fn sync<T>(&self, source: impl Into<String>, mode: ViewMode, cx: &mut Context<T>) {
        let source = source.into();
        self.editor.update(cx, |editor, cx| {
            editor.set_source(source);
            editor.set_mode(mode);
            cx.notify();
        });
    }

    /// Drain editor events emitted since the previous call.
    pub fn drain_events<T>(&self, cx: &mut Context<T>) -> Vec<EditorEvent> {
        self.editor.update(cx, |editor, _| editor.take_events())
    }
}
