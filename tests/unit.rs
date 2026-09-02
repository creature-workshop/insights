#[cfg(test)]
mod insight_tests {
    use anyhow::Result;
    use insights::server::models::insight::{self, Insight};
    use insights::server::services::search;
    use insights::server::types::SearchSort;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    /// Points the store and this machine's usage counters at a fresh temp
    /// directory, so a test run never reads or writes the operator's insights.
    fn setup_temp_insights_root(test_name: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let _unique_var = format!("INSIGHTS_ROOT_{}", test_name.to_uppercase());
        env::set_var("INSIGHTS_ROOT", temp_dir.path());
        env::set_var("INSIGHTS_USAGE_PATH", temp_dir.path().join("usage.json"));
        temp_dir
    }

    fn timestamp(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    #[serial]
    fn test_insight_creation_and_file_path() {
        let insight = Insight::new(
            "test_topic".to_string(),
            "test_name".to_string(),
            "Test overview".to_string(),
            "Test details".to_string(),
        );

        assert_eq!(insight.topic, "test_topic");
        assert_eq!(insight.name, "test_name");
        assert_eq!(insight.overview, "Test overview");
        assert_eq!(insight.details, "Test details");
    }

    #[test]
    #[serial]
    fn test_get_insights_root_with_env_var() -> Result<()> {
        let _temp = setup_temp_insights_root("root_test");
        let root = insight::get_insights_root()?;
        assert!(root.to_string_lossy().contains("tmp"));
        Ok(())
    }

    #[test]
    #[serial]
    fn test_save_and_load_insight() -> Result<()> {
        let _temp = setup_temp_insights_root("save_load");

        let insight = Insight::new(
            "save_test".to_string(),
            "test_insight".to_string(),
            "Save test overview".to_string(),
            "Save test details".to_string(),
        );

        // Save the insight
        insight::save(&insight)?;

        // Load it back
        let loaded = insight::load("save_test", "test_insight")?;
        assert_eq!(loaded.overview, "Save test overview");
        assert_eq!(loaded.details, "Save test details");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_save_duplicate_insight_fails() -> Result<()> {
        let _temp = setup_temp_insights_root("dup_test");

        let insight = Insight::new(
            "dup_test".to_string(),
            "duplicate".to_string(),
            "First save".to_string(),
            "Details".to_string(),
        );

        insight::save(&insight)?;

        // Try to save again - should fail
        let result = insight::save(&insight);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_load_nonexistent_insight() {
        let _temp = setup_temp_insights_root("load_none");

        let result = insight::load("nonexistent", "insight");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    #[serial]
    fn test_update_insight() -> Result<()> {
        let _temp = setup_temp_insights_root("update_test");

        let mut insight = Insight::new(
            "update_test".to_string(),
            "updateable".to_string(),
            "Original overview".to_string(),
            "Original details".to_string(),
        );

        insight::save(&insight)?;

        // Update just overview
        insight::update(&mut insight, Some("Updated overview"), None)?;
        assert_eq!(insight.overview, "Updated overview");
        assert_eq!(insight.details, "Original details");

        // Update just details
        insight::update(&mut insight, None, Some("Updated details"))?;
        assert_eq!(insight.overview, "Updated overview");
        assert_eq!(insight.details, "Updated details");

        // Reload to verify persistence
        let reloaded = insight::load("update_test", "updateable")?;
        assert_eq!(reloaded.overview, "Updated overview");
        assert_eq!(reloaded.details, "Updated details");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_update_with_no_changes_fails() -> Result<()> {
        let _temp = setup_temp_insights_root("no_update");

        let mut insight = Insight::new(
            "no_update".to_string(),
            "test".to_string(),
            "Overview".to_string(),
            "Details".to_string(),
        );

        insight::save(&insight)?;

        let result = insight::update(&mut insight, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("At least one"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_delete_insight() -> Result<()> {
        let _temp = setup_temp_insights_root("delete_test");

        let insight = Insight::new(
            "delete_test".to_string(),
            "deletable".to_string(),
            "To be deleted".to_string(),
            "Will be gone".to_string(),
        );

        insight::save(&insight)?;

        // Verify it exists
        assert!(insight::load("delete_test", "deletable").is_ok());

        // Delete it
        insight::delete(&insight)?;

        // Verify it's gone
        assert!(insight::load("delete_test", "deletable").is_err());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_delete_nonexistent_insight() {
        let _temp = setup_temp_insights_root("delete_none");

        let insight = Insight::new(
            "ghost".to_string(),
            "phantom".to_string(),
            "Never existed".to_string(),
            "Not there".to_string(),
        );

        let result = insight::delete(&insight);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    #[serial]
    fn test_parse_insight_content_valid() -> Result<()> {
        let content = "---\ntopic: \"TestTopic\"\nname: \"TestName\"\noverview: \"This is the overview\\nSpanning multiple lines\"\n---\n\n# Details\nThis is the details section\nWith more content";

        let (metadata, details) = insight::parse_insight_with_metadata(content)?;
        assert_eq!(metadata.topic, "TestTopic");
        assert_eq!(metadata.name, "TestName");
        assert_eq!(
            metadata.overview,
            "This is the overview\nSpanning multiple lines"
        );
        assert_eq!(details, "This is the details section\nWith more content");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_parse_insight_content_minimal() -> Result<()> {
        let content = "---\ntopic: \"MinimalTopic\"\nname: \"MinimalName\"\noverview: Simple overview\n---\n\n# Details\n";

        let (metadata, details) = insight::parse_insight_with_metadata(content)?;
        assert_eq!(metadata.topic, "MinimalTopic");
        assert_eq!(metadata.name, "MinimalName");
        assert_eq!(metadata.overview, "Simple overview");
        assert_eq!(details, "");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_parse_insight_content_trailing_metadata() -> Result<()> {
        let content = r#"---
topic: temporal_test
name: footer_metadata
overview: Simple overview
---

# Details
Details stay as the insight body.

---
metadata:
  created_at: 2024-01-15T10:30:00Z
  last_updated: 2024-01-20T14:45:00Z
  update_count: 5
"#;

        let (metadata, details) = insight::parse_insight_with_metadata(content)?;

        assert_eq!(metadata.topic, "temporal_test");
        assert_eq!(metadata.name, "footer_metadata");
        assert_eq!(details, "Details stay as the insight body.");
        assert_eq!(
            metadata.created_at,
            chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc)
        );
        assert_eq!(
            metadata.last_updated,
            chrono::DateTime::parse_from_rfc3339("2024-01-20T14:45:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc)
        );
        assert_eq!(metadata.update_count, 5);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_parse_insight_content_legacy_no_frontmatter() {
        let content = "This is not valid format";

        let result = insight::parse_insight_with_metadata(content);
        assert!(result.is_ok());
        let (metadata, details) = result.unwrap();
        assert_eq!(metadata.overview, "This is not valid format");
        assert_eq!(details, "");
    }

    #[test]
    #[serial]
    fn test_parse_insight_content_legacy_multiline_no_frontmatter() {
        let content = "Overview line\nThis is details\nMore details";

        let result = insight::parse_insight_with_metadata(content);
        assert!(result.is_ok());
        let (metadata, details) = result.unwrap();
        assert_eq!(metadata.overview, "Overview line");
        assert_eq!(details, "This is details\nMore details");
    }

    #[test]
    #[serial]
    fn test_get_topics_empty() -> Result<()> {
        let _temp = setup_temp_insights_root("topics_empty");

        let topics = insight::get_topics()?;
        assert!(topics.is_empty());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_topics_with_data() -> Result<()> {
        let _temp = setup_temp_insights_root("topics_data");

        // Create insights in different topics
        let insight1 = Insight::new(
            "alpha".to_string(),
            "test1".to_string(),
            "O1".to_string(),
            "D1".to_string(),
        );
        let insight2 = Insight::new(
            "beta".to_string(),
            "test2".to_string(),
            "O2".to_string(),
            "D2".to_string(),
        );
        let insight3 = Insight::new(
            "alpha".to_string(),
            "test3".to_string(),
            "O3".to_string(),
            "D3".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;
        insight::save(&insight3)?;

        let topics = insight::get_topics()?;
        assert_eq!(topics.len(), 2);
        assert!(topics.contains(&"alpha".to_string()));
        assert!(topics.contains(&"beta".to_string()));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_insights_all() -> Result<()> {
        let _temp = setup_temp_insights_root("insights_all");

        let insight1 = Insight::new(
            "topic1".to_string(),
            "insight1".to_string(),
            "O1".to_string(),
            "D1".to_string(),
        );
        let insight2 = Insight::new(
            "topic1".to_string(),
            "insight2".to_string(),
            "O2".to_string(),
            "D2".to_string(),
        );
        let insight3 = Insight::new(
            "topic2".to_string(),
            "insight3".to_string(),
            "O3".to_string(),
            "D3".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;
        insight::save(&insight3)?;

        let insights = insight::get_insights(None)?;
        assert_eq!(insights.len(), 3);

        // Should be sorted by name
        assert_eq!(insights[0].topic, "topic1");
        assert_eq!(insights[0].name, "insight1");
        assert_eq!(insights[1].topic, "topic1");
        assert_eq!(insights[1].name, "insight2");
        assert_eq!(insights[2].topic, "topic2");
        assert_eq!(insights[2].name, "insight3");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_insights_filtered() -> Result<()> {
        let _temp = setup_temp_insights_root("insights_filtered");

        let insight1 = Insight::new(
            "filter_topic".to_string(),
            "insight1".to_string(),
            "O1".to_string(),
            "D1".to_string(),
        );
        let insight2 = Insight::new(
            "other_topic".to_string(),
            "insight2".to_string(),
            "O2".to_string(),
            "D2".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;

        let insights = insight::get_insights(Some("filter_topic"))?;
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].topic, "filter_topic");
        assert_eq!(insights[0].name, "insight1");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_insights_nonexistent_topic() -> Result<()> {
        let _temp = setup_temp_insights_root("insights_none");

        let insights = insight::get_insights(Some("nonexistent"))?;
        assert!(insights.is_empty());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_with_highlighting() -> Result<()> {
        let _temp = setup_temp_insights_root("search_highlight");

        // Create test insights
        let insight1 = Insight::new(
            "test_topic".to_string(),
            "rust_code".to_string(),
            "This is about Rust programming language".to_string(),
            "Rust is a systems programming language that runs blazingly fast".to_string(),
        );
        let insight2 = Insight::new(
            "test_topic".to_string(),
            "other_lang".to_string(),
            "This is about Python programming".to_string(),
            "Python is great for rapid development and scripting".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;

        // Test search functionality by creating SearchOptions directly
        let search_options = search::SearchOptions {
            topic: None,
            case_sensitive: false,
            overview_only: false,
            exact: true, // Use exact search which doesn't require neural features
            semantic: false,
            ..search::SearchOptions::default()
        };

        let results = search::search(&["rust".to_string()], &search_options)?;

        // Should find the rust insight
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust_code");
        assert!(results[0].score > 0.0);

        // Test that search results can be displayed (this tests our highlighting integration)
        // The highlighting happens in the display function, so we mainly test that it doesn't crash
        search::display_results(&results, &["rust".to_string()], false);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_default_sort_preserves_relevance_ordering() -> Result<()> {
        let _temp = setup_temp_insights_root("search_relevance_sort");

        let mut older_high_relevance = Insight::new(
            "recency".to_string(),
            "older_high_relevance".to_string(),
            "needle needle".to_string(),
            "needle".to_string(),
        );
        older_high_relevance.created_at = timestamp("2024-01-01T00:00:00Z");
        older_high_relevance.last_updated = timestamp("2024-01-02T00:00:00Z");

        let mut newer_low_relevance = Insight::new(
            "recency".to_string(),
            "newer_low_relevance".to_string(),
            "needle".to_string(),
            "no extra matches".to_string(),
        );
        newer_low_relevance.created_at = timestamp("2024-02-01T00:00:00Z");
        newer_low_relevance.last_updated = timestamp("2024-02-02T00:00:00Z");

        insight::save(&older_high_relevance)?;
        insight::save(&newer_low_relevance)?;

        let results = search::search(
            &["needle".to_string()],
            &search::SearchOptions {
                exact: true,
                sort: SearchSort::Relevance,
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "older_high_relevance");
        assert_eq!(results[1].name, "newer_low_relevance");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_can_sort_by_updated_timestamp() -> Result<()> {
        let _temp = setup_temp_insights_root("search_updated_sort");

        let mut older = Insight::new(
            "recency".to_string(),
            "older".to_string(),
            "needle needle".to_string(),
            "needle".to_string(),
        );
        older.created_at = timestamp("2024-01-01T00:00:00Z");
        older.last_updated = timestamp("2024-01-02T00:00:00Z");

        let mut newer = Insight::new(
            "recency".to_string(),
            "newer".to_string(),
            "needle".to_string(),
            "no extra matches".to_string(),
        );
        newer.created_at = timestamp("2024-02-01T00:00:00Z");
        newer.last_updated = timestamp("2024-02-02T00:00:00Z");

        insight::save(&older)?;
        insight::save(&newer)?;

        let results = search::search(
            &["needle".to_string()],
            &search::SearchOptions {
                exact: true,
                sort: SearchSort::Updated,
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "newer");
        assert_eq!(results[1].name, "older");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_can_sort_by_created_timestamp() -> Result<()> {
        let _temp = setup_temp_insights_root("search_created_sort");

        let mut older_created = Insight::new(
            "recency".to_string(),
            "older_created".to_string(),
            "needle".to_string(),
            "updated later".to_string(),
        );
        older_created.created_at = timestamp("2024-01-01T00:00:00Z");
        older_created.last_updated = timestamp("2024-03-01T00:00:00Z");

        let mut newer_created = Insight::new(
            "recency".to_string(),
            "newer_created".to_string(),
            "needle".to_string(),
            "updated earlier".to_string(),
        );
        newer_created.created_at = timestamp("2024-02-01T00:00:00Z");
        newer_created.last_updated = timestamp("2024-02-02T00:00:00Z");

        insight::save(&older_created)?;
        insight::save(&newer_created)?;

        let results = search::search(
            &["needle".to_string()],
            &search::SearchOptions {
                exact: true,
                sort: SearchSort::Created,
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "newer_created");
        assert_eq!(results[1].name, "older_created");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_filters_by_updated_date_range() -> Result<()> {
        let _temp = setup_temp_insights_root("search_date_filter");

        let mut before_range = Insight::new(
            "recency".to_string(),
            "before_range".to_string(),
            "needle".to_string(),
            "too old".to_string(),
        );
        before_range.last_updated = timestamp("2024-01-01T00:00:00Z");

        let mut in_range = Insight::new(
            "recency".to_string(),
            "in_range".to_string(),
            "needle".to_string(),
            "just right".to_string(),
        );
        in_range.last_updated = timestamp("2024-01-15T00:00:00Z");

        let mut after_range = Insight::new(
            "recency".to_string(),
            "after_range".to_string(),
            "needle".to_string(),
            "too new".to_string(),
        );
        after_range.last_updated = timestamp("2024-02-01T00:00:00Z");

        insight::save(&before_range)?;
        insight::save(&in_range)?;
        insight::save(&after_range)?;

        let results = search::search(
            &["needle".to_string()],
            &search::SearchOptions {
                exact: true,
                since: Some(timestamp("2024-01-10T00:00:00Z")),
                until: Some(timestamp("2024-01-20T00:00:00Z")),
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "in_range");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_temporal_metadata_on_creation() -> Result<()> {
        let before_creation = chrono::Utc::now();

        let insight = Insight::new(
            "temporal_test".to_string(),
            "creation_test".to_string(),
            "Testing temporal metadata on creation".to_string(),
            "This tests that new insights have proper timestamps".to_string(),
        );

        let after_creation = chrono::Utc::now();

        // Check that timestamps are set
        assert!(insight.created_at >= before_creation);
        assert!(insight.created_at <= after_creation);
        assert!(insight.last_updated >= before_creation);
        assert!(insight.last_updated <= after_creation);

        // Check that created_at and last_updated are the same on creation
        assert_eq!(insight.created_at, insight.last_updated);

        // Check that update count starts at 0
        assert_eq!(insight.update_count, 0);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_temporal_metadata_on_update() -> Result<()> {
        let _temp = setup_temp_insights_root("temporal_update");

        // Create and save an initial insight
        let mut insight = Insight::new(
            "temporal_test".to_string(),
            "update_test".to_string(),
            "Original overview".to_string(),
            "Original details".to_string(),
        );

        let original_created_at = insight.created_at;
        let original_last_updated = insight.last_updated;

        insight::save(&insight)?;

        // Wait a bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update the insight
        insight::update(
            &mut insight,
            Some("Updated overview"),
            Some("Updated details"),
        )?;

        // Check that created_at hasn't changed
        assert_eq!(insight.created_at, original_created_at);

        // Check that last_updated has changed
        assert!(insight.last_updated > original_last_updated);

        // Check that update_count has increased
        assert_eq!(insight.update_count, 1);

        // Update again
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second_last_updated = insight.last_updated;
        insight::update(&mut insight, Some("Second update"), None)?;

        // Check that update_count increased again and last_updated changed
        assert_eq!(insight.update_count, 2);
        assert!(insight.last_updated > second_last_updated);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_temporal_metadata_serialization() -> Result<()> {
        let _temp = setup_temp_insights_root("temporal_serialization");

        // Create insight with known timestamps
        let mut insight = Insight::new(
            "serialization_test".to_string(),
            "temporal_metadata".to_string(),
            "Testing temporal serialization".to_string(),
            "This tests that temporal metadata is saved to files".to_string(),
        );

        // Modify timestamps to known values for testing
        insight.created_at = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        insight.last_updated = chrono::DateTime::parse_from_rfc3339("2024-01-20T14:45:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        insight.update_count = 5;

        // Save the insight
        insight::save(&insight)?;

        // Read the raw file content
        let file_path = insight::file_path(&insight)?;
        let file_content = std::fs::read_to_string(&file_path)?;

        // Verify temporal metadata is in the file
        let frontmatter = file_content
            .strip_prefix("---\n")
            .and_then(|content| {
                content
                    .split_once("\n---\n")
                    .map(|(frontmatter, _)| frontmatter)
            })
            .expect("serialized insight should start with frontmatter");
        assert!(!frontmatter.contains("created_at:"));
        assert!(!frontmatter.contains("last_updated:"));
        assert!(!frontmatter.contains("update_count:"));

        assert!(file_content.contains("\n---\nmetadata:\n"));
        assert!(file_content.contains("  created_at: 2024-01-15T10:30:00Z"));
        assert!(file_content.contains("  last_updated: 2024-01-20T14:45:00Z"));
        assert!(file_content.contains("  update_count: 5"));

        // Load the insight back from file
        let loaded_insight = insight::load("serialization_test", "temporal_metadata")?;

        // Verify temporal metadata was preserved
        assert_eq!(loaded_insight.created_at, insight.created_at);
        assert_eq!(loaded_insight.last_updated, insight.last_updated);
        assert_eq!(loaded_insight.update_count, insight.update_count);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_backwards_compatibility_frontmatter_temporal_fields() -> Result<()> {
        let content = r#"---
topic: legacy_test
name: old_temporal_insight
overview: This stores temporal metadata in frontmatter
created_at: 2024-01-15T10:30:00Z
last_updated: 2024-01-20T14:45:00Z
update_count: 3
---

# Details
This insight uses the old temporal metadata location.
"#;

        let (metadata, details) = insight::parse_insight_with_metadata(content)?;

        assert_eq!(metadata.topic, "legacy_test");
        assert_eq!(metadata.name, "old_temporal_insight");
        assert_eq!(
            metadata.created_at,
            chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc)
        );
        assert_eq!(
            metadata.last_updated,
            chrono::DateTime::parse_from_rfc3339("2024-01-20T14:45:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc)
        );
        assert_eq!(metadata.update_count, 3);
        assert_eq!(
            details,
            "This insight uses the old temporal metadata location."
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_backwards_compatibility_missing_temporal_fields() -> Result<()> {
        let _temp = setup_temp_insights_root("backwards_compat");

        // Create a legacy insight file without temporal metadata
        let legacy_content = r#"---
topic: legacy_test
name: old_insight
overview: This is a legacy insight without temporal metadata
---

# Details
This insight was created before temporal metadata was added.
"#;

        // Write the legacy file directly
        let insights_root = insight::get_insights_root()?;
        let topic_dir = insights_root.join("legacy_test");
        std::fs::create_dir_all(&topic_dir)?;
        let file_path = topic_dir.join("old_insight.insight.md");
        std::fs::write(&file_path, legacy_content)?;

        // Load the legacy insight
        let loaded_insight = insight::load("legacy_test", "old_insight")?;

        // Check that temporal metadata has default values
        assert_eq!(
            loaded_insight.created_at,
            chrono::DateTime::parse_from_rfc3339("2025-05-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc)
        );
        assert!(loaded_insight.last_updated <= chrono::Utc::now()); // Should be set to current time
        assert_eq!(loaded_insight.update_count, 0);

        // Check that other fields are correct
        assert_eq!(loaded_insight.topic, "legacy_test");
        assert_eq!(loaded_insight.name, "old_insight");
        assert_eq!(
            loaded_insight.overview,
            "This is a legacy insight without temporal metadata"
        );
        assert_eq!(
            loaded_insight.details,
            "This insight was created before temporal metadata was added."
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_exclude_filters_matching_results() -> Result<()> {
        let _temp = setup_temp_insights_root("search_exclude_basic");

        let insight1 = Insight::new(
            "lang".to_string(),
            "rust_guide".to_string(),
            "Rust programming guide".to_string(),
            "Rust is a systems language with zero-cost abstractions".to_string(),
        );
        let insight2 = Insight::new(
            "lang".to_string(),
            "python_guide".to_string(),
            "Python programming guide".to_string(),
            "Python is great for scripting and rapid prototyping".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;

        let results = search::search(
            &["programming".to_string()],
            &search::SearchOptions {
                exact: true,
                exclude: vec!["python".to_string()],
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust_guide");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_exclude_multiple_terms() -> Result<()> {
        let _temp = setup_temp_insights_root("search_exclude_multi");

        let insight1 = Insight::new(
            "tools".to_string(),
            "docker_setup".to_string(),
            "Docker container setup".to_string(),
            "How to configure Docker containers for development".to_string(),
        );
        let insight2 = Insight::new(
            "tools".to_string(),
            "k8s_deploy".to_string(),
            "Kubernetes deployment guide".to_string(),
            "Deploy apps to Kubernetes clusters".to_string(),
        );
        let insight3 = Insight::new(
            "tools".to_string(),
            "ci_pipeline".to_string(),
            "CI pipeline configuration".to_string(),
            "Setting up continuous integration pipelines".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;
        insight::save(&insight3)?;

        let results = search::search(
            &["tools".to_string()],
            &search::SearchOptions {
                exact: true,
                exclude: vec!["docker".to_string(), "kubernetes".to_string()],
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ci_pipeline");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_exclude_is_case_insensitive_by_default() -> Result<()> {
        let _temp = setup_temp_insights_root("search_exclude_case");

        let insight1 = Insight::new(
            "lang".to_string(),
            "rust_async".to_string(),
            "Async Rust patterns".to_string(),
            "Using tokio for ASYNC runtime in Rust".to_string(),
        );
        let insight2 = Insight::new(
            "lang".to_string(),
            "rust_sync".to_string(),
            "Synchronous Rust patterns".to_string(),
            "Blocking IO patterns in Rust".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;

        let results = search::search(
            &["rust".to_string()],
            &search::SearchOptions {
                exact: true,
                exclude: vec!["ASYNC".to_string()],
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust_sync");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_exclude_respects_case_sensitive_flag() -> Result<()> {
        let _temp = setup_temp_insights_root("search_exclude_case_sensitive");

        let insight1 = Insight::new(
            "lang".to_string(),
            "rust_async".to_string(),
            "Async Rust patterns".to_string(),
            "Using tokio for async runtime in Rust".to_string(),
        );
        let insight2 = Insight::new(
            "lang".to_string(),
            "rust_sync".to_string(),
            "Synchronous Rust patterns".to_string(),
            "Blocking IO patterns in Rust".to_string(),
        );

        insight::save(&insight1)?;
        insight::save(&insight2)?;

        // Excluding "ASYNC" with case_sensitive=true should NOT exclude the
        // insight that contains lowercase "async"
        let results = search::search(
            &["Rust".to_string()],
            &search::SearchOptions {
                exact: true,
                case_sensitive: true,
                exclude: vec!["ASYNC".to_string()],
                ..search::SearchOptions::default()
            },
        )?;

        assert_eq!(results.len(), 2);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search_exclude_with_no_positive_matches() -> Result<()> {
        let _temp = setup_temp_insights_root("search_exclude_no_match");

        let insight1 = Insight::new(
            "misc".to_string(),
            "something".to_string(),
            "Unrelated content".to_string(),
            "Nothing interesting here".to_string(),
        );

        insight::save(&insight1)?;

        let results = search::search(
            &["nonexistent_term".to_string()],
            &search::SearchOptions {
                exact: true,
                exclude: vec!["something".to_string()],
                ..search::SearchOptions::default()
            },
        )?;

        assert!(results.is_empty());

        Ok(())
    }
}

#[cfg(test)]
mod usage_tests {
    use anyhow::Result;
    use insights::server::models::insight::{self, Insight};
    use insights::server::models::usage::{self, AccessType};
    use serial_test::serial;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    /// Points the store and this machine's usage counters at a fresh temp
    /// directory, so a test run never reads or writes the operator's insights.
    fn setup_temp_insights_root() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("INSIGHTS_ROOT", temp_dir.path());
        env::set_var("INSIGHTS_USAGE_PATH", temp_dir.path().join("usage.json"));
        temp_dir
    }

    fn save_insight(topic: &str, name: &str) -> Result<()> {
        insight::save(&Insight::new(
            topic.to_string(),
            name.to_string(),
            "An overview".to_string(),
            "Some details".to_string(),
        ))
    }

    /// Writes an insight file in the pre-usage-store format, with the counters
    /// still in its trailing metadata block.
    fn write_legacy_insight(root: &std::path::Path, topic: &str, name: &str) -> Result<()> {
        let topic_dir = root.join(topic);
        fs::create_dir_all(&topic_dir)?;
        fs::write(
            topic_dir.join(format!("{name}.insight.md")),
            format!(
                "---\ntopic: {topic}\nname: {name}\noverview: An overview\n---\n\n\
                 # Details\nSome details\n\n---\nmetadata:\n  \
                 created_at: 2026-01-15T10:30:00Z\n  \
                 last_updated: 2026-01-20T14:45:00Z\n  \
                 update_count: 2\n  \
                 retrieval_count: 7\n  \
                 search_hit_count: 11\n  \
                 last_accessed: 2026-02-01T09:00:00Z\n  \
                 pinned: true\n"
            ),
        )?;
        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_reports_zeroes_for_an_untouched_insight() {
        let _temp = setup_temp_insights_root();

        let usage = usage::get("never", "touched");

        assert_eq!(usage.retrieval_count, 0);
        assert_eq!(usage.search_hit_count, 0);
        assert_eq!(usage.last_accessed, None);
    }

    #[test]
    #[serial]
    fn test_record_counts_retrievals_and_search_hits_separately() -> Result<()> {
        let _temp = setup_temp_insights_root();

        usage::record("rust", "async-patterns", AccessType::Retrieval)?;

        let usage = usage::get("rust", "async-patterns");
        assert_eq!(usage.retrieval_count, 1);
        assert_eq!(usage.search_hit_count, 0);
        assert!(usage.last_accessed.is_some());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_record_counts_every_access() -> Result<()> {
        let _temp = setup_temp_insights_root();

        usage::record("rust", "async-patterns", AccessType::Retrieval)?;
        usage::record("rust", "async-patterns", AccessType::Retrieval)?;
        usage::record("rust", "async-patterns", AccessType::SearchHit)?;

        let usage = usage::get("rust", "async-patterns");
        assert_eq!(usage.retrieval_count, 2);
        assert_eq!(usage.search_hit_count, 1);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_record_many_counts_every_insight_it_is_given() -> Result<()> {
        let _temp = setup_temp_insights_root();

        usage::record_many(
            &[
                ("rust".to_string(), "first".to_string()),
                ("rust".to_string(), "second".to_string()),
            ],
            AccessType::SearchHit,
        )?;

        assert_eq!(usage::get("rust", "first").search_hit_count, 1);
        assert_eq!(usage::get("rust", "second").search_hit_count, 1);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_counters_are_keyed_case_insensitively() -> Result<()> {
        let _temp = setup_temp_insights_root();

        usage::record("Rust", "Async-Patterns", AccessType::Retrieval)?;

        assert_eq!(usage::get("rust", "async-patterns").retrieval_count, 1);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_load_attaches_this_machines_counters_to_the_insight() -> Result<()> {
        let _temp = setup_temp_insights_root();
        save_insight("rust", "async-patterns")?;

        usage::record("rust", "async-patterns", AccessType::Retrieval)?;
        let loaded = insight::load("rust", "async-patterns")?;

        assert_eq!(loaded.retrieval_count, 1);
        assert!(loaded.last_accessed.is_some());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_saving_an_insight_writes_no_counters_into_its_file() -> Result<()> {
        let temp = setup_temp_insights_root();
        save_insight("rust", "async-patterns")?;

        let contents =
            fs::read_to_string(temp.path().join("rust").join("async-patterns.insight.md"))?;

        assert!(!contents.contains("retrieval_count"));
        assert!(!contents.contains("search_hit_count"));
        assert!(!contents.contains("last_accessed"));
        assert!(contents.contains("update_count"));
        assert!(contents.contains("pinned"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_reading_an_insight_leaves_its_file_byte_identical() -> Result<()> {
        let temp = setup_temp_insights_root();
        save_insight("rust", "async-patterns")?;

        let path = temp.path().join("rust").join("async-patterns.insight.md");
        let before = fs::read(&path)?;

        insight::load("rust", "async-patterns")?;
        usage::record("rust", "async-patterns", AccessType::Retrieval)?;
        usage::record_many(
            &[("rust".to_string(), "async-patterns".to_string())],
            AccessType::SearchHit,
        )?;

        assert_eq!(before, fs::read(&path)?);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_moves_footer_counters_into_the_usage_store() -> Result<()> {
        let temp = setup_temp_insights_root();
        write_legacy_insight(temp.path(), "rust", "legacy")?;

        insight::migrate_usage_footers()?;

        let usage = usage::get("rust", "legacy");
        assert_eq!(usage.retrieval_count, 7);
        assert_eq!(usage.search_hit_count, 11);
        assert!(usage.last_accessed.is_some());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_strips_counters_from_the_file_but_keeps_the_insight() -> Result<()> {
        let temp = setup_temp_insights_root();
        write_legacy_insight(temp.path(), "rust", "legacy")?;

        insight::migrate_usage_footers()?;

        let contents = fs::read_to_string(temp.path().join("rust").join("legacy.insight.md"))?;
        assert!(!contents.contains("retrieval_count"));
        assert!(!contents.contains("search_hit_count"));

        let loaded = insight::load("rust", "legacy")?;
        assert_eq!(loaded.overview, "An overview");
        assert_eq!(loaded.details, "Some details");
        assert_eq!(loaded.update_count, 2);
        assert!(loaded.pinned);
        assert_eq!(loaded.retrieval_count, 7);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_leaves_an_already_migrated_store_untouched() -> Result<()> {
        let temp = setup_temp_insights_root();
        write_legacy_insight(temp.path(), "rust", "legacy")?;

        insight::migrate_usage_footers()?;

        let path = temp.path().join("rust").join("legacy.insight.md");
        let after_first = fs::read(&path)?;
        usage::record("rust", "legacy", AccessType::Retrieval)?;

        insight::migrate_usage_footers()?;

        // The rerun neither rewrites the file nor puts the migrated 7 back over
        // the retrieval recorded since.
        assert_eq!(after_first, fs::read(&path)?);
        assert_eq!(usage::get("rust", "legacy").retrieval_count, 8);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_leaves_a_file_without_counters_byte_identical() -> Result<()> {
        let temp = setup_temp_insights_root();

        // The layout insights were written in before counters moved to the
        // footer: timestamps in the frontmatter, no counters anywhere.
        let path = temp.path().join("rust").join("older-layout.insight.md");
        fs::create_dir_all(path.parent().unwrap())?;
        let original = "---\ntopic: rust\nname: older-layout\noverview: An overview\n                        created_at: 2026-04-05T07:47:15Z\nlast_updated: 2026-04-05T07:47:15Z\n                        update_count: 0\n---\n\n# Details\nSome details";
        fs::write(&path, original)?;

        insight::migrate_usage_footers()?;

        assert_eq!(fs::read_to_string(&path)?, original);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_leaves_an_empty_file_alone() -> Result<()> {
        let temp = setup_temp_insights_root();

        let path = temp.path().join("rust").join("empty.insight.md");
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, "")?;

        insight::migrate_usage_footers()?;

        assert_eq!(fs::read_to_string(&path)?, "");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_records_nothing_for_files_that_carry_no_counters() -> Result<()> {
        let _temp = setup_temp_insights_root();
        save_insight("rust", "fresh")?;

        insight::migrate_usage_footers()?;

        assert_eq!(usage::get("rust", "fresh").retrieval_count, 0);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_migration_still_moves_counters_when_a_store_already_exists() -> Result<()> {
        let temp = setup_temp_insights_root();

        // An older build recording into the files again — after a rollback, say
        // — leaves counters behind that this machine already has a store for.
        usage::record("rust", "legacy", AccessType::Retrieval)?;
        write_legacy_insight(temp.path(), "rust", "legacy")?;

        insight::migrate_usage_footers()?;

        let contents = fs::read_to_string(temp.path().join("rust").join("legacy.insight.md"))?;
        assert!(!contents.contains("retrieval_count"));

        // The file's 7 beats the store's 1, and the file's 11 beats its absent
        // search hits.
        let usage = usage::get("rust", "legacy");
        assert_eq!(usage.retrieval_count, 7);
        assert_eq!(usage.search_hit_count, 11);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_adopting_counters_never_lowers_one_the_store_already_holds() -> Result<()> {
        let temp = setup_temp_insights_root();
        write_legacy_insight(temp.path(), "rust", "legacy")?;

        for _ in 0..9 {
            usage::record("rust", "legacy", AccessType::SearchHit)?;
        }

        insight::migrate_usage_footers()?;

        // The file carries 11 search hits and the store 9; 11 wins. The file's
        // 7 retrievals arrive against the store's none.
        let usage = usage::get("rust", "legacy");
        assert_eq!(usage.search_hit_count, 11);
        assert_eq!(usage.retrieval_count, 7);

        Ok(())
    }
}
