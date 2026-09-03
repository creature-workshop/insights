//! Setting a store up to sync, and running one sync.

use anyhow::Result;
use std::path::Path;

use crate::server::models::insight::get_insights_root;
use crate::sync::git::{self, commit_local_changes, only_insights, write_gitignore, PullOutcome};
use crate::sync::report::SyncReport;

/// Makes the store a git repository pointed at `remote`.
///
/// Safe to run against a store that is already set up: an existing repository
/// keeps its history and simply takes the new remote.
pub fn init(remote: &str) -> Result<()> {
    let store = get_insights_root()?;
    git::init(&store)?;
    write_gitignore(&store)?;
    git::set_remote(&store, remote)?;

    Ok(())
}

/// Commits this machine's insights, takes in the other machines', and pushes.
///
/// A store that is not yet a repository becomes one here, so the first sync
/// needs nothing but the command. Local work is committed before anything is
/// pulled, so an interrupted sync can never lose an insight this machine
/// wrote. A store with no remote is still committed to, which leaves its
/// history intact for whenever one is set.
pub fn sync() -> Result<SyncReport> {
    let store = get_insights_root()?;
    git::init(&store)?;
    write_gitignore(&store)?;

    let mut report = SyncReport {
        remote: git::remote_url(&store),
        store: store.clone(),
        ..Default::default()
    };

    report.committed = commit_local_changes(&store)?;

    let Some(_) = report.remote.as_ref() else {
        return Ok(report);
    };

    let branch = git::current_branch(&store)?;
    if !take_in_remote(&store, &branch, &mut report)? {
        return Ok(report);
    }

    git::push(&store, &branch)?;
    report.pushed = true;

    Ok(report)
}

/// Stops the store syncing, keeping its history for whenever a remote is set
/// again.
pub fn forget_remote() -> Result<()> {
    let store = get_insights_root()?;
    if !git::is_repo(&store) {
        return Ok(());
    }
    git::remove_remote(&store)
}

/// Brings in what the other machines pushed, and says whether the sync may go
/// on to push.
///
/// Before the first push the remote has no branch, so there is nothing to bring
/// in and the sync goes straight on. A conflict fills in the report and stops
/// it.
fn take_in_remote(store: &Path, branch: &str, report: &mut SyncReport) -> Result<bool> {
    if !git::remote_has_branch(store, branch) {
        return Ok(true);
    }

    let before = git::head(store);
    match git::pull_rebase(store, branch)? {
        PullOutcome::Merged => {
            report.pulled = arrived_since(store, before)?;
            Ok(true)
        }
        PullOutcome::Conflicted(paths) => {
            report.conflicted = paths;
            Ok(false)
        }
    }
}

/// The insights a pull brought in, told apart from this machine's own commits
/// by the commit the store sat on beforehand.
///
/// A store with no commit before the pull has its whole contents arrive, which
/// `changed_since` cannot describe, so nothing is claimed for it.
fn arrived_since(store: &Path, before: Option<String>) -> Result<Vec<String>> {
    let Some(before) = before else {
        return Ok(Vec::new());
    };
    if git::head(store).as_deref() == Some(before.as_str()) {
        return Ok(Vec::new());
    }

    Ok(only_insights(&git::changed_since(store, &before)?))
}
