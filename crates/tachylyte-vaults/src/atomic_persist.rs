//! Small, dependency-free atomic file persistence primitive.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(unix)]
use std::fs::File;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Persist `bytes` by writing a uniquely-created sibling file and renaming it.
///
/// `create_new` makes the temporary file creation exclusive, so an existing
/// file (including a symlink) is never opened for writing.  The destination is
/// replaced by the platform's atomic rename operation.
pub(crate) fn persist_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "persistence path has no valid file name",
            )
        })?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();

    let (temporary, mut file) = (0..32)
        .map(|attempt| {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(".{name}.tmp-{pid}-{stamp}-{serial}-{attempt}"));
            let result = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate);
            (candidate, result)
        })
        .find_map(|(candidate, result)| match result {
            Ok(file) => Some(Ok((candidate, file))),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
            Err(error) => Some(Err(error)),
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "unable to create unique temporary file",
            )
        })??;

    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        sync_parent(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
}
