use crate::server::models::usage::{self, Usage};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use dirs::{data_dir, home_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// Default values for backwards compatibility with existing insight files
fn default_created_at() -> DateTime<Utc> {
    // For existing insights, use a reasonable fallback date
    DateTime::parse_from_rfc3339("2025-05-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn default_last_updated() -> DateTime<Utc> {
    // For existing insights, use current time as last_updated
    Utc::now()
}

// Frontmatter parsing constants
const FRONTMATTER_START: &str = "---\n";
const FRONTMATTER_END: &str = "\n---\n";
const FRONTMATTER_START_LEN: usize = 4; // Length of "---\n"
const FRONTMATTER_END_LEN: usize = 5; // Length of "\n---\n"

/// YAML frontmatter structure for insight files.
///
/// Temporal fields are retained here so older files that stored timestamps in
/// frontmatter continue to load. New files write timestamps in a trailing
/// metadata block after the details body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightMetaData {
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub name: String,
    pub overview: String,

    #[serde(default = "default_created_at")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_last_updated")]
    pub last_updated: DateTime<Utc>,
    #[serde(default)]
    pub update_count: u32,
    #[serde(default)]
    pub retrieval_count: u32,
    #[serde(default)]
    pub search_hit_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed: Option<DateTime<Utc>>,
    #[serde(default)]
    pub pinned: bool,

    // Embedding metadata - excluded from files (set to None in write_to_file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_computed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
struct InsightFileFrontMatter {
    topic: String,
    name: String,
    overview: String,
}

/// The metadata block trailing an insight's details.
///
/// The usage counters are read but never written: files written before the
/// usage store existed carry them, and loading such a file is how those counts
/// reach the store. Everything else here describes the insight itself and is
/// written on every save.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InsightTemporalMetadata {
    created_at: DateTime<Utc>,
    last_updated: DateTime<Utc>,
    update_count: u32,
    #[serde(default)]
    pinned: bool,

    #[serde(default, skip_serializing)]
    retrieval_count: u32,
    #[serde(default, skip_serializing)]
    search_hit_count: u32,
    #[serde(default, skip_serializing)]
    last_accessed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InsightFileFooter {
    metadata: InsightTemporalMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Insight {
    pub topic: String,
    pub name: String,
    pub overview: String,
    pub details: String,

    // Temporal metadata
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub update_count: u32,

    // Usage metadata
    pub retrieval_count: u32,
    pub search_hit_count: u32,
    pub last_accessed: Option<DateTime<Utc>>,
    pub pinned: bool,

    // Embedding metadata (None if not computed yet)
    pub embedding_version: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub embedding_text: Option<String>, // The exact text that was embedded
    pub embedding_computed: Option<DateTime<Utc>>,
}

impl Insight {
    pub fn new(topic: String, name: String, overview: String, details: String) -> Self {
        let now = Utc::now();
        Self {
            topic,
            name,
            overview,
            details,
            created_at: now,
            last_updated: now,
            update_count: 0,
            retrieval_count: 0,
            search_hit_count: 0,
            last_accessed: None,
            pinned: false,
            embedding_version: None,
            embedding: None,
            embedding_text: None,
            embedding_computed: None,
        }
    }
}

pub fn file_path(insight: &Insight) -> Result<PathBuf> {
    let insights_root = get_insights_root()?;
    // Normalize file paths for x-platform compatibility.
    // Original case is preserved in insight metadata.
    let normalized_topic = insight.topic.to_lowercase();
    let normalized_name = insight.name.to_lowercase();
    Ok(insights_root
        .join(&normalized_topic)
        .join(format!("{normalized_name}.insight.md")))
}

pub fn save(insight: &Insight) -> Result<()> {
    let file_path = file_path(insight)?;
    ensure_parent_dir_exists(&file_path)?;
    check_insight_is_new(&file_path, &insight.topic, &insight.name)?;
    write_to_file(insight, &file_path)
}

/// Save an insight, overwriting if it already exists (used for embedding updates)
#[allow(dead_code)]
pub fn save_existing(insight: &Insight) -> Result<()> {
    let file_path = file_path(insight)?;
    write_to_file(insight, &file_path)
}

fn write_to_file(insight: &Insight, file_path: &PathBuf) -> Result<()> {
    ensure_parent_dir_exists(file_path)?;
    fs::write(file_path, render(insight)?)?;

    Ok(())
}

/// Renders an insight as the contents of its file.
///
/// Only the insight itself appears here. This machine's usage counters live in
/// the usage store, so reading an insight never changes what this returns.
fn render(insight: &Insight) -> Result<String> {
    let frontmatter = InsightFileFrontMatter {
        topic: insight.topic.clone(),
        name: insight.name.clone(),
        overview: insight.overview.clone(),
    };
    let temporal_metadata = InsightFileFooter {
        metadata: InsightTemporalMetadata {
            created_at: insight.created_at,
            last_updated: insight.last_updated,
            update_count: insight.update_count,
            retrieval_count: insight.retrieval_count,
            search_hit_count: insight.search_hit_count,
            last_accessed: insight.last_accessed,
            pinned: insight.pinned,
        },
    };

    let yaml_content = serde_yaml::to_string(&frontmatter)?;
    let metadata_content = serde_yaml::to_string(&temporal_metadata)?;

    Ok(format!(
        "---\n{}---\n\n# Details\n{}\n\n---\n{}",
        yaml_content, insight.details, metadata_content
    ))
}

pub fn load(topic: &str, name: &str) -> Result<Insight> {
    let file_path = make_insight_path(topic, name)?;

    if !file_path.exists() {
        return Err(anyhow!("Insight {topic}/{name} not found"));
    }

    let content = fs::read_to_string(&file_path)?;
    let mut insight = parse_insight_from_content(topic, name, &content)?;
    attach_usage(&mut insight);

    Ok(insight)
}

pub fn load_from_path(path: &std::path::Path) -> Result<Insight> {
    let mut insight = parse_file(path)?;
    attach_usage(&mut insight);

    Ok(insight)
}

/// Reads an insight file without consulting this machine's usage store, so the
/// counters are whatever the file itself carries.
///
/// The migration uses this to find counts left in files written before the
/// usage store existed; every other caller wants `load_from_path`.
fn parse_file(path: &std::path::Path) -> Result<Insight> {
    let content = fs::read_to_string(path)?;
    let topic = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|topic| topic.to_str())
        .unwrap_or("unknown");
    let name = extract_insight_name(path)
        .ok_or_else(|| anyhow!("Not an insight file: {}", path.display()))?;

    parse_insight_from_content(topic, &name, &content)
}

/// Replaces an insight's usage counters with what this machine has recorded.
pub fn attach_usage(insight: &mut Insight) {
    let usage = usage::get(&insight.topic, &insight.name);
    insight.retrieval_count = usage.retrieval_count;
    insight.search_hit_count = usage.search_hit_count;
    insight.last_accessed = usage.last_accessed;
}

pub fn update(
    insight: &mut Insight,
    new_overview: Option<&str>,
    new_details: Option<&str>,
) -> Result<()> {
    if let Some(overview) = new_overview {
        insight.overview = overview.to_string();
    }
    if let Some(details) = new_details {
        insight.details = details.to_string();
    }

    if new_overview.is_none() && new_details.is_none() {
        return Err(anyhow!(
            "At least one of overview or details must be provided"
        ));
    }

    // Update temporal metadata
    insight.last_updated = Utc::now();
    insight.update_count += 1;

    let existing_file_path = make_insight_path(&insight.topic, &insight.name)?;
    if !existing_file_path.exists() {
        return Err(anyhow!(
            "Insight {}/{} not found",
            insight.topic,
            insight.name
        ));
    }

    let new_file_path = file_path(insight)?;

    // Gets recomputed lazily on next search.
    clear_embedding(insight);

    // Delete the existing file FIRST to ensure cross-platform compatibility.
    // Prevents issues on case-insensitive filesystems
    fs::remove_file(&existing_file_path)?;

    // Clean up empty directory from old location
    if let Some(parent) = existing_file_path.parent() {
        let _ = fs::remove_dir(parent);
    }

    // Now save to the normalized path
    write_to_file(insight, &new_file_path)?;

    Ok(())
}

pub fn pin(insight: &mut Insight) -> Result<()> {
    insight.pinned = true;
    let file_path = file_path(insight)?;
    write_to_file(insight, &file_path)
}

pub fn unpin(insight: &mut Insight) -> Result<()> {
    insight.pinned = false;
    let file_path = file_path(insight)?;
    write_to_file(insight, &file_path)
}

pub fn clear_embedding(insight: &mut Insight) {
    insight.embedding_version = None;
    insight.embedding = None;
    insight.embedding_text = None;
    insight.embedding_computed = None;
}

pub fn delete(insight: &Insight) -> Result<()> {
    let file_path = file_path(insight)?;
    check_insight_exists(&file_path, &insight.topic, &insight.name)?;
    fs::remove_file(&file_path)?;
    cleanup_empty_dir(&file_path)?;
    Ok(())
}

pub fn get_insights_root() -> Result<PathBuf> {
    if let Ok(custom_root) = std::env::var("INSIGHTS_ROOT") {
        return Ok(PathBuf::from(custom_root));
    }

    // XDG Base Directory: $XDG_DATA_HOME/insights (typically ~/.local/share/insights)
    let xdg_base = data_dir().or_else(|| home_dir().map(|h| h.join(".local").join("share")));
    let xdg_insights = xdg_base.map(|b| b.join("insights"));
    let legacy = home_dir().map(|h| h.join(".blizz").join("persistent").join("insights"));

    if let Some(ref path) = xdg_insights {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    if let Some(ref path) = legacy {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    xdg_insights
        .ok_or_else(|| anyhow!("Could not resolve insights data directory (set INSIGHTS_ROOT)"))
}

pub fn get_valid_insights_dir() -> Result<std::path::PathBuf> {
    let insights_dir = get_insights_root()?;
    if !insights_dir.exists() {
        println!("No insights found. Create some insights first!");
        return Err(anyhow!("No insights directory found"));
    }
    Ok(insights_dir)
}

pub fn parse_insight_with_metadata(content: &str) -> Result<(InsightMetaData, String)> {
    if let Ok((frontmatter_section, body)) = split_frontmatter_content(content) {
        if let Ok(result) = parse_yaml_format(frontmatter_section, body) {
            Ok(result)
        } else {
            Ok(parse_legacy_format(frontmatter_section, body))
        }
    } else {
        Ok(parse_legacy_format_no_frontmatter(content))
    }
}

fn split_frontmatter_content(content: &str) -> Result<(&str, &str)> {
    if !content.starts_with(FRONTMATTER_START) {
        return Err(anyhow!("Invalid insight format: missing frontmatter"));
    }

    let content_after_start = &content[FRONTMATTER_START_LEN..];
    if let Some(end_pos) = content_after_start.find(FRONTMATTER_END) {
        let frontmatter_section = &content_after_start[..end_pos];
        let body = &content_after_start[end_pos + FRONTMATTER_END_LEN..];
        Ok((frontmatter_section, body))
    } else {
        Err(anyhow!(
            "Invalid insight format: could not find end of frontmatter"
        ))
    }
}

fn parse_yaml_format(frontmatter_section: &str, body: &str) -> Result<(InsightMetaData, String)> {
    let mut frontmatter = serde_yaml::from_str::<InsightMetaData>(frontmatter_section)?;
    let (body_without_metadata, temporal_metadata) = split_trailing_metadata(body);
    if let Some(metadata) = temporal_metadata {
        frontmatter.created_at = metadata.created_at;
        frontmatter.last_updated = metadata.last_updated;
        frontmatter.update_count = metadata.update_count;
        frontmatter.retrieval_count = metadata.retrieval_count;
        frontmatter.search_hit_count = metadata.search_hit_count;
        frontmatter.last_accessed = metadata.last_accessed;
        frontmatter.pinned = metadata.pinned;
    }

    let details = clean_body_content(body_without_metadata);
    Ok((frontmatter, details))
}

fn split_trailing_metadata(body: &str) -> (&str, Option<InsightTemporalMetadata>) {
    let trimmed_body = body.trim_end();

    if let Some((details, metadata_section)) = trimmed_body.rsplit_once("\n---\n") {
        if let Ok(footer) = serde_yaml::from_str::<InsightFileFooter>(metadata_section) {
            return (details, Some(footer.metadata));
        }
    }

    (body, None)
}

fn parse_legacy_format_no_frontmatter(content: &str) -> (InsightMetaData, String) {
    let content = content.trim();

    // For legacy files without frontmatter, use the first line as overview
    // and the rest as details (if there are multiple lines)
    let lines: Vec<&str> = content.lines().collect();
    let (overview, details) = if lines.len() > 1 {
        (
            lines[0].to_string(),
            lines[1..].join("\n").trim().to_string(),
        )
    } else {
        (content.to_string(), String::new())
    };

    let frontmatter = InsightMetaData {
        topic: "".to_string(),
        name: "".to_string(),
        overview,
        created_at: default_created_at(),
        last_updated: default_last_updated(),
        update_count: 0,
        retrieval_count: 0,
        search_hit_count: 0,
        last_accessed: None,
        pinned: false,
        embedding_version: None,
        embedding: None,
        embedding_text: None,
        embedding_computed: None,
    };

    (frontmatter, details)
}

fn parse_legacy_format(frontmatter_section: &str, body: &str) -> (InsightMetaData, String) {
    let overview = frontmatter_section.trim().to_string();
    let details = body.trim().to_string();

    let frontmatter = InsightMetaData {
        topic: "".to_string(),
        name: "".to_string(),
        overview,
        created_at: default_created_at(),
        last_updated: default_last_updated(),
        update_count: 0,
        retrieval_count: 0,
        search_hit_count: 0,
        last_accessed: None,
        pinned: false,
        embedding_version: None,
        embedding: None,
        embedding_text: None,
        embedding_computed: None,
    };

    (frontmatter, details)
}

fn clean_body_content(body: &str) -> String {
    body.lines()
        .skip_while(|line| line.trim().is_empty() || line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Moves usage counters out of insight files and into this machine's usage
/// store.
///
/// The server calls this on start, every start. Having a usage store does not
/// mean the files are clean: run an older build against the same store and it
/// writes counters back into them, so the only reliable question is whether a
/// file still holds counters right now.
///
/// Only files that keep a metadata footer are rewritten. Insights recorded in
/// older layouts are left exactly as they are, so a first sync shows the
/// counters leaving and nothing else.
pub fn migrate_usage_footers() -> Result<()> {
    let mut found = Vec::new();

    for path in insight_file_paths()? {
        migrate_file(&path, &mut found)?;
    }

    usage::adopt_counts(found)
}

/// Takes one file's usage counters into `found` and rewrites it without them.
///
/// A file that keeps no metadata footer is left untouched: the counters were
/// only ever written there, and rewriting such a file would have to invent the
/// timestamps it never recorded.
fn migrate_file(path: &std::path::Path, found: &mut Vec<(String, String, Usage)>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    if !has_metadata_footer(&content) {
        return Ok(());
    }

    let insight = parse_file(path)?;
    if carries_usage(&insight) {
        found.push((
            insight.topic.clone(),
            insight.name.clone(),
            Usage {
                retrieval_count: insight.retrieval_count,
                search_hit_count: insight.search_hit_count,
                last_accessed: insight.last_accessed,
            },
        ));
    }

    rewrite_if_changed(path, &insight)
}

/// Prints true when the file keeps its metadata in a trailing block, which is
/// the only place usage counters were ever written.
fn has_metadata_footer(content: &str) -> bool {
    match split_frontmatter_content(content) {
        Ok((_, body)) => split_trailing_metadata(body).1.is_some(),
        Err(_) => false,
    }
}

/// Prints true when the file this insight came from recorded any usage of it.
fn carries_usage(insight: &Insight) -> bool {
    insight.retrieval_count > 0 || insight.search_hit_count > 0 || insight.last_accessed.is_some()
}

/// Writes the insight back only when rendering it differs from the file on
/// disk, so re-running over an already-current store touches nothing.
fn rewrite_if_changed(path: &std::path::Path, insight: &Insight) -> Result<()> {
    let rendered = render(insight)?;
    if fs::read_to_string(path)? == rendered {
        return Ok(());
    }

    fs::write(path, rendered)?;
    Ok(())
}

/// Prints the path of every insight file in the store, across all topics.
fn insight_file_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for topic_path in get_search_paths(None)? {
        if !topic_path.exists() {
            continue;
        }
        for entry in fs::read_dir(&topic_path)? {
            let path = entry?.path();
            if is_insight_file(&path) {
                paths.push(path);
            }
        }
    }

    Ok(paths)
}

pub fn get_topics() -> Result<Vec<String>> {
    let insights_root = get_insights_root()?;

    if !insights_root.exists() {
        return Ok(vec![]);
    }

    let mut topics = Vec::new();

    for entry in fs::read_dir(&insights_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                topics.push(name.to_string());
            }
        }
    }

    topics.sort();
    Ok(topics)
}

pub fn get_insights(topic_filter: Option<&str>) -> Result<Vec<Insight>> {
    let search_paths = get_search_paths(topic_filter)?;
    let mut all_insights = Vec::new();

    for topic_path in search_paths {
        let mut topic_insights = collect_insights_from_topic(&topic_path)?;
        all_insights.append(&mut topic_insights);
    }

    all_insights.sort_by_key(|insight| insight.name.clone());
    Ok(all_insights)
}

fn get_search_paths(topic_filter: Option<&str>) -> Result<Vec<std::path::PathBuf>> {
    let insights_root = get_insights_root()?;

    if let Some(topic) = topic_filter {
        return Ok(vec![insights_root.join(topic)]);
    }

    let paths = get_topics()?
        .into_iter()
        .map(|topic| insights_root.join(topic))
        .collect();
    Ok(paths)
}

fn collect_insights_from_topic(topic_path: &std::path::Path) -> Result<Vec<Insight>> {
    if !topic_path.exists() {
        return Ok(Vec::new());
    }

    let topic_name = extract_topic_name(topic_path);
    let mut insights = Vec::new();

    for entry in fs::read_dir(topic_path)? {
        let path = entry?.path();

        if !is_insight_file(&path) {
            continue;
        }

        if let Some(insight_name) = extract_insight_name(&path) {
            insights.push(load(topic_name, &insight_name)?);
        }
    }

    Ok(insights)
}

pub fn is_insight_file(path: &std::path::Path) -> bool {
    if path.extension().and_then(|s| s.to_str()) != Some("md") {
        return false;
    }

    if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
        return file_stem.ends_with(".insight");
    }

    false
}

// Shared helper functions used by multiple public functions

fn make_insight_path(topic: &str, name: &str) -> Result<std::path::PathBuf> {
    let root = get_insights_root()?;

    // Try normalized case first.
    let normalized_topic = topic.to_lowercase();
    let normalized_name = name.to_lowercase();
    let normalized_path = root
        .join(&normalized_topic)
        .join(format!("{normalized_name}.insight.md"));

    // If normalized path exists, use it
    if normalized_path.exists() {
        return Ok(normalized_path);
    }

    // Fallback to original case for backwards compatibility with legacy insights
    let legacy_path = root.join(topic).join(format!("{name}.insight.md"));
    if legacy_path.exists() {
        return Ok(legacy_path);
    }

    // If neither exists, return the normalized path (for error messages and new file creation)
    Ok(normalized_path)
}

fn ensure_parent_dir_exists(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn check_insight_is_new(path: &std::path::Path, topic: &str, name: &str) -> Result<()> {
    if path.exists() {
        return Err(anyhow!("Insight {topic}/{name} already exists"));
    }
    Ok(())
}

fn check_insight_exists(path: &std::path::Path, topic: &str, name: &str) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Insight {topic}/{name} not found"));
    }
    Ok(())
}

fn parse_insight_from_content(topic: &str, name: &str, content: &str) -> Result<Insight> {
    let (fm, details) = parse_insight_with_metadata(content)?;

    Ok(Insight {
        // Use topic and name from frontmatter to preserve original case.
        // Fall back to parameters for backward compatibility.
        topic: if !fm.topic.is_empty() {
            fm.topic
        } else {
            topic.to_string()
        },
        name: if !fm.name.is_empty() {
            fm.name
        } else {
            name.to_string()
        },
        overview: fm.overview,
        details,
        // Handle temporal metadata with backwards compatibility
        created_at: fm.created_at,
        last_updated: fm.last_updated,
        update_count: fm.update_count,
        retrieval_count: fm.retrieval_count,
        search_hit_count: fm.search_hit_count,
        last_accessed: fm.last_accessed,
        pinned: fm.pinned,
        embedding_version: fm.embedding_version,
        embedding: fm.embedding,
        embedding_text: fm.embedding_text,
        embedding_computed: fm.embedding_computed,
    })
}

fn cleanup_empty_dir(path: &std::path::Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        if dir.read_dir()?.next().is_none() {
            fs::remove_dir(dir)?;
        }
    }
    Ok(())
}

fn extract_topic_name(topic_path: &std::path::Path) -> &str {
    topic_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
}

fn extract_insight_name(path: &std::path::Path) -> Option<String> {
    let file_stem = path.file_stem()?.to_str()?;

    if !file_stem.ends_with(".insight") {
        return None;
    }

    Some(file_stem.trim_end_matches(".insight").to_string())
}
