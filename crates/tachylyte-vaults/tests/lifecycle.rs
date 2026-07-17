use tachylyte_vaults::{CreateVaultPlan, VaultManager, VaultStatus};

#[test]
fn create_add_persist_reload_remove() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config/vaults.json");
    let mut manager = VaultManager::open(&config).unwrap();
    let plan = manager.plan_create_vault("Notes", temp.path()).unwrap();
    let created = manager.execute_create_vault(&plan).unwrap();
    assert_eq!(created.status(), VaultStatus::Available);
    let reloaded = VaultManager::open(&config).unwrap();
    assert_eq!(reloaded.vaults().len(), 1);
    assert_eq!(reloaded.vaults()[0].name, "Notes");
    drop(reloaded);
    let mut manager = VaultManager::open(&config).unwrap();
    manager.remove(&created.id).unwrap();
    assert!(manager.vaults().is_empty());

    // A plan is a capability assembled by the caller, so execution must not
    // trust a forged destination outside the supplied parent.
    let absolute = temp
        .path()
        .parent()
        .unwrap()
        .join("tachylyte-forged-absolute");
    let forged = CreateVaultPlan {
        name: "Forged".into(),
        parent: temp.path().to_path_buf(),
        path: absolute.clone(),
    };
    assert!(manager.execute_create_vault(&forged).is_err());
    assert!(!absolute.exists());

    let traversal = temp.path().join("..").join("forged-traversal");
    let forged = CreateVaultPlan {
        name: "Forged".into(),
        parent: temp.path().to_path_buf(),
        path: traversal.clone(),
    };
    assert!(manager.execute_create_vault(&forged).is_err());
    assert!(!traversal.exists());

    // The registry keeps the most recently opened usable vault as the default,
    // while retaining stale entries so the launcher can offer to prune them.
    let available = temp.path().join("available");
    std::fs::create_dir(&available).unwrap();
    let stale = temp.path().join("stale");
    std::fs::create_dir(&stale).unwrap();
    let first = manager.add("Available", &available).unwrap();
    let stale_entry = manager.add("Stale", &stale).unwrap();
    std::fs::remove_dir(&stale).unwrap();
    assert_eq!(stale_entry.status(), VaultStatus::Missing);
    assert_eq!(manager.default_vault().unwrap().id, first.id);
    manager.set_default_vault(&first.id).unwrap();
    manager.set_selected_vault(&stale_entry.id).unwrap();
    let undo = manager.prune_stale().unwrap();
    assert_eq!(undo.removed.len(), 1);
    assert!(manager.selected_vault().is_none());
    manager.undo_prune(undo).unwrap();
    assert_eq!(manager.selected_vault().unwrap().id, stale_entry.id);
    manager
        .set_appearance(serde_json::json!({ "theme": "dark", "compact": true }))
        .unwrap();
    manager
        .set_feature_settings(serde_json::json!({ "graph": true }))
        .unwrap();
    let ordered = manager.recent();
    assert!(ordered
        .windows(2)
        .all(|pair| pair[0].last_opened >= pair[1].last_opened));

    let renamed = manager.rename_display_name(&first.id, "Renamed").unwrap();
    assert_eq!(renamed.name, "Renamed");
    assert_eq!(manager.reveal(&first.id).unwrap().path, available);
    assert!(manager.rename_display_name(&first.id, "").is_err());

    // Opening the same directory merges the registration instead of creating
    // a second identity; IDs remain portable because they are path-derived.
    let merged = manager.add("Merged", &available).unwrap();
    assert_eq!(merged.id, first.id);
    assert_eq!(
        manager.vaults().iter().filter(|v| v.id == first.id).count(),
        1
    );
    let reloaded = VaultManager::open(&config).unwrap();
    assert_eq!(
        reloaded
            .vaults()
            .iter()
            .filter(|v| v.id == first.id)
            .count(),
        1
    );
    assert_eq!(
        reloaded
            .vaults()
            .iter()
            .find(|v| v.id == first.id)
            .unwrap()
            .name,
        "Merged"
    );

    // Importing an existing Obsidian-shaped directory is intentionally just a
    // folder import at this layer; Markdown and hidden metadata are preserved.
    let obsidian = temp.path().join("Obsidian");
    std::fs::create_dir(&obsidian).unwrap();
    std::fs::create_dir(obsidian.join(".obsidian")).unwrap();
    std::fs::write(
        obsidian.join(".obsidian/app.json"),
        r##"{ "theme": "obsidian-dark", "accentColor": "#7f5af0" }"##,
    )
    .unwrap();
    std::fs::write(obsidian.join("Welcome.md"), "# Welcome").unwrap();
    let imported = manager.import_folder(&obsidian).unwrap();
    assert_eq!(imported.name, "Obsidian");
    assert_eq!(imported.extra["obsidian"]["theme"], "obsidian-dark");
    assert!(obsidian.join("Welcome.md").is_file());

    let open_plan = manager.plan_open_vault(&available).unwrap();
    assert_eq!(open_plan.path, available);
    let welcome = manager.plan_welcome_seed(&available).unwrap().unwrap();
    assert!(!welcome.path.exists());
    assert!(welcome.content.contains("Welcome"));
    std::fs::write(&welcome.path, welcome.content).unwrap();
    assert!(manager.plan_welcome_seed(&available).unwrap().is_none());

    // Create/Open validation rejects invalid names, missing parents, and
    // destinations that already exist.
    assert!(manager.plan_create_vault("", temp.path()).is_err());
    assert!(manager
        .plan_create_vault("Nested/Name", temp.path())
        .is_err());
    assert!(manager
        .plan_create_vault("New", temp.path().join("missing"))
        .is_err());
    assert!(manager.plan_create_vault("available", temp.path()).is_err());

    // Migrate the app's current recent-vaults.json shape (an object with a
    // `recent` array and optional settings) without losing the launcher order.
    let legacy_path = temp.path().join("legacy/recent-vaults.json");
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy_path,
        serde_json::json!({
            "recent": [
                { "path": available, "name": "Legacy Available" },
                { "path": obsidian, "name": "Legacy Obsidian" }
            ],
            "settings": { "theme": "dark", "features": [{ "name": "graph", "enabled": true }] },
            "future_launcher_field": { "kept": true }
        })
        .to_string(),
    )
    .unwrap();
    let migrated = VaultManager::open(&legacy_path).unwrap();
    assert_eq!(migrated.vaults().len(), 2);
    assert_eq!(migrated.recent()[0].name, "Legacy Available");
    assert!(migrated.vaults()[0].id.len() >= 16);
    assert_eq!(migrated.appearance().unwrap()["theme"], "dark");
    assert_eq!(migrated.feature_settings().unwrap()[0]["enabled"], true);
    migrated.persist().unwrap();
    let migrated_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&legacy_path).unwrap()).unwrap();
    assert_eq!(migrated_json["future_launcher_field"]["kept"], true);

    // Persistence failure must roll back both the in-memory registration and
    // the directory created before the registry write.
    let blocked_parent = temp.path().join("blocked");
    std::fs::write(&blocked_parent, b"not a directory").unwrap();
    let blocked_config = blocked_parent.join("vaults.json");
    let mut blocked = VaultManager::open(&blocked_config).unwrap();
    let destination = temp.path().join("rollback-vault");
    let plan = blocked.plan_create_vault("Rollback", temp.path()).unwrap();
    assert!(blocked
        .execute_create_vault(&CreateVaultPlan {
            path: destination.clone(),
            ..plan
        })
        .is_err());
    assert!(blocked.vaults().is_empty());
    assert!(!destination.exists());

    // A legacy fixed-name temporary symlink must not be followed or touched.
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let sentinel = temp.path().join("sentinel");
        let legacy_temp = config.with_extension("json.tmp");
        std::fs::write(&sentinel, b"sentinel").unwrap();
        symlink(&sentinel, &legacy_temp).unwrap();
        manager.persist().unwrap();
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");
        assert!(std::fs::symlink_metadata(&legacy_temp)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
