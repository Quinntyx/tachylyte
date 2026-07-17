//! End-to-end contract for the public workspace interaction model.
//!
//! Keep one compact reducer scenario covering the adapter-facing behavior.

mod intended_workspace_behavior {
    use tachylyte_workspace::{LayoutNode, Orientation, View, Workspace};

    #[test]
    fn combined_workspace_session_round_trips() {
        let mut workspace = Workspace::default();

        // Open a reusable path, then use the modifier to request a new tab.
        let editor = workspace.open_reusable_path("src/main.rs");
        assert_eq!(workspace.open_reusable_path("src/main.rs"), editor);
        let forced =
            workspace.open_with_modifier(View::new("markdown").with_path("src/main.rs"), true);
        assert_ne!(forced, editor);
        let graph = workspace.open_with_modifier(View::new("graph"), true);
        let settings = workspace.open(View::new("settings"));
        let media = workspace.open(View::new("media"));

        // Pin and reorder tabs, split the active area, move the graph, and focus it.
        workspace.pin(&editor, true);
        workspace.reorder(&media, 0);
        let split_group = workspace.active_group_id().expect("active group");
        workspace.split(&editor, Orientation::Horizontal);
        workspace.move_to_group(&graph, &split_group);
        workspace.focus(&graph);

        // Browser-like history, duplicate, and close/reopen behavior.
        workspace.back();
        workspace.forward();
        let duplicate = workspace.duplicate(&graph);
        workspace.close(&duplicate);
        workspace.reopen_last_closed();

        // Typed view kinds remain present and participate in feature/settings state.
        assert!(workspace.view_kinds().contains("graph"));
        assert!(workspace.view_kinds().contains("settings"));
        assert!(workspace.view_kinds().contains("media"));

        // The resulting layout is split and the whole state is serde-restorable.
        assert!(matches!(
            workspace.windows[0].root,
            LayoutNode::Split { .. }
        ));
        let encoded = serde_json::to_string(&workspace).expect("serialize workspace");
        let restored: Workspace = serde_json::from_str(&encoded).expect("restore workspace");
        assert_eq!(workspace, restored);

        // Keep bindings live in this contract until the API settles.
        let _ = settings;
    }
}
