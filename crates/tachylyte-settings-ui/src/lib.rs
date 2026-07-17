//! Settings surface for Tachylyte applications.
//!
//! The crate provides a GPUI view for mounting a small, data-oriented settings
//! model. A GPUI host creates an entity with [`SettingsWindow::mount`] and
//! mounts that entity in its window; the window owns the model while it is
//! displayed.
//!
//! User interactions are recorded by the model as events. Hosts should call
//! [`SettingsWindow::drain_events`] after updates to take the pending events
//! and forward them to persistence or application state. The view uses stable
//! element IDs for its interactive controls, and emits neutral model events
//! rather than GPUI-specific events. This keeps event handling and persistence
//! independent of the rendering layer.
//!
//! Applications that do not render the settings window can use the model and
//! its [`model::SettingsEvent`] values directly. The model event is also
//! re-exported as [`SettingsEvent`].

mod model;
mod view;

pub use model::{
    Category, CorePlugin, EditorSettings, FilesAndLinksSettings, Hotkey, NewLinkFormat,
    NewNoteLocation, Settings, SettingsEvent, Theme,
};
pub use view::SettingsWindow;
