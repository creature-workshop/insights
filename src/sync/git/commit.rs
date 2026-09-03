//! Turning what this machine changed into a commit.

use anyhow::Result;
use std::path::Path;

use super::history::{changed_paths, commit_all};
use crate::server::models::insight::is_insight_file;

/// Commits everything this machine has changed, and reports which insights
/// those were.
pub fn commit_local_changes(store: &Path) -> Result<Vec<String>> {
    let changed = changed_paths(store)?;
    if changed.is_empty() {
        return Ok(Vec::new());
    }

    let insights = only_insights(&changed);
    commit_all(store, &commit_message(&insights))?;

    Ok(insights)
}

/// Narrows a set of changed paths to the insights among them.
///
/// A sync commits everything in the store, the `.gitignore` it maintains
/// included, but only insights are worth telling the operator about.
///
/// `only_insights(&[".gitignore", "rust/async.insight.md"])` -> `["rust/async.insight.md"]`
pub fn only_insights(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| is_insight_file(Path::new(path)))
        .cloned()
        .collect()
}

/// Names the commit after the insights it carries.
///
/// `commit_message(&["rust/async.insight.md"])` -> `"Sync 1 insight from <machine>"`
///
/// A sync that only rewrites the `.gitignore` carries no insights at all, and
/// says so rather than claiming zero of them.
fn commit_message(insights: &[String]) -> String {
    if insights.is_empty() {
        return "Sync store settings".to_string();
    }

    let plural = if insights.len() == 1 { "" } else { "s" };

    match machine_name() {
        Some(machine) => format!("Sync {} insight{plural} from {machine}", insights.len()),
        None => format!("Sync {} insight{plural}", insights.len()),
    }
}

/// The name of this machine, so a shared history says where each insight was
/// written. Absent when the machine does not tell us.
fn machine_name() -> Option<String> {
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(name) = std::env::var(key) {
            if !name.trim().is_empty() {
                return Some(name.trim().to_string());
            }
        }
    }

    let output = std::process::Command::new("hostname").output().ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();

    (!name.is_empty()).then_some(name)
}
