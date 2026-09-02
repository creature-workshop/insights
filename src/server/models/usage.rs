//! Per-machine usage statistics for insights.
//!
//! How often this machine retrieved an insight, how often it surfaced in this
//! machine's searches, and when it was last touched here. The search ranker
//! reads these to boost insights the operator actually uses.
//!
//! These counts describe a machine's relationship to an insight rather than the
//! insight itself, so they live outside the insight file. Every read of an
//! insight would otherwise rewrite it, and a store shared between machines
//! cannot distinguish that churn from a real edit.
//!
//! The whole store is one JSON object keyed by `<topic>/<name>`, both
//! lowercased to match how insight files are named:
//!
//! ```json
//! {"rust/async-patterns": {"retrieval_count": 3, "search_hit_count": 11,
//!                          "last_accessed": "2026-09-02T22:32:29Z"}}
//! ```

use anyhow::Result;
use chrono::{DateTime, Utc};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes every read-modify-write of the store file.
///
/// The server records a hit per search result concurrently, so without this the
/// last writer would drop the others' counts.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Takes the store lock, recovering it if a previous holder panicked.
///
/// Usage statistics only weight search ranking, so a panic mid-update is worth
/// carrying on from rather than propagating to a caller who asked for an
/// insight.
fn lock_store() -> std::sync::MutexGuard<'static, ()> {
    STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What one machine has done with one insight.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub retrieval_count: u32,
    #[serde(default)]
    pub search_hit_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_accessed: Option<DateTime<Utc>>,
}

/// Why an insight was reached, which decides the counter that moves.
#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    /// The operator asked for this insight by name.
    Retrieval,
    /// The insight came back among a search's results.
    SearchHit,
}

/// The file holding this machine's usage statistics.
///
/// `INSIGHTS_USAGE_PATH` overrides the location; tests set it to keep a run out
/// of the operator's real store. Otherwise the file sits under blizz's
/// `volatile` tree, alongside the other state a machine can rebuild for itself.
pub fn usage_path() -> Result<PathBuf> {
    if let Ok(custom_path) = std::env::var("INSIGHTS_USAGE_PATH") {
        return Ok(PathBuf::from(custom_path));
    }

    Ok(home_dir()
        .ok_or_else(|| {
            anyhow::anyhow!("Could not resolve home directory (set INSIGHTS_USAGE_PATH)")
        })?
        .join(".blizz")
        .join("volatile")
        .join("insights")
        .join("usage.json"))
}

/// Prints the key an insight is stored under.
///
/// `key("Rust", "Async-Patterns")` -> `"rust/async-patterns"`
fn key(topic: &str, name: &str) -> String {
    format!("{}/{}", topic.to_lowercase(), name.to_lowercase())
}

/// Reads what this machine has done with one insight.
///
/// An insight this machine has never touched reports zeroes, so a store copied
/// from another machine starts unweighted rather than failing.
pub fn get(topic: &str, name: &str) -> Usage {
    let _guard = lock_store();
    read_store()
        .unwrap_or_default()
        .remove(&key(topic, name))
        .unwrap_or_default()
}

/// Counts one access of an insight.
pub fn record(topic: &str, name: &str, access_type: AccessType) -> Result<()> {
    record_many(&[(topic.to_string(), name.to_string())], access_type)
}

/// Counts one access of each insight in a single pass over the store.
///
/// A search records a hit for every result it returns; going through the file
/// once keeps that from costing one rewrite per result.
pub fn record_many(insights: &[(String, String)], access_type: AccessType) -> Result<()> {
    if insights.is_empty() {
        return Ok(());
    }

    let _guard = lock_store();
    let mut store = read_store().unwrap_or_default();
    let now = Utc::now();

    for (topic, name) in insights {
        let usage = store.entry(key(topic, name)).or_default();
        count_access(usage, access_type);
        usage.last_accessed = Some(now);
    }

    write_store(&store)
}

/// Adopts usage counters found in insight files, keeping whichever count is
/// higher.
///
/// Counters only ever climb, so the higher of the two is the one that has seen
/// more. Taking it means neither side loses: not the store when an insight file
/// is stale, and not the file when an older build has been recording into it.
///
/// args:
///   found  One (topic, name, usage) per insight whose file carried counters
pub fn adopt_counts(found: Vec<(String, String, Usage)>) -> Result<()> {
    if found.is_empty() {
        return Ok(());
    }

    let _guard = lock_store();
    let mut store = read_store().unwrap_or_default();

    for (topic, name, from_file) in found {
        let usage = store.entry(key(&topic, &name)).or_default();
        usage.retrieval_count = usage.retrieval_count.max(from_file.retrieval_count);
        usage.search_hit_count = usage.search_hit_count.max(from_file.search_hit_count);
        usage.last_accessed = usage.last_accessed.max(from_file.last_accessed);
    }

    write_store(&store)
}

fn count_access(usage: &mut Usage, access_type: AccessType) {
    match access_type {
        AccessType::Retrieval => usage.retrieval_count += 1,
        AccessType::SearchHit => usage.search_hit_count += 1,
    }
}

fn read_store() -> Result<HashMap<String, Usage>> {
    let path = usage_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }

    Ok(serde_json::from_str(&fs::read_to_string(&path)?)?)
}

/// Replaces the store file, writing through a temporary file so an interrupted
/// write leaves the previous statistics intact rather than a truncated file.
fn write_store(store: &HashMap<String, Usage>) -> Result<()> {
    let path = usage_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, serde_json::to_string_pretty(store)?)?;
    fs::rename(&temp_path, &path)?;

    Ok(())
}
