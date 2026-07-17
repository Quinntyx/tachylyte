//! App-local mounting helpers for the settings surface.

use gpui::{div, prelude::*, px, rgba, Context, Entity, IntoElement};
use tachylyte_settings_ui::{Settings, SettingsEvent, SettingsWindow};

/// A lightweight handle for presenting settings as a modal or popout.
///
/// The settings state remains owned by the settings-ui crate.  This adapter
/// only owns its GPUI entity and provides the small amount of lifecycle glue
/// needed by an application shell.
pub struct SettingsSurface {
    inner: Entity<SettingsWindow>,
}

impl SettingsSurface {
    /// Mount a settings window with its default model in the application
    /// context.
    pub fn new<T>(cx: &mut Context<T>) -> Self {
        Self::with_model(Settings::default(), cx)
    }

    /// Mount a settings window with an explicitly supplied model.
    pub fn with_model<T>(model: Settings, cx: &mut Context<T>) -> Self {
        Self {
            inner: cx.new(|cx| SettingsWindow::new(model, cx)),
        }
    }

    /// Return the mounted settings entity for direct GPUI updates.
    pub fn inner(&self) -> Entity<SettingsWindow> {
        self.inner.clone()
    }

    /// Render the settings window inside a modal-style scrim.
    ///
    /// The returned element can be inserted into an application's overlay or
    /// popout layer.  Closing remains an event from the inner settings model.
    pub fn modal(&self) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgba(0x00000066))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(900.))
                    .h(px(650.))
                    .bg(rgba(0xffffffff))
                    .child(self.inner.clone()),
            )
    }

    /// Drain pending settings events from the mounted entity.
    pub fn drain_events<T>(&self, cx: &mut Context<T>) -> Vec<SettingsEvent> {
        self.inner.update(cx, |window, _cx| window.drain_events())
    }
}
