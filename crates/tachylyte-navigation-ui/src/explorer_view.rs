//! Compact, filesystem-free explorer surface.

use crate::{ExplorerIntent, ExplorerModel, ExplorerNodeKind};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};

#[derive(Clone, Debug)]
pub struct FileExplorerView {
    pub model: ExplorerModel,
    pub intents: Vec<ExplorerIntent>,
    pub context_path: Option<String>,
    pub toolbar_visible: bool,
}

impl FileExplorerView {
    pub fn new(model: ExplorerModel) -> Self {
        Self {
            model,
            intents: Vec::new(),
            context_path: None,
            toolbar_visible: true,
        }
    }

    pub fn take_intents(&mut self) -> Vec<ExplorerIntent> {
        std::mem::take(&mut self.intents)
    }
    pub fn model(&self) -> &ExplorerModel {
        &self.model
    }
    pub fn model_mut(&mut self) -> &mut ExplorerModel {
        &mut self.model
    }
    pub fn dispatch(&mut self, intent: ExplorerIntent) {
        self.intents.push(intent);
    }

    pub fn click(&mut self, path: impl Into<String>) {
        self.select(path);
    }
    pub fn select(&mut self, path: impl Into<String>) {
        self.dispatch(ExplorerIntent::Select { path: path.into() });
    }
    pub fn activate(&mut self, path: impl Into<String>) {
        self.dispatch(ExplorerIntent::Open { path: path.into() });
    }
    pub fn toggle(&mut self, path: impl Into<String>) {
        let path = path.into();
        let expanded = !self.model.expanded.contains(&path);
        self.dispatch(ExplorerIntent::Toggle { path, expanded });
    }
    pub fn new_note(&mut self) {
        self.dispatch(ExplorerIntent::NewNote {
            parent: self.context_path.clone(),
        });
    }
    pub fn new_folder(&mut self) {
        self.dispatch(ExplorerIntent::NewFolder {
            parent: self.context_path.clone(),
        });
    }
    pub fn rename(&mut self, path: impl Into<String>, new_name: impl Into<String>) {
        self.dispatch(ExplorerIntent::Rename {
            path: path.into(),
            new_name: new_name.into(),
        });
    }
    pub fn delete(&mut self, path: impl Into<String>) {
        self.dispatch(ExplorerIntent::Delete { path: path.into() });
    }
    pub fn move_to(&mut self, path: impl Into<String>, destination: impl Into<String>) {
        self.dispatch(ExplorerIntent::Move {
            path: path.into(),
            destination: destination.into(),
        });
    }
    pub fn duplicate(&mut self, path: impl Into<String>, destination: impl Into<String>) {
        self.dispatch(ExplorerIntent::Duplicate {
            path: path.into(),
            destination: destination.into(),
        });
    }
    pub fn reveal(&mut self, path: impl Into<String>) {
        self.dispatch(ExplorerIntent::Reveal { path: path.into() });
    }
    pub fn drag_move(&mut self, source: impl Into<String>, destination: impl Into<String>) {
        self.dispatch(ExplorerIntent::DragMove {
            source: source.into(),
            destination: destination.into(),
        });
    }
    pub fn context_menu(&mut self, path: Option<String>) {
        self.dispatch(ExplorerIntent::ContextMenu { path });
    }

    pub fn keyboard(&mut self, key: &str, selected: impl Into<String>) -> bool {
        let path = selected.into();
        match key {
            "enter" => self.activate(path),
            "space" => self.toggle(path),
            "delete" => self.delete(path),
            "f2" => self.rename(path, String::new()),
            "up" | "down" | "home" | "end" | "left" | "right" => {
                self.model.reduce_keyboard(key);
            }
            _ => return false,
        }
        true
    }
}

impl Render for FileExplorerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let rows = self.model.visible_rows();
        let active = self.model.active.clone();
        let row_elements = rows.iter().enumerate().map(|(index, row)| {
            let path = row.path.clone();
            let selected = row.selected;
            let is_active = active.as_deref() == Some(row.path.as_str());
            let target = target.clone();
            let chevron = match &row.kind {
                ExplorerNodeKind::Folder if row.expanded => "⌄",
                ExplorerNodeKind::Folder => "›",
                _ => " ",
            };
            div()
                .id(("explorer-row", index))
                .h(px(26.))
                .pl(px(8. + row.depth as f32 * 16.))
                .flex()
                .items_center()
                .gap_1()
                .text_color(rgb(if selected {
                    0x5b3c92ff
                } else if is_active {
                    0x245b8aff
                } else {
                    0x252525ff
                }))
                .bg(rgb(if selected {
                    0xe9e1f5ff
                } else if is_active {
                    0xe5f0f8ff
                } else {
                    0x00000000
                }))
                .hover(|style| style.bg(rgb(0xf0f0f0ff)))
                .child(chevron)
                .child(row.glyph)
                .child(row.name.clone())
                .on_click(move |_, _, cx| {
                    target.update(cx, |view, cx| {
                        view.click(path.clone());
                        cx.notify();
                    });
                })
        });
        let key_target = target.clone();
        let context = self.context_path.clone();
        div()
            .id("file-explorer")
            .key_context("file-explorer")
            .flex()
            .flex_col()
            .bg(rgb(0xf7f7f7ff))
            .text_color(rgb(0x252525ff))
            .when(self.toolbar_visible, |el| {
                el.child(
                    div()
                        .h(px(34.))
                        .px_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(rgb(0xe1e1e1ff))
                        .child("Files")
                        .child("⌕  Filter   +"),
                )
            })
            .children(row_elements)
            .when(rows.is_empty(), |el| {
                el.child(
                    div()
                        .px_2()
                        .py_2()
                        .text_color(rgb(0x666666ff))
                        .child("No files"),
                )
            })
            .when_some(context, |el, path| {
                el.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(0x555555ff))
                        .child(format!(
                            "Context: {path}  ·  New note  ·  New folder  ·  Rename  ·  Delete"
                        )),
                )
            })
            .on_key_down(move |event, _, cx| {
                key_target.update(cx, |view, cx| {
                    if view.keyboard(
                        &event.keystroke.key,
                        view.model.selected.clone().unwrap_or_default(),
                    ) {
                        cx.notify();
                    }
                });
            })
    }
}
