//! Small, deliberately side-effect-free workflow controls.
//!
//! These views only turn user actions into [`crate::model::WorkflowIntent`]s.  In
//! particular, they do not call the workflow engine (or touch the filesystem).

use gpui::{div, prelude::*, px, rgb, Context, IntoElement, Render, Window};
use tachylyte_workflows::{DailyNoteConfig, Snapshot};

use crate::model::WorkflowIntent;

fn button(label: &str) -> gpui::Div {
    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(rgb(0xe8edf2))
        .text_color(rgb(0x24303a))
        .child(label.to_owned())
}

/// Input for [`DailyNoteAction`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DailyNoteActionInput {
    pub config: DailyNoteConfig,
}

pub struct DailyNoteAction {
    pub input: DailyNoteActionInput,
    pending: Vec<WorkflowIntent>,
}

impl DailyNoteAction {
    pub fn new(config: DailyNoteConfig) -> Self {
        Self {
            input: DailyNoteActionInput { config },
            pending: Vec::new(),
        }
    }
    pub fn update(&mut self, input: DailyNoteActionInput) {
        self.input = input;
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        std::mem::take(&mut self.pending)
    }
    pub fn activate(&self) -> WorkflowIntent {
        WorkflowIntent::create_daily_note_from(
            self.input.config.folder.clone(),
            self.input.config.date_format.clone(),
            self.input.config.template.clone(),
        )
    }
}

impl Render for DailyNoteAction {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let create = button("Create")
            .id("daily-note-create")
            .on_click(move |_, _, cx| {
                entity.update(cx, |action, _| {
                    let intent = action.activate();
                    action.pending.push(intent);
                });
            });
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .p(px(8.0))
            .bg(rgb(0xffffff))
            .child(div().text_color(rgb(0x24303a)).child("Daily note"))
            .child(create)
    }
}

/// Input for a unique-note creation control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueNoteCreatorInput {
    pub title: String,
    pub folder: String,
}

pub struct UniqueNoteCreator {
    pub input: UniqueNoteCreatorInput,
    pending: Vec<WorkflowIntent>,
}

impl UniqueNoteCreator {
    pub fn new(title: impl Into<String>, folder: impl Into<String>) -> Self {
        Self {
            input: UniqueNoteCreatorInput {
                title: title.into(),
                folder: folder.into(),
            },
            pending: Vec::new(),
        }
    }
    pub fn update(&mut self, input: UniqueNoteCreatorInput) {
        self.input = input;
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        std::mem::take(&mut self.pending)
    }
    pub fn activate(&self) -> WorkflowIntent {
        WorkflowIntent::create_unique_note_in(self.input.folder.clone(), self.input.title.clone())
    }
}

impl Render for UniqueNoteCreator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let create = button("New note")
            .id("unique-note-create")
            .on_click(move |_, _, cx| {
                entity.update(cx, |creator, _| {
                    let intent = creator.activate();
                    creator.pending.push(intent);
                });
            });
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .p(px(8.0))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .text_color(rgb(0x24303a))
                    .child(self.input.title.clone()),
            )
            .child(create)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerModal {
    None,
    Merge,
    Split,
}

/// A text-only composer.  `submit` emits an intent; it never persists `text`.
pub struct NoteComposer {
    pub text: String,
    pub modal: ComposerModal,
}

impl NoteComposer {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            modal: ComposerModal::None,
        }
    }
    pub fn update(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
    pub fn select(&mut self, modal: ComposerModal) {
        self.modal = modal;
    }
    pub fn keyboard(&mut self, key: &str) -> Option<WorkflowIntent> {
        match key {
            "Enter" => Some(self.submit()),
            "Escape" => {
                self.modal = ComposerModal::None;
                None
            }
            _ => None,
        }
    }
    pub fn activate(&self) -> WorkflowIntent {
        self.submit()
    }
    pub fn submit(&self) -> WorkflowIntent {
        WorkflowIntent::compose_note("", self.text.clone())
    }
    pub fn merge(&self) -> WorkflowIntent {
        WorkflowIntent::merge_notes(vec![self.text.clone()])
    }
    pub fn split(&self) -> WorkflowIntent {
        WorkflowIntent::split_note(self.text.clone())
    }
}

impl Render for NoteComposer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let dialog = match self.modal {
            ComposerModal::None => div(),
            ComposerModal::Merge => div().p(px(8.0)).bg(rgb(0xf4f6f8)).child("Merge notes"),
            ComposerModal::Split => div().p(px(8.0)).bg(rgb(0xf4f6f8)).child("Split note"),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(8.0))
            .bg(rgb(0xffffff))
            .child(self.text.clone())
            .child(dialog)
    }
}

pub struct RecoverySnapshots {
    pub snapshots: Vec<Snapshot>,
    pub selected: usize,
}

impl RecoverySnapshots {
    pub fn new(snapshots: Vec<Snapshot>) -> Self {
        Self {
            snapshots,
            selected: 0,
        }
    }
    pub fn update(&mut self, snapshots: Vec<Snapshot>) {
        self.snapshots = snapshots;
        self.selected = 0;
    }
    pub fn select(&mut self, index: usize) {
        if index < self.snapshots.len() {
            self.selected = index;
        }
    }
    pub fn keyboard(&mut self, key: &str) -> Option<WorkflowIntent> {
        match key {
            "ArrowDown" => {
                self.select(self.selected.saturating_add(1));
                None
            }
            "ArrowUp" => {
                self.select(self.selected.saturating_sub(1));
                None
            }
            "Enter" => self.activate(),
            _ => None,
        }
    }
    pub fn activate(&self) -> Option<WorkflowIntent> {
        self.snapshots
            .get(self.selected)
            .map(|s| WorkflowIntent::recover_snapshot(s.revision.to_string()))
    }
}

impl Render for RecoverySnapshots {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.snapshots.iter().enumerate().fold(
            div().flex().flex_col().gap(px(2.0)),
            |rows, (i, s)| {
                rows.child(
                    div()
                        .p(px(6.0))
                        .bg(if i == self.selected {
                            rgb(0xe8edf2)
                        } else {
                            rgb(0xffffff)
                        })
                        .child(format!("#{}  {}", s.revision, s.timestamp)),
                )
            },
        );
        div().p(px(8.0)).bg(rgb(0xffffff)).child(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DailyNoteConfig {
        DailyNoteConfig {
            folder: "daily".into(),
            date_format: "%Y-%m-%d".into(),
            template: None,
        }
    }

    #[test]
    fn controls_emit_intents_without_performing_io() {
        assert_eq!(
            DailyNoteAction::new(config()).activate(),
            WorkflowIntent::CreateDailyNote {
                folder: "daily".into(),
                date_format: "%Y-%m-%d".into(),
                template: None,
            }
        );
        assert_eq!(
            UniqueNoteCreator::new("Meeting", "notes").activate(),
            WorkflowIntent::CreateUniqueNote {
                title: "Meeting".into(),
                folder: "notes".into(),
            }
        );
        assert_eq!(
            NoteComposer::new("body").merge(),
            WorkflowIntent::MergeNotes {
                paths: vec!["body".into()]
            }
        );
    }
}
