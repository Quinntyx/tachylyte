//! Filesystem-independent state and messages used by the workflow UI.

use std::collections::VecDeque;

/// The small set of keys understood by a selectable list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListKey {
    Up,
    Down,
    Home,
    End,
    Enter,
}

/// A compact, reusable list model.  The item itself remains owned by the caller's model;
/// filtering only stores matching indexes and never performs filesystem work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowListState<T> {
    pub rows: Vec<T>,
    pub query: String,
    pub selected: Option<usize>,
    intents: VecDeque<WorkflowIntent>,
}

impl<T: AsRef<str>> RowListState<T> {
    pub fn new(rows: Vec<T>) -> Self {
        Self {
            rows,
            query: String::new(),
            selected: None,
            intents: VecDeque::new(),
        }
    }

    pub fn with_query(rows: Vec<T>, query: impl Into<String>) -> Self {
        let mut state = Self::new(rows);
        state.set_query(query);
        state
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        let matching = self.filtered_indices();
        self.selected = self.selected.filter(|i| matching.contains(i));
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let query = self.query.to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| row.as_ref().to_lowercase().contains(&query).then_some(i))
            .collect()
    }

    pub fn filtered_rows(&self) -> Vec<&T> {
        self.filtered_indices()
            .into_iter()
            .map(|i| &self.rows[i])
            .collect()
    }

    pub fn selected_row(&self) -> Option<&T> {
        self.selected.and_then(|i| self.rows.get(i))
    }

    /// Applies a key and returns the selected row on `Enter`.
    pub fn key(&mut self, key: ListKey) -> Option<&T> {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            self.selected = None;
            return None;
        }
        match key {
            ListKey::Up => {
                self.selected = Some(
                    match self
                        .selected
                        .and_then(|i| indices.iter().position(|&x| x == i))
                    {
                        Some(0) | None => indices[0],
                        Some(p) => indices[p - 1],
                    },
                )
            }
            ListKey::Down => {
                self.selected = Some(
                    match self
                        .selected
                        .and_then(|i| indices.iter().position(|&x| x == i))
                    {
                        Some(p) if p + 1 < indices.len() => indices[p + 1],
                        _ => indices[indices.len() - 1],
                    },
                )
            }
            ListKey::Home => self.selected = Some(indices[0]),
            ListKey::End => self.selected = Some(indices[indices.len() - 1]),
            ListKey::Enter => {
                let index = self.selected.unwrap_or(indices[0]);
                self.selected = Some(index);
                return self.rows.get(index);
            }
        }
        None
    }

    pub fn push_intent(&mut self, intent: WorkflowIntent) {
        self.intents.push_back(intent);
    }
    pub fn drain_intents(&mut self) -> impl Iterator<Item = WorkflowIntent> + '_ {
        self.intents.drain(..)
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        self.intents.drain(..).collect()
    }
}

/// Typed actions emitted by workflow UI controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowIntent {
    OpenPath {
        path: String,
    },
    RunCommand {
        command: String,
    },
    ChooseTemplate {
        template: String,
    },
    CreateDailyNote {
        folder: String,
        date_format: String,
        template: Option<String>,
    },
    CreateUniqueNote {
        title: String,
        folder: String,
    },
    ComposeNote {
        title: String,
        body: String,
    },
    MergeNotes {
        paths: Vec<String>,
    },
    SplitNote {
        path: String,
    },
    RecoverSnapshot {
        id: String,
    },
    OpenSidebar {
        name: String,
    },
    DismissNotice {
        id: String,
    },
}

impl WorkflowIntent {
    pub fn open_path(path: impl Into<String>) -> Self {
        Self::OpenPath { path: path.into() }
    }
    pub fn run_command(command: impl Into<String>) -> Self {
        Self::RunCommand {
            command: command.into(),
        }
    }
    pub fn choose_template(template: impl Into<String>) -> Self {
        Self::ChooseTemplate {
            template: template.into(),
        }
    }
    pub fn create_daily_note(date: impl Into<String>) -> Self {
        Self::CreateDailyNote {
            folder: String::new(),
            date_format: date.into(),
            template: None,
        }
    }
    pub fn create_daily_note_from(
        folder: impl Into<String>,
        date_format: impl Into<String>,
        template: Option<String>,
    ) -> Self {
        Self::CreateDailyNote {
            folder: folder.into(),
            date_format: date_format.into(),
            template,
        }
    }
    pub fn create_unique_note(title: impl Into<String>) -> Self {
        Self::CreateUniqueNote {
            title: title.into(),
            folder: String::new(),
        }
    }
    pub fn create_unique_note_in(folder: impl Into<String>, title: impl Into<String>) -> Self {
        Self::CreateUniqueNote {
            title: title.into(),
            folder: folder.into(),
        }
    }
    pub fn compose_note(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::ComposeNote {
            title: title.into(),
            body: body.into(),
        }
    }
    pub fn merge_notes(paths: Vec<String>) -> Self {
        Self::MergeNotes { paths }
    }
    pub fn split_note(path: impl Into<String>) -> Self {
        Self::SplitNote { path: path.into() }
    }
    pub fn recover_snapshot(id: impl Into<String>) -> Self {
        Self::RecoverSnapshot { id: id.into() }
    }
    pub fn open_sidebar(name: impl Into<String>) -> Self {
        Self::OpenSidebar { name: name.into() }
    }
    pub fn dismiss_notice(id: impl Into<String>) -> Self {
        Self::DismissNotice { id: id.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_and_bounds_selection() {
        let mut list = RowListState::new(vec!["Alpha", "Beta", "Alphabet"]);
        list.set_query("ALP");
        assert_eq!(list.filtered_rows(), vec![&"Alpha", &"Alphabet"]);
        list.key(ListKey::End);
        list.key(ListKey::Down);
        assert_eq!(list.selected_row(), Some(&"Alphabet"));
    }
    #[test]
    fn intents_can_be_taken() {
        let mut list = RowListState::<String>::new(vec![]);
        list.push_intent(WorkflowIntent::open_path("notes/a.md"));
        assert_eq!(
            list.take_intents(),
            vec![WorkflowIntent::OpenPath {
                path: "notes/a.md".into()
            }]
        );
    }
}
