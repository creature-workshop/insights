use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Months, NaiveDate, TimeZone, Utc};
use clap::Args;
use colored::*;

use std::fs;
use std::path::{Path, PathBuf};

use crate::server::{
    models::insight,
    services::similarity,
    types::{SearchRequest, SearchSort},
};

// Semantic similarity threshold for meaningful results
const SEMANTIC_SIMILARITY_THRESHOLD: f32 = 0.2;

// Default terminal width for text wrapping
const DEFAULT_TERMINAL_WIDTH: usize = 80;

const DEFAULT_MAX_RESULTS: usize = 10;

#[derive(Debug)]
pub struct SearchResult {
    pub topic: String,
    pub name: String,
    pub overview: String,
    pub details: String,
    pub score: f32, // number of matching terms
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Search configuration options
#[derive(Args)]
pub struct SearchCommandOptions {
    /// Optional topic to restrict search to
    #[arg(short, long)]
    pub topic: Option<String>,
    /// Case-sensitive search
    #[arg(short, long)]
    pub case_sensitive: bool,
    /// Search only in overview sections
    #[arg(short, long)]
    pub overview_only: bool,
    /// Use exact term matching only
    #[arg(short, long)]
    pub exact: bool,
    /// Use semantic search (term matching + jaccard similarity, no embedding)
    #[arg(short, long)]
    pub semantic: bool,
    /// Sort results by relevance, updated timestamp, or creation timestamp
    #[arg(long, value_enum, default_value = "relevance")]
    pub sort: SearchSort,
    /// Include insights updated on or after a date (YYYY-MM-DD, RFC3339, or relative like 7d)
    #[arg(long)]
    pub since: Option<String>,
    /// Include insights updated on or before a date (YYYY-MM-DD, RFC3339, or relative like 7d)
    #[arg(long)]
    pub until: Option<String>,
    /// Maximum number of results to return (default: 10, -1 for unlimited)
    #[arg(
        short = 'n',
        long,
        alias = "max",
        short_alias = 'm',
        default_value = "10"
    )]
    pub max_results: i32,
}

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub topic: Option<String>,
    pub case_sensitive: bool,
    pub overview_only: bool,
    pub exact: bool,
    pub semantic: bool,
    pub sort: SearchSort,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    /// None = no limit, Some(n) = limit to n results
    pub max_results: Option<usize>,
}

impl SearchOptions {
    pub fn from_command_options(options: &SearchCommandOptions) -> Result<Self> {
        let request = SearchRequest {
            terms: Vec::new(),
            topic: options.topic.clone(),
            case_sensitive: options.case_sensitive,
            overview_only: options.overview_only,
            exact: options.exact,
            semantic: options.semantic,
            sort: options.sort,
            since: options.since.clone(),
            until: options.until.clone(),
            max_results: Some(options.max_results),
        };

        Self::from_request(&request)
    }

    pub fn from_request(request: &SearchRequest) -> Result<Self> {
        let now = Utc::now();
        Ok(Self {
            topic: request.topic.clone(),
            case_sensitive: request.case_sensitive,
            overview_only: request.overview_only,
            exact: request.exact,
            semantic: request.semantic,
            sort: request.sort,
            since: request
                .since
                .as_deref()
                .map(|value| parse_since_date_filter(value, now))
                .transpose()?,
            until: request
                .until
                .as_deref()
                .map(|value| parse_until_date_filter(value, now))
                .transpose()?,
            max_results: parse_max_results(request.max_results),
        })
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: false,
            semantic: false,
            sort: SearchSort::Relevance,
            since: None,
            until: None,
            max_results: Some(DEFAULT_MAX_RESULTS),
        }
    }
}

fn parse_max_results(value: Option<i32>) -> Option<usize> {
    match value {
        Some(n) if n < 0 => None,
        Some(n) => Some(n as usize),
        None => Some(DEFAULT_MAX_RESULTS),
    }
}

enum DateBoundary {
    StartOfDay,
    EndOfDay,
}

pub fn parse_since_date_filter(input: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    parse_date_filter(input, now, DateBoundary::StartOfDay)
}

pub fn parse_until_date_filter(input: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    parse_date_filter(input, now, DateBoundary::EndOfDay)
}

fn parse_date_filter(
    input: &str,
    now: DateTime<Utc>,
    date_boundary: DateBoundary,
) -> Result<DateTime<Utc>> {
    if let Some(relative_date) = parse_relative_date(input, now)? {
        return Ok(relative_date);
    }

    if let Ok(timestamp) = DateTime::parse_from_rfc3339(input) {
        return Ok(timestamp.with_timezone(&Utc));
    }

    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let time = match date_boundary {
            DateBoundary::StartOfDay => date.and_hms_nano_opt(0, 0, 0, 0),
            DateBoundary::EndOfDay => date.and_hms_nano_opt(23, 59, 59, 999_999_999),
        }
        .ok_or_else(|| anyhow!("Invalid date: {input}"))?;

        return Ok(Utc.from_utc_datetime(&time));
    }

    Err(anyhow!(
        "Invalid date '{input}'. Use YYYY-MM-DD, RFC3339, or relative values like 7d, 16h, 20m, 1M, or 1Y"
    ))
}

fn parse_relative_date(input: &str, now: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    if input.len() < 2 {
        return Ok(None);
    }

    let (amount, unit) = input.split_at(input.len() - 1);
    if !amount.chars().all(|character| character.is_ascii_digit()) {
        return Ok(None);
    }

    let amount = amount.parse::<u32>()?;
    let relative_date = match unit {
        "m" => now.checked_sub_signed(Duration::minutes(amount as i64)),
        "h" => now.checked_sub_signed(Duration::hours(amount as i64)),
        "d" => now.checked_sub_signed(Duration::days(amount as i64)),
        "w" => now.checked_sub_signed(Duration::weeks(amount as i64)),
        "M" => now.checked_sub_months(Months::new(amount)),
        "Y" => now.checked_sub_months(Months::new(
            amount
                .checked_mul(12)
                .ok_or_else(|| anyhow!("Relative year value is too large: {input}"))?,
        )),
        _ => return Ok(None),
    };

    relative_date
        .ok_or_else(|| anyhow!("Relative date is out of range: {input}"))
        .map(Some)
}

pub fn matches_date_range(insight: &insight::Insight, options: &SearchOptions) -> bool {
    if let Some(since) = options.since {
        if insight.last_updated < since {
            return false;
        }
    }

    if let Some(until) = options.until {
        if insight.last_updated > until {
            return false;
        }
    }

    true
}

fn sort_by_relevance(results: &mut [SearchResult]) {
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.topic.cmp(&b.topic).then_with(|| a.name.cmp(&b.name)))
    });
}

fn sort_by_requested_order(results: &mut [SearchResult], sort: SearchSort) {
    match sort {
        SearchSort::Relevance => sort_by_relevance(results),
        SearchSort::Updated => results.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.topic.cmp(&b.topic).then_with(|| a.name.cmp(&b.name)))
        }),
        SearchSort::Created => results.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.topic.cmp(&b.topic).then_with(|| a.name.cmp(&b.name)))
        }),
        SearchSort::LeastAccessed => sort_by_relevance(results),
    }
}

fn deduplicate_highest_scoring(results: &mut Vec<SearchResult>) {
    sort_by_relevance(results);

    let mut seen = std::collections::HashSet::new();
    results.retain(|result| {
        let key = (result.topic.clone(), result.name.clone());
        seen.insert(key)
    });
}

pub fn search(terms: &[String], options: &SearchOptions) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();

    // Include exact term matching if not in semantic-only mode
    if !options.semantic {
        results.extend(search_topic(terms, get_exact_match, 0.0, options)?);
    }

    // Include semantic search if not in exact-only mode
    if !options.exact {
        results.extend(search_topic(
            terms,
            get_semantic_match,
            SEMANTIC_SIMILARITY_THRESHOLD,
            options,
        )?);
    }

    // Note: Embedding search is handled asynchronously in the server handler
    // and merged with these results there

    deduplicate_highest_scoring(&mut results);
    sort_by_requested_order(&mut results, options.sort);

    if let Some(limit) = options.max_results {
        results.truncate(limit);
    }

    Ok(results)
}

/// Search a topic for matches based on a search strategy
fn search_topic(
    terms: &[String],
    search_strategy: fn(&insight::Insight, &[String], &SearchOptions) -> f32,
    threshold: f32,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();

    let insights_dir = insight::get_valid_insights_dir()?;
    let search_paths = get_search_paths(&insights_dir, options.topic.as_deref())?;

    for topic_path in search_paths {
        for entry in fs::read_dir(&topic_path)? {
            let entry = entry?;
            let path = entry.path();

            if insight::is_insight_file(&path) {
                let insight = insight::load_from_path(&path)?;
                if let Ok(Some(result)) =
                    search_insight(&insight, search_strategy, terms, threshold, options)
                {
                    results.push(result);
                }
            }
        }
    }

    Ok(results)
}

fn search_insight(
    insight: &insight::Insight,
    search_strategy: fn(&insight::Insight, &[String], &SearchOptions) -> f32,
    terms: &[String],
    threshold: f32,
    options: &SearchOptions,
) -> Result<Option<SearchResult>> {
    if !matches_date_range(insight, options) {
        return Ok(None);
    }

    let base_score = search_strategy(insight, terms, options);
    let score = base_score * usage_boost(insight);
    if score > threshold {
        Ok(Some(SearchResult {
            topic: insight.topic.to_string(),
            name: insight.name.to_string(),
            overview: insight.overview.to_string(),
            details: insight.details.to_string(),
            score,
            created_at: insight.created_at,
            updated_at: insight.last_updated,
        }))
    } else {
        Ok(None)
    }
}

pub fn usage_boost(insight: &insight::Insight) -> f32 {
    let access_total = (insight.retrieval_count + insight.search_hit_count) as f32;
    let activity_boost = (1.0 + access_total).ln() * 0.1;

    let recency_boost = match insight.last_accessed {
        Some(ts) => {
            let days_ago = (Utc::now() - ts).num_days().max(0) as f32;
            0.1 * (-days_ago / 90.0).exp()
        }
        None => 0.0,
    };

    let pin_boost = if insight.pinned { 0.15 } else { 0.0 };

    1.0 + activity_boost + recency_boost + pin_boost
}

fn get_normalized_content(insight: &insight::Insight, options: &SearchOptions) -> String {
    if options.overview_only {
        format!("{} {} {}", insight.topic, insight.name, insight.overview)
    } else {
        format!(
            "{} {} {} {}",
            insight.topic, insight.name, insight.overview, insight.details
        )
    }
}

fn get_normalized_terms(terms: &[String], options: &SearchOptions) -> Vec<String> {
    if options.case_sensitive {
        terms.to_vec()
    } else {
        terms
            .iter()
            .map(|t| t.to_lowercase())
            .collect::<Vec<String>>()
    }
}

fn get_exact_match(insight: &insight::Insight, terms: &[String], options: &SearchOptions) -> f32 {
    let normalized_content = get_normalized_content(insight, options);
    let normalized_terms = get_normalized_terms(terms, options);

    normalized_terms
        .iter()
        .map(|term| normalized_content.matches(term).count())
        .sum::<usize>() as f32
}

fn get_semantic_match(
    insight: &insight::Insight,
    terms: &[String],
    options: &SearchOptions,
) -> f32 {
    let normalized_content = get_normalized_content(insight, options);
    let normalized_terms = get_normalized_terms(terms, options);

    similarity::semantic(&normalized_terms.into_iter().collect(), &normalized_content)
}

/// Highlight search terms
fn highlight_keywords(text: &str, terms: &[String]) -> String {
    let mut result = text.to_string();

    let mut sorted = terms.to_vec();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));

    for term in sorted {
        if term.is_empty() {
            continue;
        }

        let term_lower = term.to_lowercase();
        let mut highlighted = String::new();
        let mut end = 0;

        let result_lower = result.to_lowercase();
        let mut start = 0;

        while let Some(pos) = result_lower[start..].find(&term_lower) {
            let abs_pos = start + pos;

            highlighted.push_str(&result[end..abs_pos]);

            let match_text = &result[abs_pos..abs_pos + term.len()];
            highlighted.push_str(&match_text.yellow().bold().to_string());

            end = abs_pos + term.len();
            start = end;
        }

        highlighted.push_str(&result[end..]);
        result = highlighted;
    }

    result
}

/// Build search paths based on topic filter
fn get_search_paths(insights_root: &Path, topic_filter: Option<&str>) -> Result<Vec<PathBuf>> {
    if let Some(topic) = topic_filter {
        Ok(vec![insights_root.join(topic)])
    } else {
        Ok(insight::get_topics()?
            .into_iter()
            .map(|topic| insights_root.join(topic))
            .collect())
    }
}

/// Display the combined search results
pub fn display_results(results: &[SearchResult], terms: &[String], overview_only: bool) {
    if results.is_empty() {
        println!("No matches found for: {}", terms.join(" ").yellow());
    } else {
        for result in results {
            display_single_result(result, terms, overview_only);
        }
    }
}

/// Display a single search result with keyword highlighting
fn display_single_result(result: &SearchResult, terms: &[String], overview_only: bool) {
    let header = format!(
        "=== {}/{} ===",
        result.topic.blue().bold(),
        result.name.yellow().bold()
    );

    println!("{header}");

    // Wrap and display the content with proper formatting
    let wrap_with = if header.len() < DEFAULT_TERMINAL_WIDTH {
        DEFAULT_TERMINAL_WIDTH
    } else {
        header.len()
    };

    let content = if overview_only {
        result.overview.to_string()
    } else {
        format!("{}\n\n{}", result.overview, result.details)
    };

    let highlighted_content = highlight_keywords(&content, terms);
    let wrapped_lines = wrap_text(&highlighted_content, wrap_with);
    for line in wrapped_lines {
        println!("{line}");
    }
    println!();
}

/// Wrap text to fit within a specified width
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::models::insight::Insight;
    use colored::control;
    use serial_test::serial;

    // Mock insight for testing
    fn create_test_insight() -> Insight {
        Insight::new(
            "test_topic".to_string(),
            "test_insight".to_string(),
            "This is a test overview with some content".to_string(),
            "This is detailed content with more information for testing purposes".to_string(),
        )
    }

    #[test]
    fn test_search_options_from_command_options() {
        let cmd_options = SearchCommandOptions {
            topic: Some("test_topic".to_string()),
            case_sensitive: true,
            overview_only: true,
            exact: false,
            semantic: true,
            sort: SearchSort::Updated,
            since: Some("7d".to_string()),
            until: None,
            max_results: 10,
        };

        let options = SearchOptions::from_command_options(&cmd_options).unwrap();

        assert_eq!(options.topic, Some("test_topic".to_string()));
        assert!(options.case_sensitive);
        assert!(options.overview_only);
        assert!(!options.exact);
        assert!(options.semantic);
        assert_eq!(options.sort, SearchSort::Updated);
        assert!(options.since.is_some());
        assert!(options.until.is_none());
    }

    #[test]
    fn test_parse_date_filter_absolute_date_boundaries() {
        let now = DateTime::parse_from_rfc3339("2026-04-24T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let since = parse_since_date_filter("2026-04-01", now).unwrap();
        let until = parse_until_date_filter("2026-04-01", now).unwrap();

        assert_eq!(
            since,
            DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        assert_eq!(
            until,
            DateTime::parse_from_rfc3339("2026-04-01T23:59:59.999999999Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn test_parse_date_filter_relative_values() {
        let now = DateTime::parse_from_rfc3339("2026-04-24T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            parse_since_date_filter("20m", now).unwrap(),
            now - Duration::minutes(20)
        );
        assert_eq!(
            parse_since_date_filter("16h", now).unwrap(),
            now - Duration::hours(16)
        );
        assert_eq!(
            parse_since_date_filter("7d", now).unwrap(),
            now - Duration::days(7)
        );
        assert_eq!(
            parse_since_date_filter("1M", now).unwrap(),
            DateTime::parse_from_rfc3339("2026-03-24T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        assert_eq!(
            parse_since_date_filter("1Y", now).unwrap(),
            DateTime::parse_from_rfc3339("2025-04-24T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn test_parse_date_filter_rejects_invalid_values() {
        let now = Utc::now();

        assert!(parse_since_date_filter("7q", now).is_err());
        assert!(parse_since_date_filter("yesterday", now).is_err());
    }

    #[test]
    fn test_get_normalized_content_overview_only() {
        let insight = create_test_insight();
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: true,
            exact: false,
            semantic: false,
            ..SearchOptions::default()
        };

        let content = get_normalized_content(&insight, &options);
        assert_eq!(
            content,
            "test_topic test_insight This is a test overview with some content"
        );
    }

    #[test]
    fn test_get_normalized_content_full_content() {
        let insight = create_test_insight();
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: false,
            semantic: false,
            ..SearchOptions::default()
        };

        let content = get_normalized_content(&insight, &options);
        let expected = "test_topic test_insight This is a test overview with some content This is detailed content with more information for testing purposes";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_get_normalized_terms_case_sensitive() {
        let terms = vec!["Test".to_string(), "CONTENT".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: true,
            overview_only: false,
            exact: false,
            semantic: false,
            ..SearchOptions::default()
        };

        let normalized = get_normalized_terms(&terms, &options);
        assert_eq!(normalized, vec!["Test".to_string(), "CONTENT".to_string()]);
    }

    #[test]
    fn test_get_normalized_terms_case_insensitive() {
        let terms = vec!["Test".to_string(), "CONTENT".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: false,
            semantic: false,
            ..SearchOptions::default()
        };

        let normalized = get_normalized_terms(&terms, &options);
        assert_eq!(normalized, vec!["test".to_string(), "content".to_string()]);
    }

    #[test]
    fn test_get_exact_match_single_term() {
        let insight = create_test_insight();
        let terms = vec!["test".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let score = get_exact_match(&insight, &terms, &options);
        // "test" appears in topic, name, and content multiple times
        assert!(score > 2.0); // Should find multiple matches
    }

    #[test]
    fn test_get_exact_match_multiple_terms() {
        let insight = create_test_insight();
        let terms = vec!["test".to_string(), "content".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let score = get_exact_match(&insight, &terms, &options);
        // Score should be sum of matches for both terms
        assert!(score >= 4.0);
    }

    #[test]
    fn test_get_exact_match_case_sensitive() {
        let insight = create_test_insight();
        let terms = vec!["Test".to_string()]; // Capital T
        let options = SearchOptions {
            topic: None,
            case_sensitive: true,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let score = get_exact_match(&insight, &terms, &options);
        // Should find fewer matches due to case sensitivity
        let insensitive_score = get_exact_match(
            &insight,
            &["test".to_string()],
            &SearchOptions {
                topic: None,
                case_sensitive: false,
                overview_only: false,
                exact: true,
                semantic: false,
                ..SearchOptions::default()
            },
        );

        assert!(insensitive_score >= score); // Case insensitive should find more matches
    }

    #[test]
    fn test_get_exact_match_no_matches() {
        let insight = create_test_insight();
        let terms = vec!["nonexistent".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let score = get_exact_match(&insight, &terms, &options);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_search_insight_above_threshold() {
        let insight = create_test_insight();
        let terms = vec!["test".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let result = search_insight(&insight, get_exact_match, &terms, 0.0, &options).unwrap();

        assert!(result.is_some());
        let search_result = result.unwrap();
        assert_eq!(search_result.topic, "test_topic");
        assert_eq!(search_result.name, "test_insight");
        assert_eq!(search_result.overview, insight.overview);
        assert_eq!(search_result.details, insight.details);
        assert!(search_result.score > 0.0);
    }

    #[test]
    fn test_search_insight_below_threshold() {
        let insight = create_test_insight();
        let terms = vec!["nonexistent".to_string()];
        let options = SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true,
            semantic: false,
            ..SearchOptions::default()
        };

        let result = search_insight(&insight, get_exact_match, &terms, 1.0, &options).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_highlight_keywords_basic() {
        // Force color output for this test
        control::set_override(true);

        let text = "This is a test string with test content";
        let terms = vec!["test".to_string()];

        let highlighted = highlight_keywords(text, &terms);
        // Should contain ANSI color codes for highlighting
        assert!(highlighted.contains("\x1b[")); // ANSI escape codes
        assert!(highlighted.contains("test")); // Original text should still be there

        // Reset to default behavior
        control::unset_override();
    }

    #[test]
    #[serial]
    fn test_highlight_keywords_multiple_terms() {
        control::set_override(true);

        let text = "This is a test string with test content and more content";
        let terms = vec!["test".to_string(), "content".to_string()];

        let highlighted = highlight_keywords(text, &terms);
        assert!(highlighted.contains("\x1b[")); // ANSI escape codes
        assert!(highlighted.contains("test"));
        assert!(highlighted.contains("content"));

        control::unset_override();
    }

    #[test]
    fn test_highlight_keywords_empty_terms() {
        let text = "This is a test string";
        let terms = vec![];

        let highlighted = highlight_keywords(text, &terms);
        assert_eq!(highlighted, text); // Should return unchanged
    }

    #[test]
    fn test_highlight_keywords_empty_term() {
        let text = "This is a test string";
        let terms = vec!["".to_string()];

        let highlighted = highlight_keywords(text, &terms);
        assert_eq!(highlighted, text); // Should return unchanged
    }

    #[test]
    #[serial]
    fn test_highlight_keywords_case_insensitive() {
        control::set_override(true);

        let text = "This is a TEST string";
        let terms = vec!["test".to_string()];

        let highlighted = highlight_keywords(text, &terms);
        assert!(highlighted.contains("\x1b[")); // Should still highlight despite case difference

        control::unset_override();
    }

    #[test]
    fn test_get_search_paths_with_topic_filter() {
        use std::path::Path;

        let root = Path::new("/test/root");
        let topic_filter = Some("specific_topic");

        let paths = get_search_paths(root, topic_filter).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], root.join("specific_topic"));
    }

    #[test]
    fn test_get_search_paths_without_topic_filter() {

        // This test would require mocking get_topics(), which is filesystem dependent
        // For now, we'll skip this as it's more integration than unit test
        // In a real scenario, we'd inject the topic list as a dependency
    }

    #[test]
    fn test_display_single_result() {
        let result = SearchResult {
            topic: "test_topic".to_string(),
            name: "test_insight".to_string(),
            overview: "Test overview".to_string(),
            details: "Test details".to_string(),
            score: 2.5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let terms = vec!["test".to_string()];

        // This function prints to stdout, so we can't easily test the output
        // In a real scenario, we'd modify it to accept a writer parameter
        // For now, just ensure it doesn't panic
        display_single_result(&result, &terms, false);
    }

    #[test]
    fn test_display_results_empty() {
        let results: Vec<SearchResult> = vec![];
        let terms = vec!["test".to_string()];

        // Should not panic when displaying empty results
        display_results(&results, &terms, false);
    }

    #[test]
    fn test_display_results_with_data() {
        let results = vec![
            SearchResult {
                topic: "topic1".to_string(),
                name: "insight1".to_string(),
                overview: "Overview 1".to_string(),
                details: "Details 1".to_string(),
                score: 1.0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            SearchResult {
                topic: "topic2".to_string(),
                name: "insight2".to_string(),
                overview: "Overview 2".to_string(),
                details: "Details 2".to_string(),
                score: 2.0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ];

        let terms = vec!["test".to_string()];

        // Should not panic when displaying results
        display_results(&results, &terms, false);
    }
}
