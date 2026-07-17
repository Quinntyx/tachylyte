//! Small, filesystem-free workflow surfaces.
use crate::model::{ListKey, RowListState, WorkflowIntent};
use gpui::{div, prelude::*, px, rgb, Context, IntoElement, KeyDownEvent, Render, Window};

fn key(event: &KeyDownEvent) -> Option<ListKey> {
    match event.keystroke.key.as_str() {
        "up" => Some(ListKey::Up),
        "down" => Some(ListKey::Down),
        "home" => Some(ListKey::Home),
        "end" => Some(ListKey::End),
        "enter" => Some(ListKey::Enter),
        _ => None,
    }
}
fn panel(title: &str, query: &str, rows: Vec<String>, selected: Option<usize>) -> impl IntoElement {
    let mut body = div().flex().flex_col().gap(px(2.0)).p(px(10.0));
    body = body.child(
        div()
            .text_sm()
            .text_color(rgb(0x353535))
            .child(title.to_owned()),
    );
    body = body.child(
        div()
            .text_xs()
            .text_color(rgb(0x777777))
            .child(if query.is_empty() {
                "Type to filter".into()
            } else {
                query.to_owned()
            }),
    );
    for (index, row) in rows.into_iter().enumerate() {
        body = body.child(
            div()
                .px(px(6.0))
                .py(px(4.0))
                .bg(rgb(if selected == Some(index) {
                    0xe8e5df
                } else {
                    0xf7f7f5
                }))
                .text_color(rgb(0x303030))
                .child(row),
        );
    }
    div()
        .w(px(360.0))
        .bg(rgb(0xfffffd))
        .border_1()
        .border_color(rgb(0xd8d8d2))
        .rounded(px(6.0))
        .shadow_lg()
        .child(body)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathOption {
    pub path: String,
    pub label: String,
}
impl AsRef<str> for PathOption {
    fn as_ref(&self) -> &str {
        &self.label
    }
}

pub struct QuickSwitcher {
    list: RowListState<PathOption>,
}
impl QuickSwitcher {
    pub fn new(options: Vec<PathOption>) -> Self {
        Self {
            list: RowListState::new(options),
        }
    }
    pub fn update(&mut self, options: Vec<PathOption>) {
        self.list.rows = options;
        self.list.selected = None;
    }
    pub fn query(&self) -> &str {
        &self.list.query
    }
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.list.set_query(query);
    }
    pub fn selected(&self) -> Option<&PathOption> {
        self.list.selected_row()
    }
    pub fn handle_key(&mut self, key: ListKey) {
        let intent = self
            .list
            .key(key)
            .map(|row| WorkflowIntent::open_path(row.path.clone()));
        if let Some(intent) = intent {
            self.list.push_intent(intent);
        }
    }
    pub fn handle_key_event(&mut self, event: &KeyDownEvent) {
        if let Some(key) = key(event) {
            self.handle_key(key);
        }
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        self.list.take_intents()
    }
}
impl Render for QuickSwitcher {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let indices = self.list.filtered_indices();
        let selected = self
            .list
            .selected
            .and_then(|index| indices.iter().position(|&value| value == index));
        panel(
            "Quick switcher",
            &self.list.query,
            self.list
                .filtered_rows()
                .into_iter()
                .map(|x| x.label.clone())
                .collect(),
            selected,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOption {
    pub id: String,
    pub title: String,
}
impl AsRef<str> for CommandOption {
    fn as_ref(&self) -> &str {
        &self.title
    }
}
pub struct CommandPalette {
    list: RowListState<CommandOption>,
}
impl CommandPalette {
    pub fn new(commands: Vec<CommandOption>) -> Self {
        Self {
            list: RowListState::new(commands),
        }
    }
    pub fn update(&mut self, commands: Vec<CommandOption>) {
        self.list.rows = commands;
        self.list.selected = None;
    }
    pub fn query(&self) -> &str {
        &self.list.query
    }
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.list.set_query(query);
    }
    pub fn selected(&self) -> Option<&CommandOption> {
        self.list.selected_row()
    }
    pub fn handle_key(&mut self, key: ListKey) {
        let intent = self
            .list
            .key(key)
            .map(|row| WorkflowIntent::run_command(row.id.clone()));
        if let Some(intent) = intent {
            self.list.push_intent(intent);
        }
    }
    pub fn handle_key_event(&mut self, event: &KeyDownEvent) {
        if let Some(key) = key(event) {
            self.handle_key(key);
        }
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        self.list.take_intents()
    }
}
impl Render for CommandPalette {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let indices = self.list.filtered_indices();
        let selected = self
            .list
            .selected
            .and_then(|index| indices.iter().position(|&value| value == index));
        panel(
            "Command palette",
            &self.list.query,
            self.list
                .filtered_rows()
                .into_iter()
                .map(|x| x.title.clone())
                .collect(),
            selected,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateOption {
    pub id: String,
    pub name: String,
    pub content: String,
}
impl AsRef<str> for TemplateOption {
    fn as_ref(&self) -> &str {
        &self.name
    }
}
pub struct TemplatePicker {
    list: RowListState<TemplateOption>,
}
impl TemplatePicker {
    pub fn new(templates: Vec<TemplateOption>) -> Self {
        Self {
            list: RowListState::new(templates),
        }
    }
    pub fn update(&mut self, templates: Vec<TemplateOption>) {
        self.list.rows = templates;
        self.list.selected = None;
    }
    pub fn query(&self) -> &str {
        &self.list.query
    }
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.list.set_query(query);
    }
    pub fn selected(&self) -> Option<&TemplateOption> {
        self.list.selected_row()
    }
    pub fn handle_key(&mut self, key: ListKey) {
        let intent = self
            .list
            .key(key)
            .map(|row| WorkflowIntent::choose_template(row.id.clone()));
        if let Some(intent) = intent {
            self.list.push_intent(intent);
        }
    }
    pub fn handle_key_event(&mut self, event: &KeyDownEvent) {
        if let Some(key) = key(event) {
            self.handle_key(key);
        }
    }
    pub fn take_intents(&mut self) -> Vec<WorkflowIntent> {
        self.list.take_intents()
    }
}
impl Render for TemplatePicker {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let indices = self.list.filtered_indices();
        let selected = self
            .list
            .selected
            .and_then(|index| indices.iter().position(|&value| value == index));
        panel(
            "Template picker",
            &self.list.query,
            self.list
                .filtered_rows()
                .into_iter()
                .map(|x| x.name.clone())
                .collect(),
            selected,
        )
    }
}
