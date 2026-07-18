use tachylyte_structured::{CanvasDocument, Edge, Point, Size};
use tachylyte_structured_ui::{CanvasCommand, CanvasMode, CanvasModel, ScreenPoint};

#[test]
fn canvas_scenario_preserves_domain_fields_while_emitting_intents() {
    let source = r#"
    {
      "nodes": [
        {"id":"text","type":"text","x":0,"y":0,"width":80,"height":40,"text":"Hello","futureNode":true},
        {"id":"file","type":"file","x":120,"y":0,"width":80,"height":40,"file":"notes.md"},
        {"id":"link","type":"link","x":0,"y":80,"width":80,"height":40,"url":"https://example.com"},
        {"id":"group","type":"group","x":120,"y":80,"width":160,"height":100}
      ],
      "edges": [{"id":"existing","fromNode":"text","toNode":"file","futureEdge":{"kind":"dashed"}}],
      "futureDocument": {"version": 2}
    }
    "#;
    let mut document = CanvasDocument::from_json(source).unwrap();
    let mut model = CanvasModel::new(document.clone());

    assert_eq!(
        model.select_at(ScreenPoint { x: 20.0, y: 20.0 }).as_deref(),
        Some("text")
    );
    model.set_mode(CanvasMode::Connect);
    assert_eq!(
        model
            .pointer_down(ScreenPoint { x: 140.0, y: 20.0 })
            .as_deref(),
        Some("file")
    );
    model.pan(ScreenPoint { x: 10.0, y: 5.0 });
    model.zoom(ScreenPoint { x: 100.0, y: 100.0 }, 2.0);
    model.move_intent("text", ScreenPoint { x: 50.0, y: 60.0 });
    model.resize_intent(
        "file",
        Size {
            width: 96.0,
            height: 48.0,
        },
    );

    let commands = model.take_commands();
    assert!(commands
        .iter()
        .any(|command| { matches!(command, CanvasCommand::Select(Some(id)) if id == "text") }));
    assert!(commands.iter().any(|command| {
        matches!(command, CanvasCommand::Connect(edge) if edge.from_node == "text" && edge.to_node == "file")
    }));
    assert!(commands
        .iter()
        .any(|command| matches!(command, CanvasCommand::Pan(_))));
    assert!(commands
        .iter()
        .any(|command| matches!(command, CanvasCommand::Zoom { .. })));
    assert!(commands
        .iter()
        .any(|command| { matches!(command, CanvasCommand::Move { id, .. } if id == "text") }));
    assert!(commands
        .iter()
        .any(|command| { matches!(command, CanvasCommand::Resize { id, .. } if id == "file") }));

    // CanvasModel emits intents and does not mutate the host-owned domain snapshot.
    document = model.document;
    let round_trip =
        serde_json::from_str::<serde_json::Value>(&document.to_json().unwrap()).unwrap();
    assert_eq!(round_trip["futureDocument"]["version"], 2);
    assert_eq!(round_trip["nodes"][0]["futureNode"], true);
    assert_eq!(round_trip["edges"][0]["futureEdge"]["kind"], "dashed");
    assert_eq!(
        document.node("link").unwrap().url.as_deref(),
        Some("https://example.com")
    );
    assert_eq!(document.node("group").unwrap().kind, "group");
    assert_eq!(
        document.edges[0],
        Edge {
            id: "existing".into(),
            from_node: "text".into(),
            to_node: "file".into(),
            extra: [("futureEdge".into(), serde_json::json!({"kind": "dashed"}))]
                .into_iter()
                .collect(),
            ..Default::default()
        }
    );
    assert!(document
        .node("text")
        .unwrap()
        .rect()
        .contains(Point { x: 20.0, y: 20.0 }));
}
