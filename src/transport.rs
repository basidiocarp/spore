//! Local service transport client for communicating with local service endpoints.
//!
//! Supports three transport types: unix-socket, tcp, and http.
//! Parses endpoint descriptors from JSON (as defined in septa) and provides
//! a typed client for making JSON-RPC 2.0 calls.

use crate::jsonrpc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Transport type for a local service endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    #[serde(rename = "unix-socket")]
    UnixSocket,
    Tcp,
    Http,
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transport::UnixSocket => write!(f, "unix-socket"),
            Transport::Tcp => write!(f, "tcp"),
            Transport::Http => write!(f, "http"),
        }
    }
}

/// Health probe configuration for a local service endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthProbe {
    pub method: String,
    pub timeout_ms: Option<u64>,
}

fn schema_v1_default() -> String {
    "1.0".to_string()
}

/// Descriptor for a local service endpoint.
///
/// Parsed from JSON using the septa `local-service-endpoint-v1` schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalServiceEndpoint {
    /// Schema version from the septa contract. Must be `"1.0"` for this struct.
    #[serde(default = "schema_v1_default")]
    pub schema_version: String,
    pub transport: Transport,
    pub endpoint: String,
    pub capability_id: Option<String>,
    pub timeout_ms: Option<u64>,
    pub health_probe: Option<HealthProbe>,
    pub version: Option<String>,
}

impl LocalServiceEndpoint {
    /// Parse an endpoint descriptor from JSON string.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::UnsupportedVersion` if the document carries a
    /// `schema_version` other than `"1.0"`. Returns `TransportError::Parse` if
    /// the JSON is malformed.
    pub fn from_json(json: &str) -> Result<Self, TransportError> {
        let endpoint: Self = serde_json::from_str(json).map_err(TransportError::Parse)?;
        if endpoint.schema_version != "1.0" {
            return Err(TransportError::UnsupportedVersion {
                version: endpoint.schema_version.clone(),
            });
        }
        Ok(endpoint)
    }
}

/// Error type for local service transport operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    #[error("connection failed to {endpoint}: {source}")]
    Connect {
        endpoint: String,
        source: std::io::Error,
    },

    #[error("I/O error on {endpoint}: {source}")]
    Io {
        endpoint: String,
        source: std::io::Error,
    },

    #[error("timeout after {timeout_ms}ms calling {endpoint}")]
    Timeout { endpoint: String, timeout_ms: u64 },

    #[error("RPC error from {endpoint}: code {code}: {message}")]
    Rpc {
        endpoint: String,
        code: i64,
        message: String,
    },

    #[error("protocol error on {endpoint}: {detail}")]
    Protocol { endpoint: String, detail: String },

    #[error("transport {transport} not supported on this platform")]
    NotSupported { transport: String },

    #[error("unsupported schema version {version:?}; expected \"1.0\"")]
    UnsupportedVersion { version: String },

    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Client for communicating with a local service endpoint via JSON-RPC 2.0.
pub struct LocalServiceClient {
    endpoint: LocalServiceEndpoint,
}

impl LocalServiceClient {
    /// Create a new client for the given endpoint descriptor.
    #[must_use]
    pub fn new(endpoint: LocalServiceEndpoint) -> Self {
        Self { endpoint }
    }

    /// Parse an endpoint descriptor from JSON and create a client.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid or does not conform to the expected schema.
    pub fn from_json(json: &str) -> Result<Self, TransportError> {
        let endpoint = LocalServiceEndpoint::from_json(json)?;
        Ok(Self::new(endpoint))
    }

    /// Send a JSON-RPC 2.0 request and return the result Value.
    ///
    /// Uses the endpoint's configured `timeout_ms` if present, otherwise defaults to 10 seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails, times out, or the server returns a JSON-RPC error.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, TransportError> {
        let timeout_ms = self.endpoint.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        self.call_with_timeout(method, params, Duration::from_millis(timeout_ms))
    }

    /// Send a JSON-RPC 2.0 request with an explicit timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails, times out, or the server returns a JSON-RPC error.
    pub fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, TransportError> {
        match self.endpoint.transport {
            #[cfg(unix)]
            Transport::UnixSocket => self.call_unix_socket(method, params, timeout),
            #[cfg(not(unix))]
            Transport::UnixSocket => Err(TransportError::NotSupported {
                transport: "unix-socket".to_string(),
            }),
            Transport::Tcp => self.call_tcp(method, params, timeout),
            #[cfg(feature = "http")]
            Transport::Http => self.call_http(method, params, timeout),
            #[cfg(not(feature = "http"))]
            Transport::Http => Err(TransportError::NotSupported {
                transport: "http".to_string(),
            }),
        }
    }

    /// Probe the health endpoint if configured.
    ///
    /// Returns Ok(true) if the probe succeeds (healthy), Ok(false) if the probe
    /// returns a degraded response. Returns Err if the probe fails to connect or times out.
    ///
    /// # Errors
    ///
    /// Returns an error if the health probe is not configured, the connection fails,
    /// or the operation times out.
    pub fn probe_health(&self) -> Result<bool, TransportError> {
        let probe =
            self.endpoint
                .health_probe
                .as_ref()
                .ok_or_else(|| TransportError::Protocol {
                    endpoint: self.endpoint.endpoint.clone(),
                    detail: "no health probe configured".to_string(),
                })?;

        let method = probe.method.clone();
        let timeout_ms = probe
            .timeout_ms
            .or(self.endpoint.timeout_ms)
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let timeout = Duration::from_millis(timeout_ms);

        match self.call_with_timeout(&method, Value::Null, timeout) {
            Ok(_) => Ok(true),
            Err(TransportError::Timeout { .. }) => Err(TransportError::Timeout {
                endpoint: self.endpoint.endpoint.clone(),
                timeout_ms,
            }),
            Err(e) => Err(e),
        }
    }

    #[cfg(unix)]
    fn call_unix_socket(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, TransportError> {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;

        let stream =
            UnixStream::connect(&self.endpoint.endpoint).map_err(|e| TransportError::Connect {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;

        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;

        let mut writer = stream.try_clone().map_err(|e| TransportError::Io {
            endpoint: self.endpoint.endpoint.clone(),
            source: e,
        })?;

        // Send request as newline-delimited JSON
        let request = jsonrpc::Request::new(method, params);
        let json_str = serde_json::to_string(&request).map_err(|_| TransportError::Protocol {
            endpoint: self.endpoint.endpoint.clone(),
            detail: "failed to serialize request".to_string(),
        })?;

        writer
            .write_all(json_str.as_bytes())
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;
        writer.write_all(b"\n").map_err(|e| TransportError::Io {
            endpoint: self.endpoint.endpoint.clone(),
            source: e,
        })?;

        // Read response as newline-delimited JSON
        let reader = BufReader::new(&stream);
        let mut lines = reader.lines();
        loop {
            match lines.next() {
                Some(Ok(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || !trimmed.starts_with('{') {
                        continue;
                    }
                    let response: jsonrpc::Response =
                        serde_json::from_str(trimmed).map_err(|_| TransportError::Protocol {
                            endpoint: self.endpoint.endpoint.clone(),
                            detail: "failed to parse response".to_string(),
                        })?;

                    if let Some(error) = response.error {
                        return Err(TransportError::Rpc {
                            endpoint: self.endpoint.endpoint.clone(),
                            code: error.code,
                            message: error.message,
                        });
                    }

                    return response.result.ok_or_else(|| TransportError::Protocol {
                        endpoint: self.endpoint.endpoint.clone(),
                        detail: "empty result in response".to_string(),
                    });
                }
                Some(Err(e)) => {
                    return Err(
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock
                        {
                            TransportError::Timeout {
                                endpoint: self.endpoint.endpoint.clone(),
                                timeout_ms: u64::try_from(timeout.as_millis())
                                    .unwrap_or(u64::MAX),
                            }
                        } else {
                            TransportError::Io {
                                endpoint: self.endpoint.endpoint.clone(),
                                source: e,
                            }
                        },
                    );
                }
                None => {
                    return Err(TransportError::Protocol {
                        endpoint: self.endpoint.endpoint.clone(),
                        detail: "EOF while reading response".to_string(),
                    });
                }
            }
        }
    }

    fn call_tcp(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, TransportError> {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpStream;

        let stream =
            TcpStream::connect(&self.endpoint.endpoint).map_err(|e| TransportError::Connect {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;

        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;

        let mut writer = stream.try_clone().map_err(|e| TransportError::Io {
            endpoint: self.endpoint.endpoint.clone(),
            source: e,
        })?;

        // Send request as newline-delimited JSON
        let request = jsonrpc::Request::new(method, params);
        let json_str = serde_json::to_string(&request).map_err(|_| TransportError::Protocol {
            endpoint: self.endpoint.endpoint.clone(),
            detail: "failed to serialize request".to_string(),
        })?;

        writer
            .write_all(json_str.as_bytes())
            .map_err(|e| TransportError::Io {
                endpoint: self.endpoint.endpoint.clone(),
                source: e,
            })?;
        writer.write_all(b"\n").map_err(|e| TransportError::Io {
            endpoint: self.endpoint.endpoint.clone(),
            source: e,
        })?;

        // Read response as newline-delimited JSON
        let reader = BufReader::new(&stream);
        let mut lines = reader.lines();
        loop {
            match lines.next() {
                Some(Ok(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || !trimmed.starts_with('{') {
                        continue;
                    }
                    let response: jsonrpc::Response =
                        serde_json::from_str(trimmed).map_err(|_| TransportError::Protocol {
                            endpoint: self.endpoint.endpoint.clone(),
                            detail: "failed to parse response".to_string(),
                        })?;

                    if let Some(error) = response.error {
                        return Err(TransportError::Rpc {
                            endpoint: self.endpoint.endpoint.clone(),
                            code: error.code,
                            message: error.message,
                        });
                    }

                    return response.result.ok_or_else(|| TransportError::Protocol {
                        endpoint: self.endpoint.endpoint.clone(),
                        detail: "empty result in response".to_string(),
                    });
                }
                Some(Err(e)) => {
                    return Err(
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock
                        {
                            TransportError::Timeout {
                                endpoint: self.endpoint.endpoint.clone(),
                                timeout_ms: u64::try_from(timeout.as_millis())
                                    .unwrap_or(u64::MAX),
                            }
                        } else {
                            TransportError::Io {
                                endpoint: self.endpoint.endpoint.clone(),
                                source: e,
                            }
                        },
                    );
                }
                None => {
                    return Err(TransportError::Protocol {
                        endpoint: self.endpoint.endpoint.clone(),
                        detail: "EOF while reading response".to_string(),
                    });
                }
            }
        }
    }

    #[cfg(feature = "http")]
    #[allow(clippy::unused_self)]
    fn call_http(
        &self,
        _method: &str,
        _params: Value,
        _timeout: Duration,
    ) -> Result<Value, TransportError> {
        // HTTP transport is not yet implemented. For now, return NotSupported.
        Err(TransportError::NotSupported {
            transport: "http".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_display() {
        assert_eq!(Transport::UnixSocket.to_string(), "unix-socket");
        assert_eq!(Transport::Tcp.to_string(), "tcp");
        assert_eq!(Transport::Http.to_string(), "http");
    }

    #[test]
    fn test_endpoint_from_json() {
        let json = r#"{
            "schema_version": "1.0",
            "transport": "unix-socket",
            "endpoint": "/tmp/test.sock",
            "capability_id": "test.v1",
            "timeout_ms": 5000,
            "version": "1.0"
        }"#;

        let endpoint = LocalServiceEndpoint::from_json(json).unwrap();
        assert_eq!(endpoint.transport, Transport::UnixSocket);
        assert_eq!(endpoint.endpoint, "/tmp/test.sock");
        assert_eq!(endpoint.capability_id, Some("test.v1".to_string()));
        assert_eq!(endpoint.timeout_ms, Some(5000));
        assert_eq!(endpoint.version, Some("1.0".to_string()));
    }

    #[test]
    fn test_endpoint_from_json_minimal() {
        let json = r#"{
            "transport": "tcp",
            "endpoint": "127.0.0.1:8000"
        }"#;

        let endpoint = LocalServiceEndpoint::from_json(json).unwrap();
        assert_eq!(endpoint.transport, Transport::Tcp);
        assert_eq!(endpoint.endpoint, "127.0.0.1:8000");
        assert!(endpoint.capability_id.is_none());
        assert!(endpoint.timeout_ms.is_none());
    }

    #[test]
    fn test_health_probe_parse() {
        let json = r#"{
            "method": "PING",
            "timeout_ms": 1000
        }"#;

        let probe: HealthProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.method, "PING");
        assert_eq!(probe.timeout_ms, Some(1000));
    }

    #[test]
    fn test_client_new() {
        let endpoint = LocalServiceEndpoint {
            schema_version: "1.0".to_string(),
            transport: Transport::Tcp,
            endpoint: "127.0.0.1:8000".to_string(),
            capability_id: None,
            timeout_ms: Some(5000),
            health_probe: None,
            version: None,
        };

        let client = LocalServiceClient::new(endpoint.clone());
        assert_eq!(client.endpoint.transport, Transport::Tcp);
    }

    #[test]
    fn test_client_from_json() {
        let json = r#"{
            "transport": "tcp",
            "endpoint": "127.0.0.1:9000"
        }"#;

        let client = LocalServiceClient::from_json(json).unwrap();
        assert_eq!(client.endpoint.transport, Transport::Tcp);
        assert_eq!(client.endpoint.endpoint, "127.0.0.1:9000");
    }

    #[test]
    fn test_unsupported_schema_version_rejected() {
        let json = r#"{
            "schema_version": "2.0",
            "transport": "tcp",
            "endpoint": "127.0.0.1:8000"
        }"#;
        let err = LocalServiceEndpoint::from_json(json).unwrap_err();
        assert!(matches!(err, TransportError::UnsupportedVersion { .. }));
    }

    #[test]
    fn test_transport_error_display() {
        let err = TransportError::Connect {
            endpoint: "127.0.0.1:8000".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        };
        assert!(err.to_string().contains("connection failed"));
        assert!(err.to_string().contains("127.0.0.1:8000"));
    }
}
