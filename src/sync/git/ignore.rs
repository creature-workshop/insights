//! Keeping this machine's own files out of the shared history.

use anyhow::Result;
use std::path::Path;

/// The machine-local files that sit in the store but describe this machine
/// rather than the insights, and so must never travel to another one.
const GITIGNORE: &str = include_str!("store.gitignore");

/// Writes the store's `.gitignore`, leaving a correct one alone.
///
/// Called on every sync rather than only at setup, so a store cloned from
/// another machine — or one set up before a file joined the list — still
/// excludes them.
pub fn write_gitignore(store: &Path) -> Result<()> {
    let path = store.join(".gitignore");
    if path.exists() && std::fs::read_to_string(&path)? == GITIGNORE {
        return Ok(());
    }

    std::fs::create_dir_all(store)?;
    std::fs::write(&path, GITIGNORE)?;

    Ok(())
}
