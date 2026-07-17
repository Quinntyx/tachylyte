//! Small, UI-independent state and action facade for the launcher.
//!
//! This intentionally delegates persistence and filesystem policy to
//! `tachylyte-vaults`; the launcher only owns selection and presentation
//! preferences.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tachylyte_vaults::{CreateVaultPlan, Error as VaultError, VaultEntry, VaultManager};

pub type Result<T> = std::result::Result<T, VaultError>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppearanceSettings {
    pub theme: Option<String>,
    pub compact: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureSettings {
    pub show_missing: bool,
    pub welcome_seen: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherSettings {
    pub appearance: AppearanceSettings,
    pub features: FeatureSettings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoMetadata {
    pub entry: VaultEntry,
    /// The registry operation that produced this undo record.
    pub operation: UndoOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoOperation {
    Removed,
    Pruned,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WelcomeSeedPlan {
    pub title: String,
    pub body: String,
}

impl Default for WelcomeSeedPlan {
    fn default() -> Self {
        Self {
            title: "Welcome to Tachylyte".into(),
            body: "Open or create a vault to get started.".into(),
        }
    }
}

pub struct LauncherModel {
    vaults: VaultManager,
    selected: Option<String>,
    settings: LauncherSettings,
}

impl LauncherModel {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::from_manager(VaultManager::open(path)?)
    }
    pub fn open_default() -> Result<Self> {
        Self::from_manager(VaultManager::open_default()?)
    }
    fn from_manager(vaults: VaultManager) -> Result<Self> {
        let selected = vaults
            .selected_id()
            .map(str::to_owned)
            .or_else(|| vaults.default_vault().map(|v| v.id.clone()));
        let appearance = vaults
            .appearance()
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        let features = vaults
            .feature_settings()
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        Ok(Self {
            vaults,
            selected,
            settings: LauncherSettings {
                appearance,
                features,
            },
        })
    }
    pub fn manager(&self) -> &VaultManager {
        &self.vaults
    }
    pub fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn selected(&self) -> Option<&VaultEntry> {
        self.selected
            .as_deref()
            .and_then(|id| self.vaults.vaults().iter().find(|v| v.id == id))
    }
    pub fn recent(&self) -> Vec<&VaultEntry> {
        self.vaults.recent()
    }
    pub fn vaults(&self) -> &[VaultEntry] {
        self.vaults.vaults()
    }
    pub fn default_vault(&self) -> Option<&VaultEntry> {
        self.vaults.default_vault()
    }
    pub fn select(&mut self, id: impl Into<String>) -> Result<&VaultEntry> {
        let id = id.into();
        if !self.vaults.vaults().iter().any(|v| v.id == id) {
            return Err(VaultError::NotFound(id));
        }
        self.vaults.set_selected_vault(&id)?;
        self.selected = Some(id);
        Ok(self.selected().expect("selected id was validated"))
    }
    pub fn load(&mut self, id: &str) -> Result<VaultEntry> {
        let entry = self.vaults.open_vault(id)?;
        self.selected = Some(entry.id.clone());
        Ok(entry)
    }
    pub fn create_plan(
        &self,
        name: impl Into<String>,
        parent: impl Into<PathBuf>,
    ) -> Result<CreateVaultPlan> {
        self.vaults.plan_create_vault(name, parent)
    }
    pub fn execute_create(&mut self, plan: &CreateVaultPlan) -> Result<VaultEntry> {
        let entry = self.vaults.execute_create_vault(plan)?;
        self.vaults.set_selected_vault(&entry.id)?;
        self.selected = Some(entry.id.clone());
        Ok(entry)
    }
    pub fn import(&mut self, path: impl AsRef<Path>) -> Result<VaultEntry> {
        let entry = self.vaults.import_folder(path)?;
        self.vaults.set_selected_vault(&entry.id)?;
        self.selected = Some(entry.id.clone());
        Ok(entry)
    }
    pub fn rename(&mut self, id: &str, name: impl Into<String>) -> Result<VaultEntry> {
        self.vaults.rename_display_name(id, name)
    }
    pub fn remove(&mut self, id: &str) -> Result<UndoMetadata> {
        let entry = self.vaults.remove(id)?;
        if self.selected.as_deref() == Some(id) {
            self.selected = self.default_vault().map(|v| v.id.clone());
        }
        Ok(UndoMetadata {
            entry,
            operation: UndoOperation::Removed,
        })
    }
    pub fn prune(&mut self) -> Result<Vec<UndoMetadata>> {
        let undo = self.vaults.prune_stale()?;
        self.selected = self.vaults.selected_id().map(str::to_owned);
        Ok(undo
            .removed
            .into_iter()
            .map(|entry| UndoMetadata {
                entry,
                operation: UndoOperation::Pruned,
            })
            .collect())
    }
    pub fn reveal_intent(&self, id: &str) -> Result<tachylyte_vaults::RevealIntent> {
        self.vaults.reveal(id)
    }
    pub fn settings(&self) -> &LauncherSettings {
        &self.settings
    }
    pub fn settings_mut(&mut self) -> &mut LauncherSettings {
        &mut self.settings
    }
    /// Persist settings edited through [`Self::settings_mut`].
    pub fn save_settings(&mut self) -> Result<()> {
        self.vaults.set_appearance(
            serde_json::to_value(&self.settings.appearance)
                .expect("launcher settings are serializable"),
        )?;
        self.vaults.set_feature_settings(
            serde_json::to_value(&self.settings.features)
                .expect("launcher settings are serializable"),
        )
    }
    pub fn undo(&mut self, metadata: UndoMetadata) -> Result<VaultEntry> {
        let entry = self.vaults.restore_entry(metadata.entry)?;
        self.vaults.set_selected_vault(&entry.id)?;
        self.selected = Some(entry.id.clone());
        Ok(entry)
    }
    pub fn welcome_seed_plan(&self) -> WelcomeSeedPlan {
        WelcomeSeedPlan::default()
    }
    pub fn welcome_seed_plan_for(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<Option<tachylyte_vaults::WelcomeSeedPlan>> {
        self.vaults.plan_welcome_seed(root)
    }
}
