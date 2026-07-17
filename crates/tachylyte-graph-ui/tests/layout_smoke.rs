use tachylyte_graph_ui::{GraphViewModel, NodeStyle, Point, ViewTransform};
use tachylyte_knowledge::{Document, VaultIndex};

#[test]
fn layout_is_deterministic_with_resolved_and_unresolved_links() {
    let mut index = VaultIndex::new();
    index.upsert(Document {
        path: "a.md".into(),
        content: "[[b.md]] [[missing]]".into(),
        modified: 0,
        ..Document::default()
    });
    index.upsert(Document {
        path: "b.md".into(),
        ..Document::default()
    });

    let model = GraphViewModel::new(&index);
    let repeat = GraphViewModel::new(&index);

    assert_eq!(
        model
            .nodes
            .iter()
            .map(|n| n.node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a.md", "b.md", "unresolved:missing"]
    );
    assert_eq!(model.nodes[0].position, Point { x: 120.0, y: 0.0 });
    assert_eq!(
        model.nodes.iter().map(|n| n.position).collect::<Vec<_>>(),
        repeat.nodes.iter().map(|n| n.position).collect::<Vec<_>>()
    );
    assert_eq!(model.nodes[2].style, NodeStyle::Unresolved);

    assert_eq!(model.edges.len(), 2);
    assert_eq!(model.edges[0].edge.from, "a.md");
    assert_eq!(model.edges[0].edge.to, "b.md");
    assert!(!model.edges[0].edge.unresolved);
    assert_eq!(model.edges[0].from, model.nodes[0].position);
    assert_eq!(model.edges[0].to, model.nodes[1].position);
    assert_eq!(model.edges[1].edge.to, "unresolved:missing");
    assert!(model.edges[1].edge.unresolved);
    assert_eq!(model.edges[1].from, model.nodes[0].position);
    assert_eq!(model.edges[1].to, model.nodes[2].position);

    let transform = ViewTransform::new(Point { x: 37.5, y: -18.0 }, 2.25);
    for world in [
        Point { x: 0.0, y: 0.0 },
        Point { x: -12.5, y: 8.25 },
        Point {
            x: 1_000.0,
            y: -750.0,
        },
    ] {
        let screen = transform.screen(world);
        assert!(screen.x.is_finite() && screen.y.is_finite());
        let round_trip = transform.world(screen);
        assert!((round_trip.x - world.x).abs() < 1e-4);
        assert!((round_trip.y - world.y).abs() < 1e-4);
    }

    let safe = transform.screen(Point {
        x: f32::NAN,
        y: f32::INFINITY,
    });
    assert!(safe.x.is_finite() && safe.y.is_finite());

    let mut zoom = ViewTransform::default();
    for factor in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        zoom.zoom_by(factor);
        assert_eq!(zoom.zoom(), 1.0);
        assert!(zoom.screen(Point { x: 4.0, y: -2.0 }).x.is_finite());
    }
    for value in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut transform = ViewTransform::new(Point::default(), value);
        assert!(transform.zoom().is_finite() && transform.zoom() > 0.0);
        transform.set_zoom(value);
        assert!(transform.zoom().is_finite() && transform.zoom() > 0.0);
    }
}
