use tachylyte_navigation_ui::explorer_intents::ExplorerSortMode;
use tachylyte_navigation_ui::{ExplorerIntent, ExplorerModel, ExplorerNodeKind, SortMode};

#[test]
fn explorer_tree_and_typed_interactions_are_data_only() {
    let mut explorer = ExplorerModel::from_vault_paths([
        "Projects/Alpha/readme.md",
        "Projects/Alpha/src/main.rs",
        "Projects/Beta/notes.md",
        "Inbox.md",
    ]);

    let rows = explorer.visible_rows();
    assert_eq!(rows[0].path, "Projects");
    assert!(matches!(rows[0].kind, ExplorerNodeKind::Folder));
    assert!(rows
        .iter()
        .any(|row| row.path == "Projects/Alpha/src/main.rs"));
    assert!(rows.windows(2).all(|pair| {
        pair[0].depth != pair[1].depth
            || !matches!(pair[0].kind, ExplorerNodeKind::File(_))
            || matches!(pair[1].kind, ExplorerNodeKind::File(_))
    }));

    explorer.toggle("Projects/Alpha");
    assert!(!explorer
        .visible_rows()
        .iter()
        .any(|row| row.path == "Projects/Alpha/readme.md"));
    explorer.toggle("Projects/Alpha");
    assert!(explorer
        .visible_rows()
        .iter()
        .any(|row| row.path == "Projects/Alpha/readme.md"));

    explorer.set_filter("main.rs");
    let filtered = explorer.visible_rows();
    assert!(filtered.iter().any(|row| row.path == "Projects"));
    assert!(filtered.iter().any(|row| row.path == "Projects/Alpha"));
    assert!(filtered
        .iter()
        .any(|row| row.path == "Projects/Alpha/src/main.rs"));
    explorer.set_filter("");

    for mode in [SortMode::Name, SortMode::Modified, SortMode::Created] {
        explorer.set_sort_mode(mode);
        assert!(explorer
            .visible_rows()
            .iter()
            .any(|row| row.path == "Projects"));
    }

    explorer.select("Projects/Alpha");
    assert_eq!(explorer.selected.as_deref(), Some("Projects/Alpha"));
    explorer.reduce_keyboard("right");
    explorer.reduce_keyboard("down");
    explorer.reduce_keyboard("left");
    explorer.reduce_keyboard("enter");
    assert_eq!(explorer.active.as_deref(), explorer.selected.as_deref());

    let intents = vec![
        ExplorerIntent::new_note("Projects/Alpha"),
        ExplorerIntent::new_folder("Projects/Alpha"),
        ExplorerIntent::Rename {
            path: "Inbox.md".into(),
            new_name: "Home.md".into(),
        },
        ExplorerIntent::Delete {
            path: "Inbox.md".into(),
        },
        ExplorerIntent::Move {
            path: "Inbox.md".into(),
            destination: "Projects".into(),
        },
        ExplorerIntent::Duplicate {
            path: "Inbox.md".into(),
            destination: "Projects".into(),
        },
        ExplorerIntent::Reveal {
            path: "Inbox.md".into(),
        },
        ExplorerIntent::ContextMenu {
            path: Some("Projects".into()),
        },
        ExplorerIntent::DragMove {
            source: "Inbox.md".into(),
            destination: "Projects".into(),
        },
        ExplorerIntent::SetSort {
            mode: ExplorerSortMode::Kind,
        },
    ];
    assert_eq!(intents[0].label(), "New note");
    assert_eq!(intents[8].label(), "Move");
    assert_eq!(intents.len(), 10);
}
