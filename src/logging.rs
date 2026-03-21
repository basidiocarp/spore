//! Shared tracing/logging initialization for ecosystem tools.
//!
//! Provides a consistent logging setup across hyphae, rhizome, and other
//! long-running ecosystem processes. Uses `tracing_subscriber` with env filter.

use tracing::Level;
use tracing_subscriber::EnvFilter;

// ─────────────────────────────────────────────────────────────────────────────
// Init Tracing
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize tracing with a default level and env filter override.
///
/// The environment variable `RUST_LOG` (or `<APP_NAME>_LOG` if set) overrides
/// the default level. Output goes to stderr to avoid interfering with
/// stdio-based MCP communication.
///
/// Call once at the start of `main()`.
///
/// # Examples
///
/// ```
/// // In main.rs:
/// spore::logging::init(tracing::Level::WARN);
/// ```
pub fn init(default_level: Level) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level.as_str()));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Initialize tracing with a custom env variable name.
///
/// Checks `$<env_var>` first, then `$RUST_LOG`, then falls back to `default_level`.
///
/// # Examples
///
/// ```ignore
/// // Reads HYPHAE_LOG, then RUST_LOG, then defaults to WARN
/// spore::logging::init_with_env("HYPHAE_LOG", tracing::Level::WARN);
/// ```
pub fn init_with_env(env_var: &str, default_level: Level) {
    let filter = if let Ok(val) = std::env::var(env_var) {
        EnvFilter::new(val)
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level.as_str()))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
