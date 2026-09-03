//! The remote a store syncs through: reading, setting, removing, and pushing
//! to it.

use anyhow::Result;
use std::path::Path;

use super::{run, run_checked};

/// The URL insights are pushed to and pulled from, if one is set.
pub fn remote_url(store: &Path) -> Option<String> {
    let output = run(store, &["remote", "get-url", "origin"]).ok()?;
    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// Points the store at `url`, replacing any remote it already had.
pub fn set_remote(store: &Path, url: &str) -> Result<()> {
    if remote_url(store).is_some() {
        run_checked(store, &["remote", "set-url", "origin", url])?;
    } else {
        run_checked(store, &["remote", "add", "origin", url])?;
    }

    Ok(())
}

/// Drops the store's remote, leaving its history in place.
///
/// A store with no remote is already in the state asked for, so this is safe
/// to call twice.
pub fn remove_remote(store: &Path) -> Result<()> {
    if remote_url(store).is_none() {
        return Ok(());
    }

    run_checked(store, &["remote", "remove", "origin"])?;

    Ok(())
}

/// Whether the remote has a branch to pull, which it does not before the first
/// push to an empty repository.
pub fn remote_has_branch(store: &Path, branch: &str) -> bool {
    run(store, &["ls-remote", "--heads", "origin", branch])
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// Sends local commits to the remote.
pub fn push(store: &Path, branch: &str) -> Result<()> {
    run_checked(store, &["push", "--set-upstream", "origin", branch])?;

    Ok(())
}
