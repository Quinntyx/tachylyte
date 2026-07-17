//! Typed, filesystem-free intents emitted by the explorer UI.
//!
//! These values describe what a caller should do; they do not perform any
//! filesystem or workspace operations themselves.

use std::fmt;

/// The sort choices understood by the explorer intent layer.
///
/// This intentionally lives here rather than depending on a model type, so
/// this UI vocabulary can be used without introducing a dependency cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ExplorerSortMode {
    #[default]
    Name,
    Modified,
    Created,
    Kind,
}

impl From<crate::SortMode> for ExplorerSortMode {
    fn from(mode: crate::SortMode) -> Self {
        match mode {
            crate::SortMode::Name => Self::Name,
            crate::SortMode::Modified => Self::Modified,
            crate::SortMode::Created => Self::Created,
        }
    }
}

impl ExplorerSortMode {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Created => "Created",
            Self::Kind => "Kind",
        }
    }
}

impl fmt::Display for ExplorerSortMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Toolbar and context-menu commands which do not need additional arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExplorerAction {
    NewNote,
    NewFolder,
    Rename,
    Delete,
    Move,
    Duplicate,
    Reveal,
    Open,
    CopyPath,
}

impl ExplorerAction {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::NewNote => "New note",
            Self::NewFolder => "New folder",
            Self::Rename => "Rename",
            Self::Delete => "Delete",
            Self::Move => "Move",
            Self::Duplicate => "Duplicate",
            Self::Reveal => "Reveal",
            Self::Open => "Open",
            Self::CopyPath => "Copy path",
        }
    }
}

/// A user-level explorer interaction, containing data but no I/O behavior.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExplorerIntent {
    Activate,
    NewNote { parent: Option<String> },
    NewFolder { parent: Option<String> },
    Rename { path: String, new_name: String },
    Delete { path: String },
    Move { path: String, destination: String },
    Duplicate { path: String, destination: String },
    Reveal { path: String },
    Open { path: String },
    Toggle { path: String, expanded: bool },
    Select { path: String },
    SetFilter { value: String },
    SetSort { mode: ExplorerSortMode },
    DragMove { source: String, destination: String },
    ContextMenu { path: Option<String> },
}

impl ExplorerIntent {
    pub fn new_note(parent: impl Into<String>) -> Self {
        Self::NewNote {
            parent: Some(parent.into()),
        }
    }

    pub fn new_root_note() -> Self {
        Self::NewNote { parent: None }
    }

    pub fn new_folder(parent: impl Into<String>) -> Self {
        Self::NewFolder {
            parent: Some(parent.into()),
        }
    }

    pub fn new_root_folder() -> Self {
        Self::NewFolder { parent: None }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::NewNote { .. } => "New note",
            Self::NewFolder { .. } => "New folder",
            Self::Rename { .. } => "Rename",
            Self::Delete { .. } => "Delete",
            Self::Move { .. } | Self::DragMove { .. } => "Move",
            Self::Duplicate { .. } => "Duplicate",
            Self::Reveal { .. } => "Reveal",
            Self::Open { .. } => "Open",
            Self::Activate => "Open",
            Self::Toggle { expanded: true, .. } => "Expand",
            Self::Toggle {
                expanded: false, ..
            } => "Collapse",
            Self::Select { .. } => "Select",
            Self::SetFilter { .. } => "Filter",
            Self::SetSort { .. } => "Sort",
            Self::ContextMenu { .. } => "Context menu",
        }
    }
}

impl fmt::Display for ExplorerIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_are_data_only_and_labeled() {
        assert_eq!(ExplorerIntent::new_root_note().label(), "New note");
        assert_eq!(ExplorerIntent::new_note("notes").label(), "New note");
        assert_eq!(
            ExplorerIntent::Toggle {
                path: "a".into(),
                expanded: false
            }
            .to_string(),
            "Collapse"
        );
        assert_eq!(
            ExplorerIntent::SetSort {
                mode: ExplorerSortMode::Kind
            }
            .label(),
            "Sort"
        );
    }

    #[test]
    fn constructors_preserve_parent() {
        assert_eq!(
            ExplorerIntent::new_folder("docs"),
            ExplorerIntent::NewFolder {
                parent: Some("docs".into())
            }
        );
        assert_eq!(
            ExplorerIntent::new_root_folder(),
            ExplorerIntent::NewFolder { parent: None }
        );
    }
}
