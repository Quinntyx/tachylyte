//! GPUI-independent state and typed intents for the network settings surfaces.

use std::collections::BTreeSet;

use tachylyte_services::{auth, publish, sync, Connectivity};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    SignedOut,
    SigningIn,
    SignedIn { account_id: String },
    Error(String),
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountIntent {
    Login,
    Logout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountModel {
    pub status: AccountStatus,
    pub intents: Vec<AccountIntent>,
}

impl AccountModel {
    pub fn new(session: auth::Session, connectivity: Connectivity) -> Self {
        let status = if connectivity == Connectivity::Offline {
            AccountStatus::Offline
        } else {
            match session.state {
                auth::SessionState::SignedOut | auth::SessionState::Expired => {
                    AccountStatus::SignedOut
                }
                auth::SessionState::Authenticating => AccountStatus::SigningIn,
                auth::SessionState::Authenticated { account_id } => {
                    AccountStatus::SignedIn { account_id }
                }
                auth::SessionState::Degraded => AccountStatus::Error("session degraded".into()),
            }
        };
        Self {
            status,
            intents: Vec::new(),
        }
    }

    pub fn request_login(&mut self) {
        self.intents.push(AccountIntent::Login);
    }

    pub fn request_logout(&mut self) {
        self.intents.push(AccountIntent::Logout);
    }

    pub fn drain_intents(&mut self) -> Vec<AccountIntent> {
        std::mem::take(&mut self.intents)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub last_seen: String,
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityEntry {
    pub summary: String,
    pub detail: String,
}

pub type SyncState = sync::SyncState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncIntent {
    SetEnabled(bool),
    SetSelectiveFolders(BTreeSet<String>),
    SetSelectiveSettings(BTreeSet<String>),
    Pause,
    Resume,
    ResolveConflict {
        conflict: sync::Conflict,
        resolution: sync::Resolution,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncModel {
    pub enabled: bool,
    pub selective_folders: BTreeSet<String>,
    pub selective_settings: BTreeSet<String>,
    pub devices: Vec<Device>,
    pub activity: Vec<ActivityEntry>,
    pub conflicts: Vec<sync::Conflict>,
    pub state: SyncState,
    pub paused: bool,
    pub backend_configured: bool,
    pub offline: bool,
    pub intents: Vec<SyncIntent>,
}

impl SyncModel {
    pub fn new(connectivity: Connectivity) -> Self {
        Self::with_backend(true, connectivity)
    }

    pub fn with_backend(backend_configured: bool, connectivity: Connectivity) -> Self {
        let offline = connectivity == Connectivity::Offline;
        Self {
            enabled: false,
            selective_folders: BTreeSet::new(),
            selective_settings: BTreeSet::new(),
            devices: Vec::new(),
            activity: Vec::new(),
            conflicts: Vec::new(),
            state: if offline {
                SyncState::Offline
            } else {
                SyncState::Ready
            },
            paused: false,
            backend_configured,
            offline,
            intents: Vec::new(),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.intents.push(SyncIntent::SetEnabled(enabled));
    }

    pub fn toggle_enabled(&mut self) {
        self.set_enabled(!self.enabled);
    }

    pub fn set_selective_folders(&mut self, folders: BTreeSet<String>) {
        self.selective_folders = folders.clone();
        self.intents.push(SyncIntent::SetSelectiveFolders(folders));
    }

    pub fn set_selective_folder(&mut self, folder: String, selected: bool) {
        let mut folders = self.selective_folders.clone();
        if selected {
            folders.insert(folder);
        } else {
            folders.remove(&folder);
        }
        self.set_selective_folders(folders);
    }

    pub fn set_selective_settings(&mut self, settings: BTreeSet<String>) {
        self.selective_settings = settings.clone();
        self.intents
            .push(SyncIntent::SetSelectiveSettings(settings));
    }

    pub fn set_selective_setting(&mut self, setting: String, selected: bool) {
        let mut settings = self.selective_settings.clone();
        if selected {
            settings.insert(setting);
        } else {
            settings.remove(&setting);
        }
        self.set_selective_settings(settings);
    }

    pub fn request_pause(&mut self) {
        self.paused = true;
        self.intents.push(SyncIntent::Pause);
    }

    pub fn request_resume(&mut self) {
        self.paused = false;
        self.intents.push(SyncIntent::Resume);
    }

    pub fn pause(&mut self) {
        self.request_pause();
    }

    pub fn resume(&mut self) {
        self.request_resume();
    }

    pub fn request_resolve(&mut self, resource: String, resolution: sync::Resolution) {
        if let Some(conflict) = self.conflicts.iter().find(|c| c.resource == resource) {
            self.resolve_conflict(conflict.clone(), resolution);
        }
    }

    pub fn resolve_conflict(&mut self, conflict: sync::Conflict, resolution: sync::Resolution) {
        self.intents.push(SyncIntent::ResolveConflict {
            conflict,
            resolution,
        });
    }

    pub fn drain_intents(&mut self) -> Vec<SyncIntent> {
        std::mem::take(&mut self.intents)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishIntent {
    SetSiteConfig(publish::SiteConfig),
    SelectFiles(BTreeSet<String>),
    Publish,
    Unpublish,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishModel {
    pub backend_configured: bool,
    pub offline: bool,
    pub site: Option<publish::SiteConfig>,
    pub available_files: Vec<String>,
    pub selected_files: BTreeSet<String>,
    pub current_manifest: Option<publish::PublishManifest>,
    pub preview: Vec<publish::ManifestDiff>,
    pub intents: Vec<PublishIntent>,
}

impl PublishModel {
    pub fn new(backend_configured: bool, connectivity: Connectivity) -> Self {
        Self {
            backend_configured,
            offline: connectivity == Connectivity::Offline,
            site: None,
            available_files: Vec::new(),
            selected_files: BTreeSet::new(),
            current_manifest: None,
            preview: Vec::new(),
            intents: Vec::new(),
        }
    }

    pub fn set_site_config(&mut self, site: publish::SiteConfig) {
        self.site = Some(site.clone());
        self.intents.push(PublishIntent::SetSiteConfig(site));
    }

    pub fn set_site(&mut self, title: String, base_path: String) {
        self.set_site_config(publish::SiteConfig {
            title,
            base_path,
            extra: Default::default(),
        });
    }

    pub fn select_files(&mut self, files: BTreeSet<String>) {
        self.selected_files = files.clone();
        self.intents.push(PublishIntent::SelectFiles(files));
    }

    pub fn toggle_file(&mut self, path: String) {
        let mut files = self.selected_files.clone();
        if !files.remove(&path) {
            files.insert(path);
        }
        self.select_files(files);
    }

    pub fn request_publish(&mut self) {
        self.intents.push(PublishIntent::Publish);
    }

    pub fn request_unpublish(&mut self) {
        self.intents.push(PublishIntent::Unpublish);
    }

    pub fn publish(&mut self) {
        self.request_publish();
    }

    pub fn unpublish(&mut self) {
        self.request_unpublish();
    }

    pub fn drain_intents(&mut self) -> Vec<PublishIntent> {
        std::mem::take(&mut self.intents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_status_maps_session_and_connectivity() {
        let mut session = auth::Session::signed_out();
        assert_eq!(
            AccountModel::new(session.clone(), Connectivity::Online).status,
            AccountStatus::SignedOut
        );
        session.begin_login().unwrap();
        assert_eq!(
            AccountModel::new(session, Connectivity::Online).status,
            AccountStatus::SigningIn
        );
        assert_eq!(
            AccountModel::new(auth::Session::signed_out(), Connectivity::Offline).status,
            AccountStatus::Offline
        );
    }

    #[test]
    fn intents_are_typed_and_drained() {
        let mut account = AccountModel::new(auth::Session::signed_out(), Connectivity::Online);
        account.request_login();
        account.request_logout();
        assert_eq!(
            account.drain_intents(),
            vec![AccountIntent::Login, AccountIntent::Logout]
        );

        let mut sync = SyncModel::new(Connectivity::Online);
        sync.set_selective_folder("Notes".into(), true);
        sync.request_pause();
        assert_eq!(sync.drain_intents().len(), 2);
    }

    #[test]
    fn publish_intents_never_contain_credentials() {
        let mut publish = PublishModel::new(true, Connectivity::Online);
        publish.set_site("Docs".into(), "/docs".into());
        publish.toggle_file("index.md".into());
        publish.request_publish();
        assert!(matches!(
            publish.drain_intents().as_slice(),
            [
                PublishIntent::SetSiteConfig(_),
                PublishIntent::SelectFiles(_),
                PublishIntent::Publish
            ]
        ));
    }
}
