//! REST server startup and configuration

use anyhow::Result;
use axum::serve;
use bentley::daemon_logs::DaemonLogs;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::server::{
    middleware::{self, init_global_logger},
    models::insight,
    routing::create_router,
};

#[cfg(feature = "ml-features")]
use crate::server::{
    middleware::init_global_vector_db,
    services::{lancedb::LanceDbVectorDatabase, vector_database::BoxedVectorDatabase},
};

/// Start the REST server
#[cfg(not(tarpaulin_include))] // Skip coverage - server lifecycle and daemon logs initialization
pub async fn start_server(addr: SocketAddr) -> Result<()> {
    // Initialize daemon logs for persistent logging
    let logs_path = get_server_logs_path();
    let daemon_logs = Arc::new(DaemonLogs::new(&logs_path)?);

    init_global_logger(daemon_logs.clone())
        .map_err(|_| anyhow::anyhow!("Failed to initialize global logger"))?;

    middleware::set_log_level(configured_log_level());

    // Insight files written before the usage store carry counters that belong
    // to this machine; move them before anything serves a read.
    if let Err(e) = insight::migrate_usage_footers() {
        daemon_logs
            .warn(
                &format!("Failed to migrate usage counters out of insight files: {e}"),
                "insights-server",
            )
            .await;
    }

    // Initialize vector database service (only with ml-features)
    #[cfg(feature = "ml-features")]
    {
        let lancedb_path = get_lancedb_data_path();
        let lancedb_service = LanceDbVectorDatabase::new(lancedb_path, "insights_embeddings")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize vector database: {}", e))?;

        let vector_db_service = Arc::new(BoxedVectorDatabase::new(lancedb_service));

        // Initialize global vector database service
        init_global_vector_db(vector_db_service)
            .map_err(|_| anyhow::anyhow!("Failed to initialize global vector database service"))?;

        daemon_logs
            .info(
                "Vector database service initialized successfully",
                "insights-server",
            )
            .await;
    }

    #[cfg(not(feature = "ml-features"))]
    {
        daemon_logs
            .info(
                "Running in lightweight mode (no ML features)",
                "insights-server",
            )
            .await;
    }

    // Log server startup
    daemon_logs
        .info(
            &format!("Starting insights REST server on {addr}"),
            "insights-server",
        )
        .await;
    bentley::info!(&format!("Starting insights REST server on {addr}"));

    // Create the router with additional middleware
    let app = create_router().layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive()), // TODO: Configure CORS properly for production
    );

    // Create listener
    let listener = TcpListener::bind(addr).await?;
    daemon_logs
        .info(&format!("Server listening on {addr}"), "insights-server")
        .await;

    // Start serving
    match serve(listener, app).await {
        Ok(_) => {
            daemon_logs
                .info("Server shutdown gracefully", "insights-server")
                .await;
            Ok(())
        }
        Err(e) => {
            daemon_logs
                .error(&format!("Server error: {e}"), "insights-server")
                .await;
            Err(anyhow::anyhow!("Server error: {e}"))
        }
    }
}

/// The log level configured for this process: INSIGHTS_LOG_LEVEL when set, Info otherwise
fn configured_log_level() -> middleware::LogLevel {
    std::env::var("INSIGHTS_LOG_LEVEL")
        .map(|s| middleware::LogLevel::parse(&s))
        .unwrap_or(middleware::LogLevel::Info)
}

/// Get the path for server logs
#[cfg(not(tarpaulin_include))] // Skip coverage - filesystem path operations
fn get_server_logs_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::Path::new("/tmp").to_path_buf())
        .join(".blizz")
        .join("persistent")
        .join("insights")
        .join("server-logs.jsonl")
}

/// Get the path for LanceDB data storage
#[cfg(all(feature = "ml-features", not(tarpaulin_include)))] // Skip coverage - filesystem path operations
fn get_lancedb_data_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::Path::new("/tmp").to_path_buf())
        .join(".blizz")
        .join("volatile")
        .join("insights")
        .join("lancedb")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_configured_log_level_reads_the_environment() {
        std::env::set_var("INSIGHTS_LOG_LEVEL", "debug");
        assert_eq!(configured_log_level(), middleware::LogLevel::Debug);

        std::env::remove_var("INSIGHTS_LOG_LEVEL");
        assert_eq!(configured_log_level(), middleware::LogLevel::Info);
    }

    #[test]
    #[serial]
    fn test_debug_configuration_reaches_should_log() {
        std::env::set_var("INSIGHTS_LOG_LEVEL", "debug");
        middleware::set_log_level(configured_log_level());
        assert!(middleware::should_log(middleware::LogLevel::Debug));

        std::env::remove_var("INSIGHTS_LOG_LEVEL");
        middleware::set_log_level(middleware::LogLevel::Info);
    }
}
