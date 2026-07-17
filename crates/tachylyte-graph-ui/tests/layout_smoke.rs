use tachylyte_graph_ui::{GraphViewModel, NodeStyle, Point};
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
}
