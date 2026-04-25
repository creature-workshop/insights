//! Default HTTP URL and listen address for the insights client and `insights_server`.
//!
//! Set **`INSIGHTS_SERVER_URL`** (e.g. `http://127.0.0.1:2020`) to align the CLI, auto-spawned
//! daemon, and the `insights_server` binary (when the variable is set).

use anyhow::{anyhow, Result};
use std::net::SocketAddr;
use url::Url;

/// When `INSIGHTS_SERVER_URL` is unset, the client and `insights_server` default bind use this.
pub const DEFAULT_INSIGHTS_SERVER_URL: &str = "http://127.0.0.1:2020";

/// Base URL for HTTP requests (same as [`DEFAULT_INSIGHTS_SERVER_URL`] when the env is unset).
pub fn get_insights_server_url() -> String {
    std::env::var("INSIGHTS_SERVER_URL").unwrap_or_else(|_| DEFAULT_INSIGHTS_SERVER_URL.to_string())
}

/// TCP address for `insights_server --bind` and for auto-start from the `insights` CLI.
pub fn get_insights_server_bind() -> Result<SocketAddr> {
    socket_addr_from_server_url(&get_insights_server_url())
}

/// Resolve `INSIGHTS_SERVER_URL` (or any same-shaped URL) to a [`SocketAddr`].
pub fn socket_addr_from_server_url(url: &str) -> Result<SocketAddr> {
    let url = Url::parse(url).map_err(|e| anyhow!("INSIGHTS_SERVER_URL: {e}"))?;
    if url.cannot_be_a_base() {
        return Err(anyhow!("INSIGHTS_SERVER_URL: invalid base URL"));
    }
    let default_port = || match url.scheme() {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    };
    let addrs: Vec<SocketAddr> = url
        .socket_addrs(default_port)
        .map_err(|e| anyhow!("INSIGHTS_SERVER_URL: {e}"))?;
    addrs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("INSIGHTS_SERVER_URL: no socket addresses for host"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_round_trips() {
        let a = socket_addr_from_server_url(DEFAULT_INSIGHTS_SERVER_URL).unwrap();
        assert_eq!(a, "127.0.0.1:2020".parse().unwrap());
    }

    #[test]
    fn explicit_port() {
        let a = socket_addr_from_server_url("http://127.0.0.1:4000").unwrap();
        assert_eq!(a.port(), 4000);
    }
}
