//! Policy decisions for closing editor tabs.

/// Whether an editor tab may be closed without further confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// The tab contains unsaved changes and must remain open.
    Blocked,
    /// The tab is clean (or empty) and may be closed.
    Allowed,
}

/// Decide whether a tab can be closed based on its dirty state.
pub const fn close_decision(is_dirty: bool) -> CloseDecision {
    if is_dirty {
        CloseDecision::Blocked
    } else {
        CloseDecision::Allowed
    }
}

/// State used by the tab strip.  Keeping this derived from the workspace
/// rather than from button intent prevents controls being shown for an empty
/// leaf (or after the last tab has been closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabControls {
    pub has_tab: bool,
    pub can_close: bool,
    pub can_split: bool,
}

impl TabControls {
    pub const fn from_workspace(has_tab: bool, is_dirty: bool, leaf_count: usize) -> Self {
        Self {
            has_tab,
            can_close: has_tab && !is_dirty,
            can_split: has_tab && leaf_count > 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{close_decision, CloseDecision, TabControls};

    #[test]
    fn dirty_documents_are_blocked_and_clean_documents_allowed() {
        assert_eq!(close_decision(true), CloseDecision::Blocked);
        assert_eq!(close_decision(false), CloseDecision::Allowed);
    }

    #[test]
    fn controls_follow_mounted_leaf_state() {
        assert_eq!(
            TabControls::from_workspace(false, false, 1),
            TabControls {
                has_tab: false,
                can_close: false,
                can_split: false,
            }
        );
        assert!(!TabControls::from_workspace(true, true, 1).can_close);
        assert!(TabControls::from_workspace(true, false, 2).can_split);
    }
}
