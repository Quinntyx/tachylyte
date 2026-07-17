use gpui::{div, prelude::*, px, rgb, Context, Render, SharedString, Window};
use serde_json::Value;
use std::cmp::Ordering;
use tachylyte_structured::{evaluate, BaseDocument, Direction, Record};

/// Projection style for a Bases view.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BaseLayout {
    #[default]
    Table,
    Cards,
    List,
}
/// Commands emitted by Bases controls; persistence remains the host's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseCommand {
    Select(Option<usize>),
    Edit {
        row: usize,
        property: String,
        value: Value,
    },
    Sort {
        property: String,
        direction: Direction,
    },
    Filter(String),
    Layout(BaseLayout),
}
/// A stable row projection, preserving source index for commands.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseRow {
    pub source_index: usize,
    pub cells: Vec<(String, Value)>,
}
/// Deterministic projection shared by table, cards, and list renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseProjection {
    pub columns: Vec<String>,
    pub rows: Vec<BaseRow>,
}

/// State and interaction model for a Bases view.
#[derive(Clone, Debug)]
pub struct BaseModel {
    pub document: BaseDocument,
    pub records: Vec<Record>,
    pub layout: BaseLayout,
    pub selected: Option<usize>,
    pub filter: String,
    pub sort: Option<(String, Direction)>,
    pub disabled: bool,
    commands: Vec<BaseCommand>,
}
impl BaseModel {
    /// Build a model from a domain document and records loaded by the host.
    pub fn new(document: BaseDocument, records: Vec<Record>) -> Self {
        Self {
            document,
            records,
            layout: BaseLayout::default(),
            selected: None,
            filter: String::new(),
            sort: None,
            disabled: false,
            commands: Vec::new(),
        }
    }
    /// Build a projection with stable columns and source indices.
    pub fn projection(&self) -> BaseProjection {
        // Keep source identity attached to the borrowed record for the whole
        // pipeline. In particular, never recover an index by record equality:
        // equal records are valid and must remain independently editable.
        let mut rows: Vec<(usize, &Record)> = self.records.iter().enumerate().collect();
        if !self.filter.trim().is_empty() {
            rows.retain(|(_, record)| {
                evaluate(&self.filter, record)
                    .map(|value| value_truthy(&value))
                    .unwrap_or(false)
            });
        }
        if let Some((property, direction)) = &self.sort {
            rows.sort_by(|(_, left), (_, right)| {
                let ordering = compare_values(left.get(property), right.get(property));
                if matches!(direction, Direction::Desc) {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
        let mut columns = self.document.properties.keys().cloned().collect::<Vec<_>>();
        for row in &rows {
            for key in row.1.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
        columns.sort();
        let rows = rows
            .into_iter()
            .map(|(source_index, record)| BaseRow {
                source_index,
                cells: columns
                    .iter()
                    .map(|k| (k.clone(), record.get(k).cloned().unwrap_or(Value::Null)))
                    .collect(),
            })
            .collect();
        BaseProjection { columns, rows }
    }
    /// Select a source row and emit a command.
    pub fn select(&mut self, row: Option<usize>) {
        if !self.disabled {
            self.selected = row;
            self.commands.push(BaseCommand::Select(row));
        }
    }
    /// Emit an editable-cell command.
    pub fn edit_cell(&mut self, row: usize, property: impl Into<String>, value: Value) {
        if !self.disabled {
            self.commands.push(BaseCommand::Edit {
                row,
                property: property.into(),
                value,
            });
        }
    }
    /// Set sorting and emit a command.
    pub fn set_sort(&mut self, property: impl Into<String>, direction: Direction) {
        if !self.disabled {
            let property = property.into();
            self.sort = Some((property.clone(), direction.clone()));
            self.commands.push(BaseCommand::Sort {
                property,
                direction,
            });
        }
    }
    /// Set filtering and emit a command.
    pub fn set_filter(&mut self, expression: impl Into<String>) {
        if !self.disabled {
            let expression = expression.into();
            self.filter = expression.clone();
            self.commands.push(BaseCommand::Filter(expression));
        }
    }
    /// Change projection layout and emit a command.
    pub fn set_layout(&mut self, layout: BaseLayout) {
        if !self.disabled {
            self.layout = layout;
            self.commands.push(BaseCommand::Layout(layout));
        }
    }
    /// Drain emitted commands.
    pub fn take_commands(&mut self) -> Vec<BaseCommand> {
        std::mem::take(&mut self.commands)
    }
}

fn value_truthy(value: &tachylyte_structured::Datum) -> bool {
    match value {
        tachylyte_structured::Datum::Null => false,
        tachylyte_structured::Datum::Bool(value) => *value,
        tachylyte_structured::Datum::Number(value) => *value != 0.,
        tachylyte_structured::Datum::Text(value) => !value.is_empty(),
    }
}

fn compare_values(left: Option<&Value>, right: Option<&Value>) -> Ordering {
    fn rank(value: Option<&Value>) -> u8 {
        match value.unwrap_or(&Value::Null) {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::String(_) => 3,
            _ => 4,
        }
    }
    let ordering = rank(left).cmp(&rank(right));
    if ordering != Ordering::Equal {
        return ordering;
    }
    match (left.unwrap_or(&Value::Null), right.unwrap_or(&Value::Null)) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Number(a), Value::Number(b)) => a
            .as_f64()
            .unwrap_or(0.)
            .partial_cmp(&b.as_f64().unwrap_or(0.))
            .unwrap_or(Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (a, b) => a.to_string().cmp(&b.to_string()),
    }
}

/// Native GPUI renderer for table, card, and list projections.
pub struct BasesView {
    pub model: BaseModel,
}
impl BasesView {
    /// Construct a Bases view.
    pub fn new(model: BaseModel) -> Self {
        Self { model }
    }
}
impl Render for BasesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let projection = self.model.projection();
        let selected = self.model.selected;
        let columns = projection.columns.clone();
        let row_count = projection.rows.len();
        let mut body = div().flex().flex_col().p_2().bg(rgb(0xffffffd9));
        if !columns.is_empty() {
            let headings = columns
                .iter()
                .map(|column| format!("▦  {column}"))
                .collect::<Vec<_>>()
                .join("     ");
            body = body.child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .px_2()
                    .text_color(rgb(0x5c5c5cff))
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0ff))
                    .child(headings),
            );
        }
        for row in projection.rows {
            let active = selected == Some(row.source_index);
            let text = row
                .cells
                .iter()
                .map(|(k, v)| format!("{k}: {}", value_text(v)))
                .collect::<Vec<_>>()
                .join("   ");
            let e = entity.clone();
            let index = row.source_index;
            let child = div()
                .id(SharedString::from(format!("base-row-{index}")))
                .w_full()
                .h(px(28.))
                .flex()
                .items_center()
                .px_2()
                .border_b_1()
                .border_color(rgb(0xe0e0e0ff))
                .bg(rgb(if active { 0xeee7f7ff } else { 0xffffffff }))
                .text_color(rgb(if active { 0x6b3fa0ff } else { 0x222222ff }))
                .hover(|style| style.bg(rgb(0xeeeeeeff)))
                .child(text)
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    e.update(cx, |view, cx| {
                        view.model.select(Some(index));
                        cx.notify();
                    });
                });
            body = body.child(child);
        }
        if row_count == 0 {
            body = body.child(
                div()
                    .p_2()
                    .text_color(rgb(0x5c5c5cff))
                    .child("No records yet"),
            );
        }
        let e = entity.clone();
        let cards = entity.clone();
        let list = entity.clone();
        let sort = entity.clone();
        let filter = entity.clone();
        let disabled = self.model.disabled;
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0xf6f6f6ff))
            .text_color(rgb(0x222222ff))
            .child(
                div()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(rgb(0xffffffff))
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0ff))
                    .child(if disabled {
                        "▦  Bases (disabled)"
                    } else {
                        "▦  Bases"
                    })
                    .child(
                        div()
                            .id("bases-table")
                            .h(px(28.))
                            .px_2()
                            .items_center()
                            .hover(|s| s.bg(rgb(0xeeeeeeff)))
                            .child("▤ Table")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                e.update(cx, |v, cx| {
                                    v.model.set_layout(BaseLayout::Table);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("bases-cards")
                            .h(px(28.))
                            .px_2()
                            .items_center()
                            .hover(|s| s.bg(rgb(0xeeeeeeff)))
                            .child("▦ Cards")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                cards.update(cx, |v, cx| {
                                    v.model.set_layout(BaseLayout::Cards);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("bases-list")
                            .h(px(28.))
                            .px_2()
                            .items_center()
                            .hover(|s| s.bg(rgb(0xeeeeeeff)))
                            .child("☷ List")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                list.update(cx, |v, cx| {
                                    v.model.set_layout(BaseLayout::List);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("bases-filter")
                            .h(px(28.))
                            .px_2()
                            .items_center()
                            .border_1()
                            .border_color(rgb(0xe0e0e0ff))
                            .bg(rgb(0xfafafaff))
                            .child(if self.model.filter.is_empty() {
                                "⌕  Filter"
                            } else {
                                "⌕  Filtered"
                            })
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                filter.update(cx, |v, cx| {
                                    v.model.set_filter("");
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("bases-sort")
                            .h(px(28.))
                            .px_2()
                            .items_center()
                            .hover(|s| s.bg(rgb(0xeeeeeeff)))
                            .child("↕ Sort")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                sort.update(cx, |v, cx| {
                                    if let Some(property) =
                                        v.model.projection().columns.first().cloned()
                                    {
                                        v.model.set_sort(property, Direction::Asc);
                                    }
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(body)
    }
}
fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::String(v) => v.clone(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_and_commands_are_deterministic() {
        let doc = BaseDocument::default();
        let mut a = Record::new();
        a.insert("name".into(), Value::String("A".into()));
        let mut b = Record::new();
        b.insert("name".into(), Value::String("B".into()));
        let mut m = BaseModel::new(doc, vec![a, b]);
        assert_eq!(m.projection().columns, vec!["name"]);
        assert_eq!(m.projection().rows[0].source_index, 0);
        m.select(Some(1));
        m.edit_cell(1, "name", Value::String("C".into()));
        assert_eq!(m.take_commands().len(), 2);
    }
    #[test]
    fn disabled_state_blocks_edit_and_selection() {
        let mut m = BaseModel::new(BaseDocument::default(), vec![]);
        m.disabled = true;
        m.select(Some(0));
        m.set_filter("true");
        assert!(m.take_commands().is_empty());
    }

    #[test]
    fn equal_records_keep_distinct_source_indices_through_filter_and_sort() {
        let mut first = Record::new();
        first.insert("name".into(), Value::String("same".into()));
        let second = first.clone();
        let mut model = BaseModel::new(BaseDocument::default(), vec![first, second]);
        model.set_filter("name = \"same\"");
        model.set_sort("name", Direction::Asc);
        let projection = model.projection();
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|row| row.source_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        model.edit_cell(
            projection.rows[1].source_index,
            "name",
            Value::String("edited".into()),
        );
        assert_eq!(
            model.take_commands().last(),
            Some(&BaseCommand::Edit {
                row: 1,
                property: "name".into(),
                value: Value::String("edited".into())
            })
        );
    }
}
