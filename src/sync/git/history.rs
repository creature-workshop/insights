//! What the store has changed, committed, and sits on.

use anyhow::{anyhow, Result};
use std::path::Path;

use super::{paths, run, run_checked};

/// The insights changed since the last sync, as paths relative to the store.
///
/// Covers files added, edited and deleted, and files git has never seen.
///
/// `--untracked-files=all` is what makes a new topic legible: git otherwise
/// collapses a directory it has never seen to the directory itself, so a first
/// insight under a new topic would arrive as `rust/` rather than the file.
pub fn changed_paths(store: &Path) -> Result<Vec<String>> {
    let status = run_checked(store, &["status", "--porcelain", "--untracked-files=all"])?;

    Ok(status
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect())
}

/// Records every local change as one commit.
pub fn commit_all(store: &Path, message: &str) -> Result<()> {
    run_checked(store, &["add", "--all"])?;
    run_checked(store, &["commit", "--message", message])?;

    Ok(())
}

/// The branch the store is on.
///
/// Answers for a store with no commits too, which is why this asks `git branch`
/// rather than resolving HEAD — there is nothing for HEAD to resolve to until
/// the first commit.
pub fn current_branch(store: &Path) -> Result<String> {
    let branch = run_checked(store, &["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err(anyhow!(
            "The insight store is not on a branch, so there is nothing to sync."
        ));
    }

    Ok(branch)
}

/// The commit the store is on, absent before the first one.
pub fn head(store: &Path) -> Option<String> {
    let output = run(store, &["rev-parse", "HEAD"]).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The paths that differ between a commit and where the store is now.
///
/// Answers what a pull brought in, given the commit the store sat on before it.
pub fn changed_since(store: &Path, commit: &str) -> Result<Vec<String>> {
    let output = run_checked(store, &["diff", "--name-only", commit, "HEAD"])?;

    Ok(paths(&output))
}
