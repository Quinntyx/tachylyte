//! Filesystem-backed actions used by the workspace shell.

use std::path::PathBuf;

use tachylyte_core::{FileKind, VaultPath};

use crate::AppController;

impl AppController {
    /// Create a uniquely named Markdown note, then open it in the editor.
    pub fn create_note(&mut self) -> bool {
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        let root = vault.root().to_path_buf();
        for n in 0.. {
            let name = if n == 0 { "Untitled.md".into() } else { format!("Untitled {n}.md") };
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
            let folder = if n == 0 { "New folder".into() } else { format!("New folder {n}") };
            let path = VaultPath::new(PathBuf::from(&folder).join(".keep.md")).expect("generated folder path is valid");
            if vault.create(&path, b"# Folder\n").is_ok() {
                self.open_vault(&root);
                return true;
            }
        }
        false
    }

    /// Seed and select `Welcome.md` when the vault has no Markdown files.
    pub fn ensure_welcome(&mut self) -> bool {
        if self.vault.is_none() {
            self.status = "Open a vault first".into();
            return false;
        }
        let Some(existing) = self.entries.iter().find(|e| e.kind == FileKind::Markdown).map(|e| e.path.clone()) else {
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
