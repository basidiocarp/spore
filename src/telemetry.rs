//! Shared OpenTelemetry initialization and propagation helpers.
//!
//! This module is intentionally narrow. It gives ecosystem crates a feature-gated
//! way to initialize a tracer provider and round-trip W3C trace context without
//! forcing downstream repos to adopt full tracing instrumentation in the same step.

use crate::{Result, SporeError};
use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::trace::{TraceContextExt, TracerProvider as _};
use opentelemetry::{Context, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Standard OTLP endpoint environment variable.
pub const OTEL_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

static TELEMETRY_RUNTIME: OnceLock<TelemetryRuntime> = OnceLock::new();

#[derive(Debug)]
struct TelemetryRuntime {
    service_name: String,
    endpoint: String,
    #[allow(dead_code)]
    provider: SdkTracerProvider,
}

/// Result of calling [`init_tracer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryInit {
    pub enabled: bool,
    pub service_name: String,
    pub endpoint: Option<String>,
}

impl TelemetryInit {
    #[must_use]
    pub fn disabled(service_name: &str) -> Self {
        Self {
            enabled: false,
            service_name: service_name.to_string(),
            endpoint: None,
        }
    }

    #[must_use]
    pub fn enabled(service_name: &str, endpoint: &str) -> Self {
        Self {
            enabled: true,
            service_name: service_name.to_string(),
            endpoint: Some(endpoint.to_string()),
        }
    }
}

/// Serializable W3C trace context carrier.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceContextCarrier {
    pub traceparent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracestate: Option<String>,
}

impl TraceContextCarrier {
    /// Capture a valid trace context from an OpenTelemetry context.
    #[must_use]
    pub fn from_context(context: &Context) -> Option<Self> {
        if !context.span().span_context().is_valid() {
            return None;
        }

        let mut headers = BTreeMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(context, &mut HeaderInjector(&mut headers));
        });

        headers.get("traceparent").map(|traceparent| Self {
            traceparent: traceparent.clone(),
            tracestate: headers.get("tracestate").cloned(),
        })
    }

    /// Capture a valid trace context from the current context, if one exists.
    #[must_use]
    pub fn from_current() -> Option<Self> {
        Self::from_context(&Context::current())
    }

    /// Serialize into a stable header map for transport across process boundaries.
    #[must_use]
    pub fn into_headers(self) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::from([("traceparent".to_string(), self.traceparent)]);
        if let Some(tracestate) = self.tracestate {
            headers.insert("tracestate".to_string(), tracestate);
        }
        headers
    }

    /// Parse a carrier from a stable header map.
    #[must_use]
    pub fn from_headers(headers: &BTreeMap<String, String>) -> Option<Self> {
        headers.get("traceparent").map(|traceparent| Self {
            traceparent: traceparent.clone(),
            tracestate: headers.get("tracestate").cloned(),
        })
    }

    /// Extract an OpenTelemetry context from the serialized carrier.
    #[must_use]
    pub fn to_context(&self) -> Context {
        global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor {
                headers: &self.clone().into_headers(),
            })
        })
    }
}

/// Initialize an OTLP tracer provider when an endpoint is configured.
///
/// When `OTEL_EXPORTER_OTLP_ENDPOINT` is missing or empty, this is a no-op and
/// returns a disabled result so downstream tools can opt in safely.
/// Initialization is process-global. Once a provider is installed, later calls
/// return the existing configuration instead of reconfiguring telemetry.
///
/// # Errors
///
/// Returns an error when the configured OTLP exporter cannot be constructed.
pub fn init_tracer(service_name: &str) -> Result<TelemetryInit> {
    init_tracer_with_endpoint(service_name, configured_endpoint())
}

fn configured_endpoint() -> Option<String> {
    std::env::var(OTEL_ENDPOINT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn init_tracer_with_endpoint(
    service_name: &str,
    endpoint: Option<String>,
) -> Result<TelemetryInit> {
    let Some(endpoint) = endpoint else {
        return Ok(TelemetryInit::disabled(service_name));
    };

    if let Some(runtime) = TELEMETRY_RUNTIME.get() {
        return Ok(TelemetryInit::enabled(
            &runtime.service_name,
            &runtime.endpoint,
        ));
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint.clone())
        .build()
        .map_err(|error| SporeError::Logging(error.to_string()))?;

    let resource = Resource::builder_empty()
        .with_attributes([KeyValue::new("service.name", service_name.to_string())])
        .build();

    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(provider.clone());

    let _ = provider.tracer(service_name.to_string());

    let _ = TELEMETRY_RUNTIME.set(TelemetryRuntime {
        service_name: service_name.to_string(),
        endpoint: endpoint.clone(),
        provider,
    });

    Ok(TelemetryInit::enabled(service_name, &endpoint))
}

#[derive(Debug)]
struct HeaderInjector<'a>(&'a mut BTreeMap<String, String>);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

#[derive(Debug)]
struct HeaderExtractor<'a> {
    headers: &'a BTreeMap<String, String>,
}

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::Tracer as _;

    #[test]
    fn init_tracer_is_noop_without_endpoint() {
        let state = init_tracer_with_endpoint("spore-test", None).expect("no-op init succeeds");

        assert!(!state.enabled);
        assert_eq!(state.service_name, "spore-test");
        assert_eq!(state.endpoint, None);
    }

    #[test]
    fn init_tracer_is_enabled_when_endpoint_is_present() {
        let state = init_tracer_with_endpoint("spore-test", Some("http://127.0.0.1:4318".into()))
            .expect("otel init succeeds with configured endpoint");

        assert!(state.enabled);
        assert_eq!(state.service_name, "spore-test");
        assert_eq!(state.endpoint.as_deref(), Some("http://127.0.0.1:4318"));
    }

    #[test]
    fn trace_context_carrier_serializes_and_deserializes() {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("spore-test");
        let span = tracer.start("serialize-roundtrip");
        let context = Context::current_with_span(span);

        let carrier =
            TraceContextCarrier::from_context(&context).expect("valid span context is exported");
        let json = serde_json::to_string(&carrier).expect("carrier serializes");
        let parsed: TraceContextCarrier =
            serde_json::from_str(&json).expect("carrier deserializes");
        let restored = parsed.to_context();

        let original_binding = context.span();
        let original = original_binding.span_context();
        let restored_binding = restored.span();
        let restored_span = restored_binding.span_context();

        assert_eq!(carrier, parsed);
        assert!(carrier.traceparent.starts_with("00-"));
        assert_eq!(original.trace_id(), restored_span.trace_id());
        assert_eq!(original.span_id(), restored_span.span_id());
    }

    #[test]
    fn trace_context_carrier_from_headers_roundtrips() {
        let headers = BTreeMap::from([
            (
                "traceparent".to_string(),
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
            ),
            ("tracestate".to_string(), "vendor=value".to_string()),
        ]);

        let carrier = TraceContextCarrier::from_headers(&headers).expect("carrier parsed");
        let roundtrip = carrier.clone().into_headers();

        assert_eq!(roundtrip, headers);
        assert_eq!(carrier.tracestate.as_deref(), Some("vendor=value"));
    }
}
