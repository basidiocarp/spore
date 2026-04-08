//! Shared tracing/logging initialization for ecosystem tools.
//!
//! Provides a consistent logging setup across hyphae, rhizome, and other
//! long-running ecosystem processes. Uses `tracing_subscriber` with env filter.

use crate::{Result, SporeError};
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

/// Output format for tracing events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Compact,
    Pretty,
    Json,
}

/// Output target for tracing events.
///
/// `Stderr` is the safe default for MCP-aware tools because it does not
/// interfere with stdout-based transport framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogOutput {
    Stderr,
    Stdout,
}

/// Span event verbosity.
///
/// Use span events when you need better failure localization for request,
/// subprocess, session, or workflow boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanEvents {
    Off,
    Lifecycle,
    Full,
}

impl SpanEvents {
    fn into_fmt_span(self) -> FmtSpan {
        match self {
            Self::Off => FmtSpan::NONE,
            Self::Lifecycle => FmtSpan::NEW | FmtSpan::CLOSE,
            Self::Full => FmtSpan::FULL,
        }
    }
}

/// Shared logging configuration for ecosystem binaries.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub default_level: Level,
    pub app_name: Option<String>,
    pub env_var: Option<String>,
    pub format: LogFormat,
    pub output: LogOutput,
    pub span_events: SpanEvents,
    pub include_target: bool,
}

impl LoggingConfig {
    #[must_use]
    pub fn new(default_level: Level) -> Self {
        Self {
            default_level,
            app_name: None,
            env_var: None,
            format: LogFormat::Compact,
            output: LogOutput::Stderr,
            span_events: SpanEvents::Off,
            include_target: true,
        }
    }

    #[must_use]
    pub fn for_app(app_name: impl Into<String>, default_level: Level) -> Self {
        Self::new(default_level).with_app_name(app_name)
    }

    #[must_use]
    pub fn with_app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    #[must_use]
    pub fn with_env_var(mut self, env_var: impl Into<String>) -> Self {
        self.env_var = Some(env_var.into());
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    #[must_use]
    pub fn with_output(mut self, output: LogOutput) -> Self {
        self.output = output;
        self
    }

    #[must_use]
    pub fn with_span_events(mut self, span_events: SpanEvents) -> Self {
        self.span_events = span_events;
        self
    }

    #[must_use]
    pub fn with_target(mut self, include_target: bool) -> Self {
        self.include_target = include_target;
        self
    }

    #[must_use]
    pub fn env_var_name(&self) -> Option<String> {
        self.env_var
            .clone()
            .or_else(|| self.app_name.as_deref().map(app_log_env_var))
    }
}

/// Derive the conventional `<APP>_LOG` environment variable name.
#[must_use]
pub fn app_log_env_var(app_name: &str) -> String {
    let normalized = app_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{normalized}_LOG")
}

// ─────────────────────────────────────────────────────────────────────────────
// Init Tracing
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize tracing with a default level and env filter override.
///
/// Checks `RUST_LOG` first, then falls back to `default_level`. Output goes to
/// stderr to avoid interfering with stdio-based MCP communication.
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
    try_init(default_level).expect("failed to initialize tracing subscriber");
}

/// Try to initialize tracing with a default level and env filter override.
///
/// Use this in libraries, tests, and repeated startup paths where a double init
/// should return an error instead of panicking the process.
pub fn try_init(default_level: Level) -> Result<()> {
    try_init_with_config(LoggingConfig::new(default_level))
}

/// Initialize tracing with an app-aware env var.
///
/// `app_name` is converted to `<APP>_LOG`, with non-alphanumeric characters
/// replaced by underscores. The derived env var is checked before `RUST_LOG`.
pub fn init_app(app_name: impl Into<String>, default_level: Level) {
    try_init_app(app_name, default_level).expect("failed to initialize tracing subscriber");
}

/// Try to initialize tracing with an app-aware env var.
pub fn try_init_app(app_name: impl Into<String>, default_level: Level) -> Result<()> {
    try_init_with_config(LoggingConfig::for_app(app_name, default_level))
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
    try_init_with_env(env_var, default_level).expect("failed to initialize tracing subscriber");
}

/// Try to initialize tracing with a custom env variable name.
pub fn try_init_with_env(env_var: &str, default_level: Level) -> Result<()> {
    try_init_with_config(LoggingConfig::new(default_level).with_env_var(env_var))
}

/// Initialize tracing with a fully explicit config surface.
pub fn init_with_config(config: LoggingConfig) {
    try_init_with_config(config).expect("failed to initialize tracing subscriber");
}

/// Try to initialize tracing with a fully explicit config surface.
pub fn try_init_with_config(config: LoggingConfig) -> Result<()> {
    let filter = resolve_filter(&config);
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(config.span_events.into_fmt_span())
        .with_target(config.include_target);

    let init_result = match (config.output, config.format) {
        (LogOutput::Stderr, LogFormat::Compact) => {
            builder.compact().with_writer(std::io::stderr).try_init()
        }
        (LogOutput::Stderr, LogFormat::Pretty) => {
            builder.pretty().with_writer(std::io::stderr).try_init()
        }
        (LogOutput::Stderr, LogFormat::Json) => {
            builder.json().with_writer(std::io::stderr).try_init()
        }
        (LogOutput::Stdout, LogFormat::Compact) => {
            builder.compact().with_writer(std::io::stdout).try_init()
        }
        (LogOutput::Stdout, LogFormat::Pretty) => {
            builder.pretty().with_writer(std::io::stdout).try_init()
        }
        (LogOutput::Stdout, LogFormat::Json) => {
            builder.json().with_writer(std::io::stdout).try_init()
        }
    };

    init_result.map_err(|error| SporeError::Logging(error.to_string()))
}

fn resolve_filter(config: &LoggingConfig) -> EnvFilter {
    let directive = resolve_filter_directive(
        config.env_var_name().as_deref(),
        std::env::var("RUST_LOG").ok().as_deref(),
        config.default_level,
    );
    EnvFilter::new(directive)
}

fn resolve_filter_directive(
    app_env_value: Option<&str>,
    rust_log_value: Option<&str>,
    default_level: Level,
) -> String {
    app_env_value
        .filter(|value| !value.trim().is_empty())
        .or(rust_log_value.filter(|value| !value.trim().is_empty()))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_level.to_string().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_log_env_var_normalizes_names() {
        assert_eq!(app_log_env_var("hyphae"), "HYPHAE_LOG");
        assert_eq!(app_log_env_var("rhizome-mcp"), "RHIZOME_MCP_LOG");
        assert_eq!(app_log_env_var("volva bridge"), "VOLVA_BRIDGE_LOG");
    }

    #[test]
    fn logging_config_derives_env_var_from_app_name() {
        let config = LoggingConfig::for_app("canopy", Level::WARN);
        assert_eq!(config.env_var_name().as_deref(), Some("CANOPY_LOG"));
    }

    #[test]
    fn explicit_env_var_overrides_app_name() {
        let config = LoggingConfig::for_app("hyphae", Level::WARN).with_env_var("CUSTOM_LOG");
        assert_eq!(config.env_var_name().as_deref(), Some("CUSTOM_LOG"));
    }

    #[test]
    fn filter_directive_prefers_app_env_then_rust_log_then_default() {
        assert_eq!(
            resolve_filter_directive(Some("hyphae=debug"), Some("info"), Level::WARN),
            "hyphae=debug"
        );
        assert_eq!(
            resolve_filter_directive(None, Some("rhizome=trace"), Level::WARN),
            "rhizome=trace"
        );
        assert_eq!(resolve_filter_directive(None, None, Level::ERROR), "error");
    }

    #[test]
    fn span_events_map_to_fmt_span() {
        assert_eq!(SpanEvents::Off.into_fmt_span(), FmtSpan::NONE);
        assert_eq!(
            SpanEvents::Lifecycle.into_fmt_span(),
            FmtSpan::NEW | FmtSpan::CLOSE
        );
        assert_eq!(SpanEvents::Full.into_fmt_span(), FmtSpan::FULL);
    }
}
