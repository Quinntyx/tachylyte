//! Native light GPUI surface for synchronization settings.

use std::collections::BTreeSet;

use crate::model::{SyncIntent, SyncModel, SyncState};
use gpui::{div, prelude::*, rgb, Context, ElementId, FocusHandle, Render, Window};
use tachylyte_services::sync::{Resolution, SyncState as ServiceSyncState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncSnapshot {
    pub enabled: bool,
    pub selective_folders: BTreeSet<String>,
    pub selective_settings: BTreeSet<String>,
    pub devices: Vec<String>,
    pub activity: Vec<String>,
    pub conflicts: Vec<String>,
    pub state: SyncState,
    pub paused: bool,
    pub backend_configured: bool,
    pub offline: bool,
}

pub struct SyncSurface {
    pub model: SyncModel,
    pub focus_handle: FocusHandle,
}

impl SyncSurface {
    pub fn new(model: SyncModel, cx: &mut Context<Self>) -> Self {
        Self {
            model,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn mount(model: SyncModel, cx: &mut Context<Self>) -> gpui::Entity<Self> {
        cx.new(|cx| Self::new(model, cx))
    }

    pub fn snapshot(&self) -> SyncSnapshot {
        SyncSnapshot {
            enabled: self.model.enabled,
            selective_folders: self.model.selective_folders.clone(),
            selective_settings: self.model.selective_settings.clone(),
            devices: self
                .model
                .devices
                .iter()
                .map(|device| {
                    format!(
                        "{} ({}) — {}",
                        device.name, device.platform, device.last_seen
                    )
                })
                .collect(),
            activity: self
                .model
                .activity
                .iter()
                .map(|entry| format!("{} — {}", entry.summary, entry.detail))
                .collect(),
            conflicts: self
                .model
                .conflicts
                .iter()
                .map(|conflict| conflict.resource.clone())
                .collect(),
            state: self.model.state.clone(),
            paused: self.model.paused,
            backend_configured: self.model.backend_configured,
            offline: self.model.offline,
        }
    }

    pub fn drain_intents(&mut self) -> Vec<SyncIntent> {
        self.model.drain_intents()
    }

    pub fn toggle(&mut self) {
        self.model.toggle_enabled();
    }

    pub fn set_folder(&mut self, folder: impl Into<String>, selected: bool) {
        self.model.set_selective_folder(folder.into(), selected);
    }

    pub fn set_setting(&mut self, setting: impl Into<String>, selected: bool) {
        self.model.set_selective_setting(setting.into(), selected);
    }

    pub fn pause(&mut self) {
        self.model.request_pause();
    }

    pub fn resume(&mut self) {
        self.model.request_resume();
    }

    pub fn resolve(&mut self, resource: impl Into<String>, resolution: Resolution) {
        self.model.request_resolve(resource.into(), resolution);
    }
}

fn row(text: impl Into<String>) -> gpui::Div {
    div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(0xe5e5e5))
        .child(text.into())
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<String>,
    target: gpui::Entity<SyncSurface>,
    action: impl Fn(&mut SyncSurface) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .bg(rgb(0x7852ee))
        .text_color(rgb(0xffffff))
        .child(label.into())
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            target.update(cx, |surface, cx| {
                action(surface);
                cx.notify();
            });
        })
}

impl Render for SyncSurface {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let snapshot = self.snapshot();
        let header = div().text_xl().child("Sync");

        if !snapshot.backend_configured {
            return div()
                .id("sync-surface")
                .size_full()
                .bg(rgb(0xffffff))
                .text_color(rgb(0x222222))
                .p_6()
                .child(header)
                .child(row("Sync is unavailable: no sync backend is configured."));
        }
        if snapshot.offline || matches!(snapshot.state, ServiceSyncState::Offline) {
            return div()
                .id("sync-surface")
                .size_full()
                .bg(rgb(0xffffff))
                .text_color(rgb(0x222222))
                .p_6()
                .child(header)
                .child(row(
                    "You are offline. Sync changes will wait for connectivity.",
                ));
        }

        let toggle_target = target.clone();
        let toggle = div()
            .id("sync-enabled")
            .flex()
            .items_center()
            .justify_between()
            .child(if snapshot.enabled {
                "Sync enabled"
            } else {
                "Sync disabled"
            })
            .child(button(
                "sync-enabled-toggle",
                if snapshot.enabled {
                    "Disable"
                } else {
                    "Enable"
                },
                toggle_target,
                |surface| surface.toggle(),
            ));

        let folder_rows = snapshot.selective_folders.iter().map(|folder| {
            let folder = folder.clone();
            let selected = true;
            let target = target.clone();
            div()
                .id(ElementId::Name(format!("sync-folder-{folder}").into()))
                .child(if selected { "☑ " } else { "☐ " })
                .child(folder.clone())
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    target.update(cx, |surface, cx| {
                        surface.set_folder(folder.clone(), !selected);
                        cx.notify();
                    });
                })
        });
        let setting_rows = snapshot.selective_settings.iter().map(|setting| {
            let setting = setting.clone();
            let target = target.clone();
            div()
                .id(ElementId::Name(format!("sync-setting-{setting}").into()))
                .child("☑ ")
                .child(setting.clone())
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    target.update(cx, |surface, cx| {
                        surface.set_setting(setting.clone(), false);
                        cx.notify();
                    });
                })
        });
        let devices = self.model.devices.iter().map(|device| {
            row(format!(
                "{} · {} · {}{}",
                device.name,
                device.platform,
                device.last_seen,
                if device.current {
                    " · This device"
                } else {
                    ""
                }
            ))
        });
        let activity = self
            .model
            .activity
            .iter()
            .map(|entry| row(format!("{} · {}", entry.summary, entry.detail)));
        let conflicts = self.model.conflicts.iter().map(|conflict| {
            let resource = conflict.resource.clone();
            let local_target = target.clone();
            let remote_target = target.clone();
            div()
                .flex()
                .flex_col()
                .child(row(format!("Conflict: {}", resource)))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(button(
                            ElementId::Name(
                                format!("sync-keep-local-{}", conflict.resource).into(),
                            ),
                            "Keep local",
                            local_target,
                            {
                                let resource = conflict.resource.clone();
                                move |surface| {
                                    surface.resolve(resource.clone(), Resolution::KeepLocal)
                                }
                            },
                        ))
                        .child(button(
                            ElementId::Name(
                                format!("sync-keep-remote-{}", conflict.resource).into(),
                            ),
                            "Keep remote",
                            remote_target,
                            {
                                let resource = conflict.resource.clone();
                                move |surface| {
                                    surface.resolve(resource.clone(), Resolution::KeepRemote)
                                }
                            },
                        )),
                )
        });
        let pause_target = target.clone();
        let pause_button = if snapshot.paused {
            button("sync-resume", "Resume", pause_target, |surface| {
                surface.resume()
            })
        } else {
            button("sync-pause", "Pause", pause_target, |surface| {
                surface.pause()
            })
        };

        div()
            .id("sync-surface")
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222))
            .flex()
            .flex_col()
            .p_6()
            .child(header)
            .child(div().mt_4().child(toggle))
            .child(row(format!("Status: {:?}", snapshot.state)))
            .child(row("Selective folders"))
            .children(folder_rows)
            .child(row("Selective settings"))
            .children(setting_rows)
            .child(row("Devices"))
            .children(devices)
            .child(row("Activity"))
            .children(activity)
            .child(row("Conflicts"))
            .children(conflicts)
            .child(div().mt_4().child(pause_button))
    }
}
