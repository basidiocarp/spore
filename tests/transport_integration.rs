use spore::{LocalServiceClient, LocalServiceEndpoint, TransportError};

#[test]
fn test_parse_unix_socket_endpoint_from_json() {
    let json = r#"{
        "schema_version": "1.0",
        "transport": "unix-socket",
        "endpoint": "/tmp/hyphae-memory.sock",
        "capability_id": "memory.store.v1",
        "timeout_ms": 5000,
        "health_probe": {
            "method": "PING",
            "timeout_ms": 1000
        },
        "version": "1.4.2"
    }"#;

    let endpoint = LocalServiceEndpoint::from_json(json).expect("should parse");
    assert_eq!(endpoint.endpoint, "/tmp/hyphae-memory.sock");
    assert_eq!(endpoint.capability_id, Some("memory.store.v1".to_string()));
    assert_eq!(endpoint.timeout_ms, Some(5000));
    assert_eq!(endpoint.version, Some("1.4.2".to_string()));

    assert!(endpoint.health_probe.is_some());
    let probe = endpoint.health_probe.unwrap();
    assert_eq!(probe.method, "PING");
    assert_eq!(probe.timeout_ms, Some(1000));
}

#[test]
fn test_parse_tcp_endpoint_from_json() {
    let json = r#"{
        "schema_version": "1.0",
        "transport": "tcp",
        "endpoint": "127.0.0.1:8001",
        "capability_id": "code.intelligence.search.v1",
        "timeout_ms": 3000,
        "health_probe": {
            "method": "GET /health",
            "timeout_ms": 500
        },
        "version": "0.9.0"
    }"#;

    let endpoint = LocalServiceEndpoint::from_json(json).expect("should parse");
    assert_eq!(endpoint.endpoint, "127.0.0.1:8001");
    assert_eq!(
        endpoint.capability_id,
        Some("code.intelligence.search.v1".to_string())
    );
    assert_eq!(endpoint.timeout_ms, Some(3000));
    assert_eq!(endpoint.version, Some("0.9.0".to_string()));

    assert!(endpoint.health_probe.is_some());
    let probe = endpoint.health_probe.unwrap();
    assert_eq!(probe.method, "GET /health");
    assert_eq!(probe.timeout_ms, Some(500));
}

#[test]
#[cfg(unix)]
fn test_connect_to_missing_unix_socket_returns_connect_error() {
    let endpoint = LocalServiceEndpoint {
        schema_version: "1.0".to_string(),
        transport: spore::transport::Transport::UnixSocket,
        endpoint: "/tmp/nonexistent-socket-12345.sock".to_string(),
        capability_id: None,
        timeout_ms: Some(1000),
        health_probe: None,
        version: None,
    };

    let client = LocalServiceClient::new(endpoint);
    let result = client.call("test_method", serde_json::Value::Null);

    assert!(result.is_err(), "call to missing socket should fail");

    match result {
        Err(TransportError::Connect { endpoint, .. }) => {
            assert!(endpoint.contains("nonexistent-socket"));
        }
        other => {
            panic!("expected Connect error, got {other:?}");
        }
    }
}
