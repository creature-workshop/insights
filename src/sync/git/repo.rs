//! Making the store a repository of its own, and refusing one that is not.

use anyhow::{anyhow, Result};
use std::path::Path;

use super::{run, run_checked};

/// Whether the store is a git repository in its own right.
///
/// Checks for the store's own `.git` rather than asking git, because git
/// answers for the nearest repository at or above a directory — a store sitting
/// inside a checkout would otherwise look like a repository when it is only a
/// directory in someone else's.
pub fn is_repo(store: &Path) -> bool {
    store.join(".git").exists()
}

/// Turns the store into a git repository, leaving an existing one alone.
///
/// A store that sits inside someone else's checkout is refused rather than
/// nested inside it, because that placement is a mistake in where the store
/// lives and burying a second repository in the project hides it.
pub fn init(store: &Path) -> Result<()> {
    if is_repo(store) {
        return Ok(());
    }

    std::fs::create_dir_all(store)?;
    refuse_enclosing_repo(store)?;
    run_checked(store, &["init"])?;

    Ok(())
}

/// Fails when the store, which is not yet a repository, lies within one.
///
/// Every command here runs as `git -C <store>`, and git resolves that against
/// the nearest enclosing repository. Made a repository there, the store would
/// take that project's remote, and a sync would commit the project's working
/// tree as though it were insights.
fn refuse_enclosing_repo(store: &Path) -> Result<()> {
    let output = run(store, &["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "The insight store at {} sits inside the git repository at {}. \
         Move the store outside it, or set INSIGHTS_ROOT to a directory of its own.",
        store.display(),
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}
