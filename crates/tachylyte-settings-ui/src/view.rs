//! GPUI view for the neutral settings model.

use crate::model::{Category, Settings, Theme};
use gpui::{div, prelude::*, px, rgb, Context, ElementId, FocusHandle, Render, Window};

/// A mountable settings window.  State and events belong to [`Settings`], so a
/// host can snapshot or drain them without depending on GPUI.
pub struct SettingsWindow {
    pub model: Settings,
    focus_handle: FocusHandle,
}

impl SettingsWindow {
    pub fn new(model: Settings, cx: &mut Context<Self>) -> Self {
        Self {
            model,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn mount(model: Settings, cx: &mut Context<Self>) -> gpui::Entity<Self> {
        cx.new(|cx| Self::new(model, cx))
    }

    pub fn snapshot(&self) -> Settings {
        self.model.clone()
    }
    pub fn drain_events(&mut self) -> Vec<crate::model::SettingsEvent> {
        self.model.drain_events()
    }
    pub fn set_search(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        self.model.set_search(value);
        cx.notify();
    }
}

fn toggle(
    label: &'static str,
    value: bool,
    id: &'static str,
    target: gpui::Entity<SettingsWindow>,
    action: &'static str,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(0xe0e0e0))
        .child(label)
        .child(
            div()
                .id(ElementId::Name(format!("toggle-control-{id}").into()))
                .px_2()
                .py_1()
                .bg(rgb(if value { 0x7852ee } else { 0xe0e0e0 }))
                .text_color(rgb(if value { 0xffffff } else { 0x222222 }))
                .child(if value { "On" } else { "Off" })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    target.update(cx, |window, cx| {
                        match action {
                            "spellcheck" => window.model.toggle_spellcheck(),
                            "readable" => window.model.toggle_readable_line_length(),
                            "strict" => window.model.toggle_strict_line_breaks(),
                            "fold-heading" => window.model.toggle_fold_heading(),
                            "fold-indent" => window.model.toggle_fold_indent(),
                            "confirm-delete" => {
                                let mut v = window.model.files_and_links;
                                v.confirm_file_deletion = !v.confirm_file_deletion;
                                window.model.set_files_and_links(v);
                            }
                            _ => {}
                        };
                        cx.notify();
                    });
                }),
        )
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let category = self.model.category;
        let title = category.label();
        let sidebar = Category::ALL.into_iter().map(|item| {
            let selected = item == category;
            let target = target.clone();
            div()
                .id(ElementId::Name(format!("category-{}", item.label()).into()))
                .px_3()
                .py_2()
                .text_color(rgb(if selected { 0x7852ee } else { 0x222222 }))
                .bg(rgb(if selected { 0xf6f6f6 } else { 0xffffff }))
                .child(item.label())
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    target.update(cx, |w, cx| {
                        w.model.set_category(item);
                        cx.notify();
                    })
                })
        });
        let search = self.model.search().to_owned();
        let content: gpui::Div = match category {
            Category::About => div()
                .child("Tachylyte")
                .child("A fast, local knowledge workspace."),
            Category::Appearance => {
                let themes = [Theme::Light, Theme::System, Theme::Dark]
                    .into_iter()
                    .map(|theme| {
                        let active = self.model.theme == theme;
                        let target = target.clone();
                        div()
                            .id(match theme {
                                Theme::Light => "theme-light",
                                Theme::System => "theme-system",
                                Theme::Dark => "theme-dark",
                            })
                            .px_3()
                            .py_2()
                            .border_1()
                            .border_color(rgb(if active { 0x7852ee } else { 0xe0e0e0 }))
                            .bg(rgb(if active { 0xf6f6f6 } else { 0xffffff }))
                            .child(format!("{:?}", theme))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                target.update(cx, |w, cx| {
                                    w.model.set_theme(theme);
                                    cx.notify();
                                })
                            })
                    });
                let accent_target = target.clone();
                div()
                    .child("Theme")
                    .child(div().flex().gap_2().mt_2().children(themes))
                    .child(
                        div()
                            .id("accent-control")
                            .mt_4()
                            .px_3()
                            .py_2()
                            .border_1()
                            .border_color(rgb(0xe0e0e0))
                            .child(format!("Accent: {}", self.model.accent))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                accent_target.update(cx, |window, cx| {
                                    window.model.set_accent("#7c3aed");
                                    cx.notify();
                                });
                            }),
                    )
            }
            Category::Editor => {
                let e = self.model.editor;
                div().child("Editor").children([
                    toggle(
                        "Spellcheck",
                        e.spellcheck,
                        "spellcheck",
                        target.clone(),
                        "spellcheck",
                    ),
                    toggle(
                        "Readable line length",
                        e.readable_line_length,
                        "readable",
                        target.clone(),
                        "readable",
                    ),
                    toggle(
                        "Strict line breaks",
                        e.strict_line_breaks,
                        "strict",
                        target.clone(),
                        "strict",
                    ),
                    toggle(
                        "Fold headings",
                        e.fold_heading,
                        "fold-heading",
                        target.clone(),
                        "fold-heading",
                    ),
                    toggle(
                        "Fold indentation",
                        e.fold_indent,
                        "fold-indent",
                        target.clone(),
                        "fold-indent",
                    ),
                ])
            }
            Category::FilesAndLinks => div()
                .child("Files and links")
                .child(format!(
                    "New notes: {:?}",
                    self.model.files_and_links.default_new_note_location
                ))
                .child(format!(
                    "New links: {:?}",
                    self.model.files_and_links.new_link_format
                ))
                .child(toggle(
                    "Confirm file deletion",
                    self.model.files_and_links.confirm_file_deletion,
                    "confirm-delete",
                    target.clone(),
                    "confirm-delete",
                )),
            Category::Hotkeys => div()
                .child("Hotkeys")
                .child(div().child(format!("Search: {}", search)))
                .children(self.model.filtered_hotkeys().into_iter().map(|h| {
                    div()
                        .py_2()
                        .border_b_1()
                        .border_color(rgb(0xe0e0e0))
                        .child(format!(
                            "{}    {}",
                            h.label,
                            h.shortcut.as_deref().unwrap_or("Unset")
                        ))
                })),
            Category::CorePlugins => {
                div()
                    .child("Core plugins")
                    .children(self.model.filtered_plugins().into_iter().map(|p| {
                        let id = p.id;
                        let label = p.label;
                        let enabled = p.enabled;
                        let target = target.clone();
                        div()
                            .id(ElementId::Name(format!("plugin-{id}").into()))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0xe0e0e0))
                            .child(label)
                            .child(
                                div()
                                    .id(ElementId::Name(format!("plugin-toggle-{id}").into()))
                                    .px_2()
                                    .py_1()
                                    .bg(rgb(if enabled { 0x7852ee } else { 0xe0e0e0 }))
                                    .text_color(rgb(if enabled { 0xffffff } else { 0x222222 }))
                                    .child(if enabled { "On" } else { "Off" })
                                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                        target.update(cx, |w, cx| {
                                            w.model.toggle_plugin(id);
                                            cx.notify();
                                        })
                                    }),
                            )
                    }))
            }
        };
        let search_target = target.clone();
        let search_box = div()
            .id("settings-search")
            .focusable()
            .tab_index(0)
            .key_context("settings-search")
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .bg(rgb(0xffffff))
            .px_3()
            .py_2()
            .text_color(rgb(0x222222))
            .on_mouse_down(gpui::MouseButton::Left, {
                let focus_handle = self.focus_handle.clone();
                move |_, window, _| focus_handle.focus(window)
            })
            .child(if search.is_empty() {
                "⌕  Search settings".to_owned()
            } else {
                search.clone()
            })
            .on_key_down(move |event, _, cx| {
                search_target.update(cx, |window, cx| {
                    let current = window.model.search().to_owned();
                    let next = if event.keystroke.key == "backspace" {
                        current
                            .char_indices()
                            .next_back()
                            .map_or_else(String::new, |(index, _)| current[..index].to_owned())
                    } else if let Some(character) = &event.keystroke.key_char {
                        if !character.chars().any(char::is_control)
                            && !event.keystroke.modifiers.control
                            && !event.keystroke.modifiers.platform
                            && !event.keystroke.modifiers.alt
                        {
                            format!("{current}{character}")
                        } else {
                            current.clone()
                        }
                    } else {
                        current.clone()
                    };
                    if next != current {
                        window.model.set_search(next);
                        cx.notify();
                    }
                });
            });
        let close_target = target.clone();
        div()
            .id("settings-window")
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(52.))
                    .px_5()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0))
                    .child("Settings")
                    .child(
                        div()
                            .id("settings-close")
                            .px_2()
                            .py_1()
                            .child("Close")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                close_target.update(cx, |w, cx| {
                                    w.model.request_close();
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(div().w(px(190.)).bg(rgb(0xf6f6f6)).p_3().children(sidebar))
                    .child(
                        div()
                            .flex_1()
                            .p_6()
                            .child(div().text_xl().child(title))
                            .child(search_box)
                            .child(div().mt_4().child(content)),
                    ),
            )
    }
}
