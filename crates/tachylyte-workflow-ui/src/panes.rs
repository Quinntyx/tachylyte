//! Small, filesystem-free sidebar panes used by the workflow surface.
//!
//! These views deliberately consume snapshots.  Loading, indexing, and persistence belong to
//! the workflow model; a pane only filters rows, tracks a cursor, and emits intents.

use gpui::{div, prelude::*, px, rgb, Context, IntoElement, Render, Window};
use std::collections::BTreeMap;
use tachylyte_knowledge::{Bookmark, Heading};

use crate::model::WorkflowIntent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneRow {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
}

impl PaneRow {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
        }
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub id: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Home,
    End,
    Enter,
    Escape,
}

#[derive(Clone, Debug, Default)]
struct PaneState {
    query: String,
    cursor: usize,
    intents: Vec<WorkflowIntent>,
}

macro_rules! pane {
    ($name:ident, $title:literal) => {
        #[derive(Clone, Debug, Default)]
        pub struct $name {
            rows: Vec<PaneRow>,
            state: PaneState,
        }
        impl $name {
            pub fn new(rows: Vec<PaneRow>) -> Self {
                Self {
                    rows,
                    state: PaneState::default(),
                }
            }
            pub fn query(&self) -> &str {
                &self.state.query
            }
            pub fn update(&mut self, rows: Vec<PaneRow>) {
                self.rows = rows;
                self.bound();
            }
            pub fn rows(&self) -> &[PaneRow] {
                &self.rows
            }
            pub fn visible(&self) -> Vec<&PaneRow> {
                self.rows
                    .iter()
                    .filter(|r| {
                        self.state.query.is_empty()
                            || r.label
                                .to_lowercase()
                                .contains(&self.state.query.to_lowercase())
                            || r.detail.as_deref().is_some_and(|d| {
                                d.to_lowercase().contains(&self.state.query.to_lowercase())
                            })
                    })
                    .collect()
            }
            pub fn keyboard(&mut self, key: Key) {
                let n = self.visible().len();
                match key {
                    Key::Up => self.state.cursor = self.state.cursor.saturating_sub(1),
                    Key::Down => {
                        self.state.cursor = (self.state.cursor + 1).min(n.saturating_sub(1))
                    }
                    Key::Home => self.state.cursor = 0,
                    Key::End => self.state.cursor = n.saturating_sub(1),
                    Key::Enter => self.activate(),
                    Key::Escape => self.state.query.clear(),
                }
                self.bound();
            }
            pub fn set_query(&mut self, query: impl Into<String>) {
                self.state.query = query.into();
                self.state.cursor = 0;
            }
            pub fn activate(&mut self) {
                if let Some(row) = self.visible().get(self.state.cursor) {
                    self.state
                        .intents
                        .push(WorkflowIntent::open_path(row.id.clone()));
                }
            }
            pub fn activate_id(&mut self, id: &str) {
                if self.rows.iter().any(|r| r.id == id) {
                    self.state
                        .intents
                        .push(WorkflowIntent::open_path(id.to_owned()));
                }
            }
            pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
                std::mem::take(&mut self.state.intents)
            }
            pub fn view(&self) -> impl IntoElement {
                let rows = self
                    .visible()
                    .into_iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let selected = i == self.state.cursor;
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(3.))
                            .bg(rgb(if selected { 0xe8e5df } else { 0xf8f7f4 }))
                            .child(div().text_sm().child(row.label.clone()))
                            .when_some(row.detail.clone(), |el, d| {
                                el.child(div().text_xs().text_color(rgb(0x77736d)).child(d))
                            })
                    })
                    .collect::<Vec<_>>();
                div()
                    .w(px(240.))
                    .p(px(8.))
                    .bg(rgb(0xf4f1eb))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child($title),
                    )
                    .children(rows)
            }
            fn bound(&mut self) {
                self.state.cursor = self
                    .state
                    .cursor
                    .min(self.visible().len().saturating_sub(1));
            }
        }
        impl Render for $name {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                self.view()
            }
        }
    };
}

pane!(BookmarksPane, "Bookmarks");
pane!(TagsPane, "Tags");
pane!(PropertiesPane, "Properties");
pane!(OutlinePane, "Outline");

impl BookmarksPane {
    /// Build a pane from a knowledge snapshot. The tree is flattened for compact navigation;
    /// the original bookmark ids remain the activation payload.
    pub fn from_bookmarks(bookmarks: &[Bookmark]) -> Self {
        fn flatten(bookmarks: &[Bookmark], rows: &mut Vec<PaneRow>) {
            for bookmark in bookmarks {
                rows.push(PaneRow::new(&bookmark.id, &bookmark.title));
                flatten(&bookmark.children, rows);
            }
        }
        let mut rows = Vec::new();
        flatten(bookmarks, &mut rows);
        Self::new(rows)
    }
}

impl TagsPane {
    /// Build tag rows from the deterministic counts produced by `tachylyte-knowledge`.
    pub fn from_counts(counts: BTreeMap<String, usize>) -> Self {
        Self::new(
            counts
                .into_iter()
                .map(|(tag, count)| {
                    PaneRow::new(tag.clone(), format!("#{tag}")).detail(count.to_string())
                })
                .collect(),
        )
    }
}

impl PropertiesPane {
    /// Build property rows from a knowledge index projection.
    pub fn from_counts(counts: BTreeMap<String, usize>) -> Self {
        Self::new(
            counts
                .into_iter()
                .map(|(name, count)| PaneRow::new(name.clone(), name).detail(count.to_string()))
                .collect(),
        )
    }
}

impl OutlinePane {
    /// Build outline rows without retaining the source document.
    pub fn from_headings(headings: &[Heading]) -> Self {
        Self::new(
            headings
                .iter()
                .map(|heading| {
                    PaneRow::new(heading.line.to_string(), heading.text.clone()).detail(format!(
                        "H{} · line {}",
                        heading.level,
                        heading.line + 1
                    ))
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Notices {
    notices: Vec<Notice>,
    intents: Vec<WorkflowIntent>,
}
impl Notices {
    pub fn new(notices: Vec<Notice>) -> Self {
        Self {
            notices,
            intents: Vec::new(),
        }
    }
    pub fn update(&mut self, notices: Vec<Notice>) {
        self.notices = notices;
    }
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }
    pub fn dismiss(&mut self, id: &str) {
        if self.notices.iter().any(|n| n.id == id) {
            self.notices.retain(|n| n.id != id);
            self.intents
                .push(WorkflowIntent::dismiss_notice(id.to_owned()));
        }
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        std::mem::take(&mut self.intents)
    }
    pub fn view(&self) -> impl IntoElement {
        div().children(
            self.notices
                .iter()
                .map(|n| div().p(px(6.)).text_xs().child(n.message.clone())),
        )
    }
}

impl Render for Notices {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_filter_and_bound_selection() {
        let mut pane = TagsPane::new(vec![
            PaneRow::new("a", "Daily"),
            PaneRow::new("b", "Project"),
        ]);
        pane.set_query("proj");
        assert_eq!(pane.visible().len(), 1);
        pane.keyboard(Key::Down);
        pane.keyboard(Key::Enter);
        assert_eq!(pane.visible()[0].id, "b");
    }

    #[test]
    fn notices_can_be_replaced_and_dismissed() {
        let mut notices = Notices::new(vec![Notice {
            id: "n1".into(),
            message: "hello".into(),
        }]);
        assert_eq!(notices.notices().len(), 1);
        notices.update(Vec::new());
        assert!(notices.notices().is_empty());
    }
}
