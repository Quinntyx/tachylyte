use tachylyte_vaults::{VaultManager, VaultStatus};

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
}
