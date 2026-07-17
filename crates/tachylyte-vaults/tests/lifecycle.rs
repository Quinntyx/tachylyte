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
