//! Settings surface for Tachylyte applications.
//!
//! The crate provides a GPUI view for mounting a small, data-oriented settings
//! model. Applications that do not render the settings window can use the
//! model and its [`model::SettingsEvent`] values directly, while GPUI callers
//! can mount [`SettingsWindow`] with [`SettingsWindow::mount`].
//!
//! The model event is re-exported as [`SettingsEvent`]. The view emits those
//! same neutral events, allowing a host to reduce changes without coupling its
//! persistence layer to GPUI.

mod model;
mod view;

pub use model::{
    Category, CorePlugin, EditorSettings, FilesAndLinksSettings, Hotkey, NewLinkFormat,
    NewNoteLocation, Settings, SettingsEvent, Theme,
};
pub use view::SettingsWindow;
