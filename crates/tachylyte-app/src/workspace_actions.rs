//! Filesystem-backed actions used by the workspace shell.

use std::path::PathBuf;

use tachylyte_core::{FileKind, VaultPath};

use crate::{tab_notice, tab_policy, AppController};

impl AppController {
    /// Open a vault file in the current leaf, retaining the existing dirty
    /// protection implemented by [`AppController::select`].  Callers should
    /// use this for file activation instead of replacing the document
    /// directly; selecting an already active path is intentionally a no-op.
    pub fn open_file(&mut self, path: &VaultPath) -> bool {
        if self.selected_path.as_ref() == Some(path) {
            return true;
        }
        self.select(path)
    }

    /// Return the controls that are valid for the currently mounted leaf.
    /// The app currently has one mounted leaf; the workspace reducer can pass
    /// its actual leaf count when it owns the layout.
    pub fn tab_controls(&self, leaf_count: usize) -> tab_policy::TabControls {
        let has_tab = self.document.is_some() || self.selected_path.is_some();
        let is_dirty = self
            .document
            .as_ref()
            .is_some_and(|document| document.editor.is_dirty());
        tab_policy::TabControls::from_workspace(has_tab, is_dirty, leaf_count)
    }

    /// Close the selected document when it has no unsaved changes.
    pub fn try_close_document(&mut self) -> bool {
        let is_dirty = self
            .document
            .as_ref()
            .is_some_and(|document| document.editor.is_dirty());

        if tab_policy::close_decision(is_dirty) == tab_policy::CloseDecision::Blocked {
            self.status = tab_notice::dirty_close_notice().into();
            return false;
        }

        let had_document = self.document.take().is_some();
        self.selected_path = None;
        self.status = if had_document {
            "Document closed".into()
        } else {
            "No document".into()
        };
        true
    }

    /// Create a uniquely named Markdown note, then open it in the editor.
    pub fn create_note(&mut self) -> bool {
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        let root = vault.root().to_path_buf();
        for n in 0.. {
            let name = if n == 0 {
                "Untitled.md".into()
            } else {
                format!("Untitled {n}.md")
            };
            let path = VaultPath::new(name).expect("generated note path is valid");
            if vault.create(&path, b"# Untitled\n\n").is_ok() {
                self.open_vault(&root);
                return self.select(&path);
            }
        }
        false
    }

    /// Create a uniquely named folder containing a Markdown marker note.
    pub fn create_folder(&mut self) -> bool {
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        let root = vault.root().to_path_buf();
        for n in 0.. {
            let folder = if n == 0 {
                "New folder".into()
            } else {
                format!("New folder {n}")
            };
            let path = VaultPath::new(PathBuf::from(&folder).join(".keep.md"))
                .expect("generated folder path is valid");
            if vault.create(&path, b"# Folder\n").is_ok() {
                self.open_vault(&root);
                return true;
            }
        }
        false
    }

    fn mutation_allowed(&mut self, path: &VaultPath) -> bool {
        if self.selected_path.as_ref() == Some(path)
            && self
                .document
                .as_ref()
                .is_some_and(|document| document.editor.is_dirty())
        {
            self.status = "Unsaved changes: save before changing this file".into();
            return false;
        }
        self.vault.is_some()
    }

    fn refresh_entries(&mut self) -> bool {
        let Some(vault) = &self.vault else {
            return false;
        };
        match vault.scan() {
            Ok(entries) => {
                self.entries = entries;
                self.rebuild_index();
                true
            }
            Err(error) => {
                self.status = format!("Unable to refresh vault: {error}");
                false
            }
        }
    }

    /// Rename an entry after validating its vault-relative destination.
    pub fn rename_entry(&mut self, path: &str, new_name: &str) -> bool {
        let Ok(from) = VaultPath::new(path) else {
            self.status = "Invalid vault path".into();
            return false;
        };
        let name = std::path::Path::new(new_name);
        if name.components().count() != 1 || name.file_name().is_none() {
            self.status = "Rename requires a single file name".into();
            return false;
        }
        let parent = from
            .as_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        let to = VaultPath::new(parent.join(name));
        let Ok(to) = to else {
            self.status = "Invalid rename destination".into();
            return false;
        };
        if !self.mutation_allowed(&from) {
            return false;
        }
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        match vault.rename(&from, &to) {
            Ok(()) => {
                if self.selected_path.as_ref() == Some(&from) {
                    self.selected_path = Some(to.clone());
                    if let Some(document) = &mut self.document {
                        document.path = to;
                    }
                }
                self.refresh_entries()
            }
            Err(error) => {
                self.status = format!("Rename failed: {error}");
                false
            }
        }
    }

    /// Delete an entry while protecting a dirty active document.
    pub fn delete_entry(&mut self, path: &str) -> bool {
        let Ok(path) = VaultPath::new(path) else {
            self.status = "Invalid vault path".into();
            return false;
        };
        if !self.mutation_allowed(&path) {
            return false;
        }
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        match vault.delete(&path) {
            Ok(()) => {
                if self.selected_path.as_ref() == Some(&path) {
                    self.selected_path = None;
                    self.document = None;
                }
                self.refresh_entries()
            }
            Err(error) => {
                self.status = format!("Delete failed: {error}");
                false
            }
        }
    }

    /// Move an entry into a vault-relative destination folder.
    pub fn move_entry(&mut self, path: &str, destination: &str) -> bool {
        self.transfer_entry(path, destination, false)
    }

    /// Duplicate an entry into a vault-relative destination folder.
    pub fn duplicate_entry(&mut self, path: &str, destination: &str) -> bool {
        self.transfer_entry(path, destination, true)
    }

    fn transfer_entry(&mut self, path: &str, destination: &str, duplicate: bool) -> bool {
        let Ok(from) = VaultPath::new(path) else {
            self.status = "Invalid vault path".into();
            return false;
        };
        if !self.mutation_allowed(&from) {
            return false;
        }
        let Some(name) = from.as_path().file_name() else {
            self.status = "Cannot move a vault root".into();
            return false;
        };
        let Ok(to) = VaultPath::new(PathBuf::from(destination).join(name)) else {
            self.status = "Invalid destination".into();
            return false;
        };
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        let result = if duplicate {
            vault.read(&from).and_then(|data| vault.create(&to, &data))
        } else {
            vault.rename(&from, &to)
        };
        match result {
            Ok(()) => {
                if !duplicate && self.selected_path.as_ref() == Some(&from) {
                    self.selected_path = Some(to.clone());
                    if let Some(document) = &mut self.document {
                        document.path = to;
                    }
                }
                self.refresh_entries()
            }
            Err(error) => {
                self.status = format!(
                    "{} failed: {error}",
                    if duplicate { "Duplicate" } else { "Move" }
                );
                false
            }
        }
    }

    /// Seed and select `Welcome.md` when the vault has no Markdown files.
    pub fn ensure_welcome(&mut self) -> bool {
        if self.vault.is_none() {
            self.status = "Open a vault first".into();
            return false;
        }
        let Some(existing) = self
            .entries
            .iter()
            .find(|e| e.kind == FileKind::Markdown)
            .map(|e| e.path.clone())
        else {
            let vault = self.vault.as_ref().expect("checked above");
            let root = vault.root().to_path_buf();
            let path = VaultPath::new("Welcome.md").expect("literal welcome path is valid");
            if vault.create(&path, WELCOME.as_bytes()).is_err() {
                self.status = "Unable to create Welcome.md".into();
                return false;
            }
            self.open_vault(&root);
            return self.select(&path);
        };
        self.select(&existing)
    }
}

const WELCOME: &str = "# Welcome to Tachylyte\n\nTachylyte is a local Markdown workspace.\n\n## Getting started\n\n- Create notes and folders from the workspace shell.\n- Edit Markdown in the source view.\n- Use search to find notes in this vault.\n\nYour notes stay on disk in this vault directory.\n";
