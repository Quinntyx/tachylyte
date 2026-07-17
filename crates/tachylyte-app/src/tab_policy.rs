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

#[cfg(test)]
mod tests {
    use super::{close_decision, CloseDecision};

    #[test]
    fn dirty_documents_are_blocked_and_clean_documents_allowed() {
        assert_eq!(close_decision(true), CloseDecision::Blocked);
        assert_eq!(close_decision(false), CloseDecision::Allowed);
    }
}
