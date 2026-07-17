//! Small, render-neutral command types for workspace integrations.
//!
//! These types intentionally do not derive (or depend on) `serde` traits.  A
//! UI adapter can translate them to the reducer's internal actions, while a
//! persistence adapter can choose its own wire format.

/// Whether opening an item should reuse the current tab or create a tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenMode {
    /// Replace the currently selected tab when possible.
    #[default]
    Reuse,
    /// Always create a new tab.
    NewTab,
}

/// Request to open a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCommand {
    pub view: String,
    pub mode: OpenMode,
}

impl OpenCommand {
    pub fn new(view: impl Into<String>) -> Self {
        Self {
            view: view.into(),
            mode: OpenMode::default(),
        }
    }
}

/// Scope used when closing tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseScope {
    #[default]
    Single,
    Others,
    ToRight,
}

/// Close a tab, optionally including its neighbours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CloseCommand {
    pub tab: String,
    pub scope: CloseScope,
}

/// Reorder a tab within its tab group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReorderCommand {
    pub tab: String,
    pub index: usize,
}

/// Direction of a split.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Split the leaf containing `tab`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitCommand {
    pub tab: String,
    pub orientation: SplitOrientation,
}

/// Move a leaf into another tab group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveLeafCommand {
    pub leaf: String,
    pub target_group: String,
}

/// Duplicate a tab, optionally selecting the copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DuplicateCommand {
    pub tab: String,
    pub activate: bool,
}

/// Navigate the selected view's history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryCommand {
    Back,
    Forward,
}

/// Reopen the most recently closed tab (or a specific closed tab).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReopenClosedCommand {
    pub tab: Option<String>,
}

/// An action involving a serialized layout snapshot.  The snapshot remains
/// opaque so callers can use JSON, a database blob, or any other format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistedLayoutAction {
    Save { snapshot: String },
    Restore { snapshot: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ergonomic_defaults_are_safe() {
        assert_eq!(OpenMode::default(), OpenMode::Reuse);
        assert_eq!(CloseScope::default(), CloseScope::Single);
        assert_eq!(SplitOrientation::default(), SplitOrientation::Horizontal);
        assert!(!DuplicateCommand::default().activate);
        assert_eq!(ReopenClosedCommand::default().tab, None);
    }

    #[test]
    fn open_command_accepts_owned_and_borrowed_views() {
        assert_eq!(OpenCommand::new("editor").view, "editor");
        assert_eq!(
            OpenCommand::new(String::from("terminal")).mode,
            OpenMode::Reuse
        );
    }
}
