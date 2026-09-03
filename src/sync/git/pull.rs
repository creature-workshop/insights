//! Replaying local commits onto the remote's, and what that comes to.

use anyhow::{anyhow, Result};
use std::path::Path;

use super::{paths, run, run_checked};

/// What pulling did.
#[derive(Debug, Clone, PartialEq)]
pub enum PullOutcome {
    /// The remote's insights are now in the store.
    Merged,
    /// These insights were edited on two machines, and the store was left as it
    /// was for the operator to reconcile.
    Conflicted(Vec<String>),
}

/// Replays local commits on top of the remote's.
///
/// Reports the conflicting paths instead of leaving the rebase in progress: the
/// server reads this directory continuously, and a half-applied rebase would
/// hand it insight files full of conflict markers.
///
/// Why a conflict stops the sync rather than resolving it, and what is meant to
/// replace that, is `docs/decisions/000001-sync-stops-on-conflict.md`.
pub fn pull_rebase(store: &Path, branch: &str) -> Result<PullOutcome> {
    let output = run(store, &["pull", "--rebase", "origin", branch])?;
    if output.status.success() {
        return Ok(PullOutcome::Merged);
    }

    let conflicts = conflicted_paths(store)?;
    abort_rebase(store);

    if conflicts.is_empty() {
        return Err(anyhow!(
            "git pull --rebase failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(PullOutcome::Conflicted(conflicts))
}

/// The paths a stopped rebase could not reconcile.
fn conflicted_paths(store: &Path) -> Result<Vec<String>> {
    let output = run_checked(store, &["diff", "--name-only", "--diff-filter=U"])?;

    Ok(paths(&output))
}

/// Puts the store back the way it was before a rebase stopped on a conflict.
fn abort_rebase(store: &Path) {
    let _ = run(store, &["rebase", "--abort"]);
}
