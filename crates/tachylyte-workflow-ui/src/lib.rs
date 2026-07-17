//! Pure GPUI workflow surfaces and typed intent models.
//!
//! This crate performs no I/O. It presents workflow state and emits typed
//! intents for callers to interpret and execute at an application boundary.
//!
//! In filtered lists, keyboard navigation from an unselected state selects the
//! first or last visible row. Enter normalizes stale selection before emitting
//! an activation intent.

pub mod composer;
pub mod model;
pub mod panes;
pub mod surfaces;

pub use composer::*;
pub use model::*;
pub use panes::*;
pub use surfaces::*;
