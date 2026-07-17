//! Notices shown while managing editor tabs.

/// Return the notice displayed when a dirty tab cannot be closed yet.
pub const fn dirty_close_notice() -> &'static str {
    "Please save your changes before closing this tab."
}
