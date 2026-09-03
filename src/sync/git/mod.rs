//! Running git against the insight store.
//!
//! Every function here shells out to the `git` on PATH rather than linking a
//! git library, so the operator's own configuration applies: their SSH agent
//! and credential helper authenticate the push, and their signing and identity
//! settings sign the commit.

mod commit;
mod history;
mod ignore;
mod pull;
mod remote;
mod repo;

pub use commit::{commit_local_changes, only_insights};
pub use history::{changed_since, current_branch, head};
pub use ignore::write_gitignore;
pub use pull::{pull_rebase, PullOutcome};
pub use remote::{push, remote_has_branch, remote_url, remove_remote, set_remote};
pub use repo::{init, is_repo};

use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::{Command, Output};

/// The environment git uses to locate a repository, which overrides `-C`.
///
/// A hook runs with these set to the repository that invoked it, so a sync
/// started from one — or from any script that exports them — would act on that
/// repository instead of the store.
const REPO_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_PREFIX",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// Runs a git command in the store and hands back what it produced.
///
/// Failure is left for the caller to read out of the `Output`, because whether
/// a non-zero status is a problem depends on the question being asked — a
/// rebase that stops on a conflict is a fact to report, not an error.
fn run(store: &Path, args: &[&str]) -> Result<Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(store).args(args);

    for key in REPO_ENV {
        command.env_remove(key);
    }

    command
        .output()
        .map_err(|e| anyhow!("Could not run git (is it installed and on PATH?): {e}"))
}

/// Runs a git command in the store, failing when git does.
///
/// Only trailing whitespace is stripped. `git status --porcelain` puts each
/// file's state in the first two columns, and a file modified but not staged
/// leaves the first of them blank — trimming the front would shift that line's
/// path a character to the left.
fn run_checked(store: &Path, args: &[&str]) -> Result<String> {
    let output = run(store, args)?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// Splits git's line-per-path output into paths, dropping blank lines.
fn paths(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect()
}
