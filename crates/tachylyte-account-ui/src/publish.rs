//! Native GPUI surface for configuring and reviewing publication.

use crate::model::{PublishIntent, PublishModel};
use gpui::{div, prelude::*, Context, ElementId, FocusHandle, Render, Window};
use tachylyte_services::publish::{DiffKind, ManifestDiff};

/// The publication settings surface.  The model remains the source of truth;
/// this view only records typed intents through its model.
pub struct PublishSurface {
    pub model: PublishModel,
    pub focus_handle: FocusHandle,
}

impl PublishSurface {
    pub fn new(model: PublishModel, cx: &mut Context<Self>) -> Self {
        Self {
            model,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn mount(model: PublishModel, cx: &mut Context<Self>) -> gpui::Entity<Self> {
        cx.new(|cx| Self::new(model, cx))
    }

    pub fn snapshot(&self) -> PublishModel {
        self.model.clone()
    }

    pub fn drain_intents(&mut self) -> Vec<PublishIntent> {
        self.model.drain_intents()
    }
}

fn diff_label(diff: &ManifestDiff) -> String {
    let kind = match &diff.kind {
        DiffKind::Added => "Added",
        DiffKind::Changed => "Changed",
        DiffKind::Removed => "Removed",
    };
    format!("{kind}: {}", diff.path)
}

impl Render for PublishSurface {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let unavailable = !self.model.backend_configured;
        let offline = self.model.offline;
        let body = if unavailable {
            div().child("Publishing is unavailable until a backend is configured.")
        } else if offline {
            div().child("Publishing is unavailable while offline.")
        } else {
            let site = self.model.site.clone();
            let site_summary = site.as_ref().map_or_else(
                || "No site configured".to_owned(),
                |site| format!("{} · {}", site.title, site.base_path),
            );
            let site_setup = match site {
                Some(site) => {
                    let site_target = target.clone();
                    div()
                        .id("publish-site-setup")
                        .child("Site setup")
                        .child(div().child(format!("Title: {}", site.title)))
                        .child(div().child(format!("Base path: {}", site.base_path)))
                        .child(
                            div()
                                .id("publish-site-save")
                                .px_3()
                                .py_2()
                                .bg(gpui::rgb(0x7852ee))
                                .text_color(gpui::rgb(0xffffff))
                                .child("Apply site setup")
                                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                    site_target.update(cx, |surface, cx| {
                                        surface.model.set_site_config(site.clone());
                                        cx.notify();
                                    });
                                }),
                        )
                }
                None => div()
                    .id("publish-site-setup")
                    .child("Site setup is required before publishing."),
            };
            let files = self.model.available_files.clone().into_iter().map(|path| {
                let selected = self.model.selected_files.contains(&path);
                let target = target.clone();
                let id = format!("publish-file-{path}");
                div()
                    .id(ElementId::Name(id.into()))
                    .child(if selected { "☑ " } else { "☐ " })
                    .child(path.clone())
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        target.update(cx, |surface, cx| {
                            surface.model.toggle_file(path.clone());
                            cx.notify();
                        });
                    })
            });
            let preview = self
                .model
                .preview
                .iter()
                .map(|diff| div().child(diff_label(diff)));
            let publish_target = target.clone();
            let unpublish_target = target.clone();
            div()
                .child(site_setup)
                .child(div().id("publish-site-summary").child(site_summary))
                .child(div().mt_4().child("Files").children(files))
                .child(div().mt_4().child("Changes").children(preview))
                .child(
                    div().mt_4().flex().gap_2().children([
                        div().id("publish-action").child("Publish").on_mouse_down(
                            gpui::MouseButton::Left,
                            move |_, _, cx| {
                                publish_target.update(cx, |surface, cx| {
                                    surface.model.request_publish();
                                    cx.notify();
                                });
                            },
                        ),
                        div()
                            .id("unpublish-action")
                            .child("Unpublish")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                unpublish_target.update(cx, |surface, cx| {
                                    surface.model.request_unpublish();
                                    cx.notify();
                                });
                            }),
                    ]),
                )
        };
        div().id("publish-surface").size_full().p_6().child(body)
    }
}
