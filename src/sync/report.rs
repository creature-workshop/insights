//! What one sync did, and how it is told to the operator.

use colored::*;
use std::path::{Path, PathBuf};

/// How many insights a report names before it just gives the count.
///
/// A first sync carries the whole store, and a thousand names tell the operator
/// less than the number does.
const NAMED_LIMIT: usize = 8;

/// The outcome of a single sync, in the order the sync produced it.
///
/// `conflicted` being non-empty means the sync stopped: nothing was pulled in
/// and nothing pushed, and the store was left as it was.
#[derive(Debug, Default, PartialEq)]
pub struct SyncReport {
    /// Insights this machine had changed since the last sync.
    pub committed: Vec<String>,
    /// Insights that arrived from another machine.
    pub pulled: Vec<String>,
    /// Insights edited on two machines, which the operator has to reconcile.
    pub conflicted: Vec<String>,
    /// Whether local commits reached the remote.
    pub pushed: bool,
    /// Where the store syncs to, absent until a remote is set.
    pub remote: Option<String>,
    /// The directory holding the insights, so the CLI can name it when the
    /// operator has to take over.
    pub store: PathBuf,
}

impl SyncReport {
    /// Says what the sync did, in the order it did it, one line per fact.
    pub fn render(&self) -> String {
        let mut lines = insight_lines("Sent", &self.committed, "green");
        lines.extend(insight_lines("Received", &self.pulled, "cyan"));

        if !self.conflicted.is_empty() {
            lines.extend(conflict_lines(&self.conflicted, &self.store));
            return lines.join("\n");
        }

        lines.push(self.outcome_line());
        lines.join("\n")
    }

    /// How the sync ended. With a remote it means the push succeeded, because
    /// a failed one is an error rather than a report.
    fn outcome_line(&self) -> String {
        match &self.remote {
            Some(remote) => format!("{} In sync with {}", "✓".green(), remote.cyan()),
            None => format!(
                "{} Committed locally. Set a remote with {} to share these.",
                "!".yellow(),
                "insights sync --remote <url>".cyan()
            ),
        }
    }
}

/// Lists what moved, naming each insight rather than its file.
fn insight_lines(action: &str, paths: &[String], color: &str) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }

    let plural = if paths.len() == 1 { "" } else { "s" };
    let mut lines = vec![format!(
        "{} {action} {} insight{plural}",
        "✓".green(),
        paths.len().to_string().color(color)
    )];

    for path in paths.iter().take(NAMED_LIMIT) {
        lines.push(format!("    {}", insight_label(path).dimmed()));
    }

    if let Some(remaining) = paths.len().checked_sub(NAMED_LIMIT).filter(|n| *n > 0) {
        lines.push(format!("    {}", format!("and {remaining} more").dimmed()));
    }

    lines
}

/// Turns a store path back into the insight it holds.
///
/// `insight_label("rust/async-patterns.insight.md")` -> `"rust/async-patterns"`
fn insight_label(path: &str) -> String {
    path.strip_suffix(".insight.md").unwrap_or(path).to_string()
}

/// Explains a conflict and hands over the commands to settle it.
///
/// The store is left as it was rather than mid-rebase, so the server keeps
/// reading it while the operator decides.
fn conflict_lines(paths: &[String], store: &Path) -> Vec<String> {
    let (plural, verb) = if paths.len() == 1 {
        ("", "has")
    } else {
        ("s", "have")
    };

    let mut lines = vec![format!(
        "{} Conflict: {} insight{plural} you've updated {verb} also recently changed on the remote:",
        "✗".red(),
        paths.len()
    )];

    for path in paths {
        lines.push(format!("    {}", insight_label(path).yellow()));
    }

    lines.push(format!(
        "\nThe store was left untouched. Reconcile them by hand, then sync again:\n  \
         {}\n  resolve the conflicts, commit, then\n  {}",
        format!("git -C {} pull --rebase", store.display()).cyan(),
        "insights sync".cyan()
    ));

    lines
}
