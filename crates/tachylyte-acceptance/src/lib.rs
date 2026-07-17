//! Acceptance tests deliberately use only public crate APIs.  The fixtures model a
//! small, believable work vault rather than isolated unit-test strings.

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use tachylyte_core::{FileKind, Vault, VaultPath};
    use tachylyte_knowledge::{
        backlinks, graph, links, search, Document, GraphFilter, Task, VaultIndex,
    };
    use tachylyte_markdown::{Document as MarkdownDocument, EditorDocument};
    use tachylyte_services::{auth, web, Secret};
    use tachylyte_structured::{BaseDocument, CanvasDocument, Point};
    use tachylyte_workflows::{
        daily_note_plan, render_template, retention_plan, DailyNoteConfig, RecoveryPlan,
        Sha256Digest, Snapshot,
    };
    use tachylyte_workspace::{Action, Effect, View, Workspace};

    const HOME: &str = include_str!("../fixtures/Home.md");
    const PROJECT: &str = include_str!("../fixtures/Projects/River.md");
    const CANVAS: &str = include_str!("../fixtures/Planning.canvas");
    const BASE: &str = include_str!("../fixtures/Projects.base");

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        vault
            .write(&VaultPath::new("Home.md").unwrap(), HOME.as_bytes())
            .unwrap();
        vault
            .write(
                &VaultPath::new("Projects/River.md").unwrap(),
                PROJECT.as_bytes(),
            )
            .unwrap();
        vault
            .write(
                &VaultPath::new("Planning.canvas").unwrap(),
                CANVAS.as_bytes(),
            )
            .unwrap();
        vault
            .write(&VaultPath::new("Projects.base").unwrap(), BASE.as_bytes())
            .unwrap();
        (dir, vault)
    }

    #[test]
    fn vault_scan_read_write_rename_and_trash_are_composable() {
        let (_dir, vault) = vault();
        let entries = vault.scan().unwrap();
        assert!(entries
            .iter()
            .any(|e| e.path.to_string() == "Home.md" && e.kind == FileKind::Markdown));
        assert_eq!(
            vault.read(&VaultPath::new("Home.md").unwrap()).unwrap(),
            HOME.as_bytes()
        );
        let draft = VaultPath::new("Projects/Draft.md").unwrap();
        vault.create(&draft, b"# Draft\n").unwrap();
        vault.write(&draft, b"# Revised draft\n").unwrap();
        let renamed = VaultPath::new("Projects/Revised.md").unwrap();
        vault.rename(&draft, &renamed).unwrap();
        let trashed = vault.trash(&renamed).unwrap();
        assert_eq!(trashed.to_string(), ".trash/Revised.md");
        assert!(vault.scan().unwrap().iter().all(|e| e.path != trashed));
    }

    #[test]
    fn markdown_edit_save_and_reparse_preserves_semantics() {
        let (_dir, vault) = vault();
        let path = VaultPath::new("Home.md").unwrap();
        let parsed =
            MarkdownDocument::parse(String::from_utf8(vault.read(&path).unwrap()).unwrap());
        assert_eq!(parsed.outline().headings[0].slug, "home");
        assert_eq!(parsed.wikilinks()[0].target, "Projects/River");
        let mut editor = EditorDocument::new(parsed.source());
        let heading = editor.document().outline().headings[0].span;
        editor.edit(heading, "# Home base").unwrap();
        vault.write(&path, editor.source().as_bytes()).unwrap();
        let reparsed =
            MarkdownDocument::parse(String::from_utf8(vault.read(&path).unwrap()).unwrap());
        assert_eq!(reparsed.outline().headings[0].text, "Home base");
        assert!(editor.undo());
    }

    fn index() -> VaultIndex {
        let mut index = VaultIndex::new();
        index.upsert(Document {
            path: "Home.md".into(),
            content: "# Home\n\nThe weekly review links to [[River|the river plan]]. #work".into(),
            modified: 2,
            tags: vec!["work".into()],
            properties: [("status".into(), "active".into())].into_iter().collect(),
            tasks: vec![Task {
                text: "Review river plan".into(),
                done: false,
            }],
        });
        index.upsert(Document {
            path: "Projects/River.md".into(),
            content: PROJECT.into(),
            modified: 1,
            tags: vec!["project".into()],
            properties: BTreeMap::new(),
            tasks: vec![],
        });
        index
    }

    #[test]
    fn index_search_backlinks_and_graph_share_link_resolution() {
        let index = index();
        let results = search(&index, "tag:work").unwrap();
        assert_eq!(results[0].path, "Home.md");
        assert_eq!(links(&index, "Home.md")[0].target, "River");
        assert_eq!(backlinks(&index, "Projects/River.md")[0].source, "Home.md");
        let (nodes, edges) = graph(
            &index,
            &GraphFilter {
                include_unresolved: true,
                ..Default::default()
            },
        );
        assert_eq!(nodes.len(), 2);
        assert!(edges
            .iter()
            .any(|e| e.from == "Home.md" && e.to == "Projects/River.md"));
    }

    struct SafeAdapter<'a> {
        vault: &'a Vault,
        snapshots: Vec<Snapshot>,
    }
    impl SafeAdapter<'_> {
        fn apply_daily(&self, plan: &tachylyte_workflows::DailyNotePlan) {
            if plan.create {
                self.vault
                    .create(
                        &VaultPath::new(&plan.path).unwrap(),
                        plan.content.as_deref().unwrap_or("").as_bytes(),
                    )
                    .unwrap();
            }
        }
        fn apply_recovery(&mut self, plan: RecoveryPlan) {
            self.snapshots = plan.retain;
        }
    }

    #[test]
    fn daily_template_and_recovery_plans_apply_through_safe_adapter() {
        let (_dir, vault) = vault();
        let now = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 6, 7, 9, 30, 0)
            .unwrap();
        let config = DailyNoteConfig {
            folder: "Daily".into(),
            date_format: "%Y-%m-%d".into(),
            template: None,
        };
        let plan = daily_note_plan(
            &config,
            now,
            &BTreeSet::new(),
            Some("---\ntags: daily\n---\n# {{title}} at {{time}}\n"),
        )
        .unwrap();
        assert_eq!(plan.path, "Daily/2024-06-07.md");
        assert_eq!(
            render_template("{{title}} {{date}}", now, "Standup", "x").unwrap(),
            "Standup 2024-06-07"
        );
        let mut adapter = SafeAdapter {
            vault: &vault,
            snapshots: Vec::new(),
        };
        adapter.apply_daily(&plan);
        assert!(vault.read(&VaultPath::new(&plan.path).unwrap()).is_ok());
        let snapshots = (1..=3)
            .map(|revision| Snapshot {
                revision,
                timestamp: now,
                content: revision.to_string(),
            })
            .collect();
        adapter.apply_recovery(retention_plan(snapshots, 2));
        assert_eq!(adapter.snapshots.len(), 2);
        assert_eq!(Sha256Digest::of("stable"), Sha256Digest::of("stable"));
    }

    #[test]
    fn canvas_and_base_fixtures_round_trip_with_extensions() {
        let mut canvas = CanvasDocument::from_json(CANVAS).unwrap();
        canvas
            .move_node("brief", Point { x: 40.0, y: 30.0 })
            .unwrap();
        assert!(canvas.to_json().unwrap().contains("vendorCanvasExtension"));
        let base = BaseDocument::from_yaml(BASE).unwrap();
        assert!(base.to_yaml().unwrap().contains("future-view-key"));
    }

    #[test]
    fn workspace_layout_roundtrip_and_feature_disable_are_observable() {
        let mut workspace = Workspace::default();
        workspace.view_kinds.insert("graph".into());
        workspace.dispatch(Action::Open {
            window: None,
            view: View::new("graph"),
        });
        workspace.dispatch(Action::SetFeature {
            feature: "graph".into(),
            enabled: false,
        });
        assert!(matches!(
            workspace
                .dispatch(Action::Open {
                    window: None,
                    view: View::new("graph")
                })
                .as_slice(),
            [Effect::Error(_)]
        ));
        let encoded = serde_json::to_string(&workspace).unwrap();
        let mut restored = Workspace::default();
        restored.dispatch(Action::Restore(encoded));
        assert!(restored.validate());
    }

    #[test]
    fn auth_and_url_boundaries_remain_offline_and_redacted() {
        let mut session = auth::Session::signed_out();
        session.begin_login().unwrap();
        session
            .authenticated("local-user", Secret::new("offline-token"), None)
            .unwrap();
        assert!(format!("{session:?}").contains("[REDACTED]"));
        let policy = web::NavigationPolicy {
            allowed_hosts: ["docs.example".into()].into_iter().collect(),
            allow_external: false,
        };
        assert!(web::navigation(&policy, "file:///etc/passwd").is_err());
        assert!(web::navigation(&policy, "https://evil.example").is_err());
        assert_eq!(
            web::navigation(&policy, "https://docs.example/guide")
                .unwrap()
                .as_str(),
            "https://docs.example/guide"
        );
        let _: Value = serde_json::json!({"network": "not contacted"});
    }
}
