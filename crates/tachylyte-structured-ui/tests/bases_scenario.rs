use serde_json::json;
use tachylyte_structured::{BaseDocument, Direction, Property, Record};
use tachylyte_structured_ui::{BaseCommand, BaseLayout, BaseModel};

fn record(name: &str, score: i64) -> Record {
    [("name".into(), json!(name)), ("score".into(), json!(score))]
        .into_iter()
        .collect()
}

#[test]
fn bases_projection_keeps_source_identity_across_filter_and_sort() {
    let mut document = BaseDocument::default();
    document
        .properties
        .insert("name".into(), Property::Text(String::new()));
    document
        .properties
        .insert("score".into(), Property::Number(0.));
    // Formula display and summary helpers are not part of the crate's public
    // API yet; keep this scenario limited to the currently exposed surface.

    let mut model = BaseModel::new(
        document,
        vec![record("first", 2), record("second", 1), record("third", 3)],
    );

    model.set_filter("score > 1");
    model.set_sort("score", Direction::Desc);
    let projection = model.projection();
    assert_eq!(
        projection
            .rows
            .iter()
            .map(|row| row.source_index)
            .collect::<Vec<_>>(),
        vec![2, 0]
    );

    model.set_layout(BaseLayout::Cards);
    model.set_layout(BaseLayout::List);
    model.edit_cell(2, "name", json!("updated"));
    assert_eq!(model.layout, BaseLayout::List);
    assert_eq!(
        model.take_commands(),
        vec![
            BaseCommand::Filter("score > 1".into()),
            BaseCommand::Sort {
                property: "score".into(),
                direction: Direction::Desc,
            },
            BaseCommand::Layout(BaseLayout::Cards),
            BaseCommand::Layout(BaseLayout::List),
            BaseCommand::Edit {
                row: 2,
                property: "name".into(),
                value: json!("updated"),
            },
        ]
    );
}
