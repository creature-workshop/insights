//! Insights REST Server
//!
//! HTTP REST API server for the insights knowledge management system.
//! Provides RESTful endpoints for insight management, search, and administration.

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

use insights::cli::server_config;
use insights::server::startup::start_server;

#[derive(Parser)]
#[command(name = "insights_server")]
#[command(about = "Insights REST API Server")]
#[command(version = concat!(env!("CARGO_PKG_VERSION"), ", courtesy of Blizz and Kernelle Software"))]
struct Args {
    /// Listen address (overrides `INSIGHTS_SERVER_URL` when set)
    #[arg(long)]
    bind: Option<SocketAddr>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let bind = if let Some(addr) = args.bind {
        addr
    } else {
        server_config::get_insights_server_bind()?
    };

    // Initialize logging with reduced verbosity for Lance and other noisy libraries
    let filter = if args.verbose {
        // Verbose mode: info for most, but reduced for noisy libraries
        EnvFilter::new("info,lance=warn,lance_datafusion=warn,datafusion=warn")
    } else {
        // Normal mode: info for insights, warn for everything else
        EnvFilter::new("insights=info,lance=error,lance_datafusion=error,datafusion=error,warn")
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    bentley::info!(&format!(
        "Starting Insights REST Server v{}",
        env!("CARGO_PKG_VERSION")
    ));
    bentley::info!(&format!("Binding to address: {bind}"));

    // Start the server
    start_server(bind).await?;

    Ok(())
}
