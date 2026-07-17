use tachylyte_navigation_ui::{FileExplorerModel, FileNode, PaneAction, PaneEvent};

#[test]
fn activate_after_collapse_count_change_stays_in_bounds() {
    let mut explorer = FileExplorerModel::new(vec![
        FileNode {
            id: "docs".into(),
            label: "Docs".into(),
            folder: true,
            children: vec![
                FileNode {
                    id: "guide".into(),
                    label: "Guide".into(),
                    folder: false,
                    children: vec![],
                },
                FileNode {
                    id: "notes".into(),
                    label: "Notes".into(),
                    folder: false,
                    children: vec![],
                },
            ],
        },
        FileNode {
            id: "readme".into(),
            label: "README".into(),
            folder: false,
            children: vec![],
        },
    ]);

    explorer.reduce(PaneAction::End);
    explorer.reduce(PaneAction::Toggle("docs".into()));
    explorer.reduce(PaneAction::Activate);

    assert_eq!(explorer.state.selected, 1);
    assert_eq!(
        explorer.take_events().last(),
        Some(&PaneEvent::Activated("readme".into()))
    );
}
