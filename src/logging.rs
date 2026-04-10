//! Shared tracing/logging initialization for ecosystem tools.
//!
//! Provides a consistent logging setup across hyphae, rhizome, and other
//! long-running ecosystem processes. Uses `tracing_subscriber` with env filter.

use crate::{Result, SporeError};
use std::fmt;
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

pub const SERVICE_FIELD: &str = "service";
pub const TOOL_FIELD: &str = "tool";
pub const REQUEST_ID_FIELD: &str = "request_id";
pub const SESSION_ID_FIELD: &str = "session_id";
pub const WORKSPACE_ROOT_FIELD: &str = "workspace_root";
pub const SPAN_KIND_FIELD: &str = "span_kind";
pub const OPERATION_FIELD: &str = "operation";
pub const COMMAND_FIELD: &str = "command";

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
    pub session_id: Option<String>,
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
            session_id: None,
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
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
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

    #[must_use]
    pub fn span_context(&self) -> SpanContext {
        let session_id = self.session_id.clone().or_else(crate::claude_session_id);

        SpanContext {
            service: self.app_name.clone(),
            tool: None,
            request_id: None,
            session_id,
            workspace_root: None,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Root,
    Request,
    Tool,
    Workflow,
    Subprocess,
}

impl SpanKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Request => "request",
            Self::Tool => "tool",
            Self::Workflow => "workflow",
            Self::Subprocess => "subprocess",
        }
    }
}

/// Standardized tracing context for ecosystem workflow spans.
///
/// Build this once per request or workflow and pass it to the helper span
/// constructors in this module so field names stay consistent across repos.
#[derive(Debug, Clone, Default)]
pub struct SpanContext {
    pub service: Option<String>,
    pub tool: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub workspace_root: Option<String>,
}

impl SpanContext {
    #[must_use]
    pub fn for_app(app_name: impl Into<String>) -> Self {
        Self {
            service: Some(app_name.into()),
            tool: None,
            request_id: None,
            session_id: crate::claude_session_id(),
            workspace_root: None,
        }
    }

    #[must_use]
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    #[must_use]
    pub fn with_workspace_root(mut self, workspace_root: impl Into<String>) -> Self {
        self.workspace_root = Some(workspace_root.into());
        self
    }
}

struct OptionalField<'a>(Option<&'a str>);

impl fmt::Display for OptionalField<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) if !value.trim().is_empty() => f.write_str(value),
            _ => f.write_str("-"),
        }
    }
}

fn optional_field(value: Option<&str>) -> tracing::field::DisplayValue<OptionalField<'_>> {
    tracing::field::display(OptionalField(value))
}

/// Root span for injecting service and session metadata into a whole runtime.
///
/// Enter this once near startup if you want all nested spans and events to
/// inherit a consistent service/session context.
#[must_use]
pub fn root_span(context: &SpanContext) -> tracing::Span {
    tracing::info_span!(
        "runtime",
        span_kind = SpanKind::Root.as_str(),
        service = optional_field(context.service.as_deref()),
        tool = optional_field(context.tool.as_deref()),
        request_id = optional_field(context.request_id.as_deref()),
        session_id = optional_field(context.session_id.as_deref()),
        workspace_root = optional_field(context.workspace_root.as_deref()),
    )
}

/// Standard request span using the ecosystem field convention.
#[must_use]
pub fn request_span(operation: &str, context: &SpanContext) -> tracing::Span {
    tracing::info_span!(
        "request",
        span_kind = SpanKind::Request.as_str(),
        operation = operation,
        service = optional_field(context.service.as_deref()),
        tool = optional_field(context.tool.as_deref()),
        request_id = optional_field(context.request_id.as_deref()),
        session_id = optional_field(context.session_id.as_deref()),
        workspace_root = optional_field(context.workspace_root.as_deref()),
    )
}

/// Standard tool span using the ecosystem field convention.
#[must_use]
pub fn tool_span(operation: &str, context: &SpanContext) -> tracing::Span {
    tracing::info_span!(
        "tool",
        span_kind = SpanKind::Tool.as_str(),
        operation = operation,
        service = optional_field(context.service.as_deref()),
        tool = optional_field(context.tool.as_deref()),
        request_id = optional_field(context.request_id.as_deref()),
        session_id = optional_field(context.session_id.as_deref()),
        workspace_root = optional_field(context.workspace_root.as_deref()),
    )
}

/// Standard workflow span using the ecosystem field convention.
#[must_use]
pub fn workflow_span(operation: &str, context: &SpanContext) -> tracing::Span {
    tracing::info_span!(
        "workflow",
        span_kind = SpanKind::Workflow.as_str(),
        operation = operation,
        service = optional_field(context.service.as_deref()),
        tool = optional_field(context.tool.as_deref()),
        request_id = optional_field(context.request_id.as_deref()),
        session_id = optional_field(context.session_id.as_deref()),
        workspace_root = optional_field(context.workspace_root.as_deref()),
    )
}

/// Standard subprocess span using the ecosystem field convention.
#[must_use]
pub fn subprocess_span(command: &str, context: &SpanContext) -> tracing::Span {
    tracing::info_span!(
        "subprocess",
        span_kind = SpanKind::Subprocess.as_str(),
        command = command,
        service = optional_field(context.service.as_deref()),
        tool = optional_field(context.tool.as_deref()),
        request_id = optional_field(context.request_id.as_deref()),
        session_id = optional_field(context.session_id.as_deref()),
        workspace_root = optional_field(context.workspace_root.as_deref()),
    )
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

    init_result.map_err(
        |error: Box<dyn std::error::Error + Send + Sync + 'static>| {
            SporeError::Logging(error.to_string())
        },
    )
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
    use tracing::subscriber;
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    fn with_info_subscriber<T>(f: impl FnOnce() -> T) -> T {
        let subscriber = tracing_subscriber::registry().with(LevelFilter::INFO);
        subscriber::with_default(subscriber, f)
    }

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

    #[test]
    fn logging_config_builds_standard_span_context() {
        let context = LoggingConfig::for_app("hyphae", Level::WARN)
            .with_session_id("session-123")
            .span_context();

        assert_eq!(context.service.as_deref(), Some("hyphae"));
        assert_eq!(context.session_id.as_deref(), Some("session-123"));
        assert_eq!(context.tool.as_deref(), None);
        assert_eq!(context.request_id.as_deref(), None);
        assert_eq!(context.workspace_root.as_deref(), None);
    }

    #[test]
    fn root_span_uses_standard_metadata_fields() {
        with_info_subscriber(|| {
            let span = root_span(
                &SpanContext::for_app("canopy")
                    .with_session_id("session-123")
                    .with_workspace_root("/tmp/project"),
            );

            assert!(span.has_field(SERVICE_FIELD));
            assert!(span.has_field(SESSION_ID_FIELD));
            assert!(span.has_field(WORKSPACE_ROOT_FIELD));
            assert!(span.has_field(SPAN_KIND_FIELD));
        });
    }

    #[test]
    fn request_and_tool_spans_use_standard_metadata_fields() {
        with_info_subscriber(|| {
            let context = SpanContext::for_app("rhizome")
                .with_tool("get_diagnostics")
                .with_request_id("req-7")
                .with_session_id("session-123")
                .with_workspace_root("/tmp/rhizome");

            let request = request_span("mcp_request", &context);
            assert!(request.has_field(SERVICE_FIELD));
            assert!(request.has_field(TOOL_FIELD));
            assert!(request.has_field(REQUEST_ID_FIELD));
            assert!(request.has_field(SESSION_ID_FIELD));
            assert!(request.has_field(WORKSPACE_ROOT_FIELD));
            assert!(request.has_field(OPERATION_FIELD));
            assert!(request.has_field(SPAN_KIND_FIELD));

            let tool = tool_span("get_diagnostics", &context);
            assert!(tool.has_field(SERVICE_FIELD));
            assert!(tool.has_field(TOOL_FIELD));
            assert!(tool.has_field(REQUEST_ID_FIELD));
            assert!(tool.has_field(SESSION_ID_FIELD));
            assert!(tool.has_field(WORKSPACE_ROOT_FIELD));
            assert!(tool.has_field(OPERATION_FIELD));
            assert!(tool.has_field(SPAN_KIND_FIELD));
        });
    }

    #[test]
    fn workflow_and_subprocess_spans_use_standard_metadata_fields() {
        with_info_subscriber(|| {
            let context = SpanContext::for_app("mycelium")
                .with_request_id("req-8")
                .with_session_id("session-456");

            let workflow = workflow_span("rewrite_pipeline", &context);
            assert!(workflow.has_field(SERVICE_FIELD));
            assert!(workflow.has_field(REQUEST_ID_FIELD));
            assert!(workflow.has_field(SESSION_ID_FIELD));
            assert!(workflow.has_field(OPERATION_FIELD));
            assert!(workflow.has_field(SPAN_KIND_FIELD));

            let subprocess = subprocess_span("cargo test", &context);
            assert!(subprocess.has_field(SERVICE_FIELD));
            assert!(subprocess.has_field(REQUEST_ID_FIELD));
            assert!(subprocess.has_field(SESSION_ID_FIELD));
            assert!(subprocess.has_field(COMMAND_FIELD));
            assert!(subprocess.has_field(SPAN_KIND_FIELD));
        });
    }
}
