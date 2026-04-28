//! Capability registry discovery and endpoint resolution.
//!
//! Implements the consumer side of the `capability-registry-v1` and
//! `capability-runtime-lease-v1` Septa contracts. Stipe is the writer of the
//! installed registry; running tools are the writers of runtime leases. This
//! module provides read-only primitives for locating, parsing, and resolving
//! those files.
//!
//! # Resolution order
//!
//! [`resolve_capability`] applies lease-first, registry-fallback logic:
//!
//! 1. Load all lease files from the lease directory.
//! 2. Return the first non-stale lease that matches the requested capability id.
//! 3. If no live lease exists, load the installed registry.
//! 4. Return the first registry entry whose `capability_ids` contains a match.
//! 5. Return `None` when neither source has the capability.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Result, SporeError};

// ─────────────────────────────────────────────────────────────────────────────
// Shared enums
// ─────────────────────────────────────────────────────────────────────────────

/// Transport kind used to call a capability endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TransportKind {
    /// MCP over stdin/stdout.
    Stdio,
    /// Unix domain socket.
    UnixSocket,
    /// TCP endpoint.
    Tcp,
    /// Subprocess invocation via CLI.
    Cli,
}

// ─────────────────────────────────────────────────────────────────────────────
// capability-registry-v1 types
// ─────────────────────────────────────────────────────────────────────────────

/// Who manages a registry entry installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum CapabilityManager {
    /// Managed by Stipe install/update/uninstall.
    Stipe,
    /// User-installed binary outside Stipe.
    Manual,
    /// Tool manages its own installation.
    #[serde(rename = "self")]
    SelfManaged,
}

/// Last known health state for a registry entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum RegistryHealthStatus {
    Ok,
    Degraded,
    Missing,
}

/// Health hint stored in the installed registry entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryHealthHint {
    pub status: RegistryHealthStatus,
    pub message: Option<String>,
}

/// One entry in the installed capability registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Ecosystem tool name, e.g. `"hyphae"`.
    pub tool: String,
    /// Installed version string.
    pub version: String,
    /// Who installed this tool.
    pub manager: CapabilityManager,
    /// Capability ids this tool satisfies.
    pub capability_ids: Vec<String>,
    /// Related Septa contract ids.
    pub contract_ids: Vec<String>,
    /// Preferred transport kind for callers.
    pub transport: TransportKind,
    /// Absolute path to the binary, when applicable.
    pub binary_path: Option<String>,
    /// Health state at registration time.
    pub health: Option<RegistryHealthHint>,
}

/// Parsed `capability-registry-v1` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    pub schema_version: String,
    pub written_at_unix: u64,
    pub entries: Vec<RegistryEntry>,
}

impl CapabilityRegistry {
    /// Load and parse the registry from a file path.
    ///
    /// Returns `Ok(None)` when the file does not exist. Returns `Err` on I/O
    /// errors other than "not found", or when the file is not valid JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SporeError::Json`] on parse failure or [`SporeError::Other`]
    /// on unexpected I/O errors.
    pub fn load_from(path: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let registry = serde_json::from_str(&content)?;
                Ok(Some(registry))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SporeError::Other(format!(
                "failed to read capability registry at {}: {e}",
                path.display()
            ))),
        }
    }

    /// Load from the default ecosystem registry path.
    ///
    /// Delegates to [`crate::paths::capability_registry_path`] and
    /// [`Self::load_from`].
    ///
    /// # Errors
    ///
    /// Returns [`SporeError::Json`] on parse failure or [`SporeError::Other`]
    /// on unexpected I/O errors.
    pub fn load() -> Result<Option<Self>> {
        Self::load_from(&crate::paths::capability_registry_path())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// capability-runtime-lease-v1 types
// ─────────────────────────────────────────────────────────────────────────────

/// Self-reported health in a runtime lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LeaseHealthStatus {
    Ok,
    Degraded,
}

/// Health hint in a runtime lease.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseHealthHint {
    pub status: LeaseHealthStatus,
    pub message: Option<String>,
}

/// Parsed `capability-runtime-lease-v1` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLease {
    pub schema_version: String,
    /// Ecosystem tool name.
    pub tool: String,
    /// The capability id this lease satisfies.
    pub capability_id: String,
    /// Active transport kind.
    pub transport: TransportKind,
    /// Process id of the serving process.
    pub pid: u32,
    /// Unix timestamp when this lease was written.
    pub leased_at_unix: u64,
    /// Unix timestamp when this lease expires, if bounded.
    pub expires_at_unix: Option<u64>,
    /// For socket transports: socket path or `host:port`.
    pub endpoint: Option<String>,
    /// For CLI/stdio transports: binary path or command.
    pub command: Option<String>,
    /// Running version, if available.
    pub version: Option<String>,
    /// Self-reported health at lease time.
    pub health: Option<LeaseHealthHint>,
}

impl RuntimeLease {
    /// Load all valid lease files from a directory.
    ///
    /// Silently skips files that cannot be read or parsed — a stale or
    /// malformed lease file must not prevent the registry from being consulted.
    /// Non-existent directories return an empty vec.
    #[must_use]
    pub fn load_from_dir(dir: &Path) -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };

        entries
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            })
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|content| serde_json::from_str::<Self>(&content).ok())
            .collect()
    }

    /// Returns `true` when this lease has passed its `expires_at_unix` deadline.
    ///
    /// Leases with no expiry (`expires_at_unix == None`) are never considered
    /// expired by this check alone; callers should additionally verify pid
    /// liveness when needed.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let Some(expires) = self.expires_at_unix else {
            return false;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        now > expires
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Resolution
// ─────────────────────────────────────────────────────────────────────────────

/// A resolved endpoint candidate for a requested capability id.
#[derive(Debug, Clone)]
pub struct EndpointCandidate {
    /// The tool that provides this capability.
    pub tool: String,
    /// How to reach the tool.
    pub transport: TransportKind,
    /// For socket transports: path or `host:port`.
    pub endpoint: Option<String>,
    /// For CLI/stdio transports: binary or command path.
    pub command: Option<PathBuf>,
    /// Version string, when known.
    pub version: Option<String>,
    /// Whether this candidate came from a live runtime lease (true) or the
    /// installed registry fallback (false).
    pub from_lease: bool,
}

/// Resolve a capability id to a preferred endpoint candidate.
///
/// Checks runtime leases first (live endpoints), then falls back to the
/// installed registry. Returns the first non-stale match, or `None` when
/// neither source has the capability.
///
/// Pass explicit `registry_path` and `lease_dir` to override the default
/// ecosystem paths — useful in tests.
///
/// # Errors
///
/// Returns an error only when a registry file exists but cannot be parsed.
/// Missing files and unreadable lease directories are treated as absent rather
/// than erroring.
pub fn resolve_capability(
    capability_id: &str,
    registry_path: &Path,
    lease_dir: &Path,
) -> Result<Option<EndpointCandidate>> {
    // Step 1: check live leases first.
    let leases = RuntimeLease::load_from_dir(lease_dir);
    for lease in &leases {
        if lease.capability_id == capability_id && !lease.is_expired() {
            return Ok(Some(EndpointCandidate {
                tool: lease.tool.clone(),
                transport: lease.transport.clone(),
                endpoint: lease.endpoint.clone(),
                command: lease.command.as_deref().map(PathBuf::from),
                version: lease.version.clone(),
                from_lease: true,
            }));
        }
    }

    // Step 2: fall back to the installed registry.
    let Some(registry) = CapabilityRegistry::load_from(registry_path)? else {
        return Ok(None);
    };

    for entry in &registry.entries {
        if entry.capability_ids.iter().any(|id| id == capability_id) {
            // Skip entries flagged as missing at registration time.
            if matches!(
                entry.health.as_ref().map(|h| &h.status),
                Some(RegistryHealthStatus::Missing)
            ) {
                continue;
            }
            return Ok(Some(EndpointCandidate {
                tool: entry.tool.clone(),
                transport: entry.transport.clone(),
                endpoint: None,
                command: entry.binary_path.as_deref().map(PathBuf::from),
                version: Some(entry.version.clone()),
                from_lease: false,
            }));
        }
    }

    Ok(None)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn registry_fixture() -> &'static str {
        include_str!("../../septa/fixtures/capability-registry-v1.example.json")
    }

    fn lease_fixture() -> &'static str {
        include_str!("../../septa/fixtures/capability-runtime-lease-v1.example.json")
    }

    // ── CapabilityRegistry ───────────────────────────────────────────────────

    #[test]
    fn capability_registry_parses_fixture() {
        let registry: CapabilityRegistry =
            serde_json::from_str(registry_fixture()).expect("fixture parses");
        assert_eq!(registry.schema_version, "1.0");
        assert!(!registry.entries.is_empty());
    }

    #[test]
    fn capability_registry_load_from_absent_returns_none() {
        let dir = tempdir().unwrap();
        let result = CapabilityRegistry::load_from(&dir.path().join("missing.json"));
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn capability_registry_load_from_valid_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("capability-registry-v1.json");
        fs::write(&path, registry_fixture()).unwrap();
        let reg = CapabilityRegistry::load_from(&path).unwrap().unwrap();
        assert!(!reg.entries.is_empty());
    }

    #[test]
    fn capability_registry_load_from_invalid_json_returns_err() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, b"not json").unwrap();
        assert!(CapabilityRegistry::load_from(&path).is_err());
    }

    #[test]
    fn capability_registry_entry_deserializes_manager_variants() {
        let registry: CapabilityRegistry =
            serde_json::from_str(registry_fixture()).expect("fixture parses");
        let managers: Vec<_> = registry.entries.iter().map(|e| &e.manager).collect();
        assert!(managers.iter().any(|m| **m == CapabilityManager::Stipe));
        assert!(managers.iter().any(|m| **m == CapabilityManager::Manual));
    }

    // ── RuntimeLease ─────────────────────────────────────────────────────────

    #[test]
    fn runtime_lease_parses_fixture() {
        let lease: RuntimeLease = serde_json::from_str(lease_fixture()).expect("fixture parses");
        assert_eq!(lease.schema_version, "1.0");
        assert_eq!(lease.capability_id, "memory.store.v1");
    }

    #[test]
    fn runtime_lease_load_from_dir_empty_when_absent() {
        let leases = RuntimeLease::load_from_dir(Path::new("/nonexistent-spore-lease-dir-12345"));
        assert!(leases.is_empty());
    }

    #[test]
    fn runtime_lease_load_from_dir_skips_non_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("not-a-lease.txt"), b"ignore me").unwrap();
        fs::write(dir.path().join("bad.json"), b"not json").unwrap();
        let leases = RuntimeLease::load_from_dir(dir.path());
        assert!(leases.is_empty());
    }

    #[test]
    fn runtime_lease_load_from_dir_reads_valid_lease() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("hyphae.json"), lease_fixture()).unwrap();
        let leases = RuntimeLease::load_from_dir(dir.path());
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].tool, "hyphae");
    }

    #[test]
    fn runtime_lease_not_expired_when_no_expiry() {
        let lease: RuntimeLease = serde_json::from_str(lease_fixture()).unwrap();
        // fixture has expires_at_unix set to a future value (year 2025+)
        // but we check the no-expiry case here
        let mut l = lease;
        l.expires_at_unix = None;
        assert!(!l.is_expired());
    }

    #[test]
    fn runtime_lease_expired_when_past_deadline() {
        let lease: RuntimeLease = serde_json::from_str(lease_fixture()).unwrap();
        let mut l = lease;
        l.expires_at_unix = Some(1); // Unix epoch + 1 second — always in the past
        assert!(l.is_expired());
    }

    // ── resolve_capability ───────────────────────────────────────────────────

    #[test]
    fn resolve_capability_returns_none_when_both_absent() {
        let dir = tempdir().unwrap();
        let result = resolve_capability(
            "memory.store.v1",
            &dir.path().join("no-registry.json"),
            &dir.path().join("no-leases"),
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_capability_returns_registry_entry_when_no_lease() {
        let dir = tempdir().unwrap();
        let reg_path = dir.path().join("registry.json");
        fs::write(&reg_path, registry_fixture()).unwrap();

        let candidate = resolve_capability(
            "memory.store.v1",
            &reg_path,
            &dir.path().join("empty-leases"),
        )
        .unwrap()
        .expect("hyphae entry should be found");

        assert_eq!(candidate.tool, "hyphae");
        assert!(!candidate.from_lease);
    }

    #[test]
    fn resolve_capability_prefers_live_lease_over_registry() {
        let dir = tempdir().unwrap();
        let reg_path = dir.path().join("registry.json");
        let lease_dir = dir.path().join("leases");
        fs::create_dir_all(&lease_dir).unwrap();

        fs::write(&reg_path, registry_fixture()).unwrap();
        // Write a non-expired lease for the same capability
        let mut lease: RuntimeLease = serde_json::from_str(lease_fixture()).unwrap();
        lease.expires_at_unix = None; // open-ended — never expires
        let lease_json = serde_json::to_string(&lease).unwrap();
        fs::write(lease_dir.join("hyphae.json"), lease_json).unwrap();

        let candidate = resolve_capability("memory.store.v1", &reg_path, &lease_dir)
            .unwrap()
            .expect("should resolve");

        assert!(candidate.from_lease, "live lease should win over registry");
    }

    #[test]
    fn resolve_capability_skips_stale_lease_falls_back_to_registry() {
        let dir = tempdir().unwrap();
        let reg_path = dir.path().join("registry.json");
        let lease_dir = dir.path().join("leases");
        fs::create_dir_all(&lease_dir).unwrap();

        fs::write(&reg_path, registry_fixture()).unwrap();
        // Write an expired lease
        let mut lease: RuntimeLease = serde_json::from_str(lease_fixture()).unwrap();
        lease.expires_at_unix = Some(1); // always in the past
        let lease_json = serde_json::to_string(&lease).unwrap();
        fs::write(lease_dir.join("hyphae.json"), lease_json).unwrap();

        let candidate = resolve_capability("memory.store.v1", &reg_path, &lease_dir)
            .unwrap()
            .expect("registry fallback should fire");

        assert!(!candidate.from_lease, "stale lease must not win");
    }

    #[test]
    fn resolve_capability_returns_none_for_unknown_capability() {
        let dir = tempdir().unwrap();
        let reg_path = dir.path().join("registry.json");
        fs::write(&reg_path, registry_fixture()).unwrap();

        let result = resolve_capability(
            "unknown.capability.v99",
            &reg_path,
            &dir.path().join("empty-leases"),
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_capability_skips_missing_health_registry_entries() {
        let dir = tempdir().unwrap();
        // Build a registry with a single "missing" entry
        let reg = serde_json::json!({
            "schema_version": "1.0",
            "written_at_unix": 1_700_000_000_u64,
            "entries": [{
                "tool": "broken",
                "version": "0.1.0",
                "manager": "stipe",
                "capability_ids": ["test.cap.v1"],
                "contract_ids": [],
                "transport": "cli",
                "binary_path": null,
                "health": { "status": "missing", "message": "binary gone" }
            }]
        });
        let reg_path = dir.path().join("registry.json");
        fs::write(&reg_path, serde_json::to_string(&reg).unwrap()).unwrap();

        let result =
            resolve_capability("test.cap.v1", &reg_path, &dir.path().join("empty-leases")).unwrap();
        assert!(result.is_none(), "missing-health entry should be skipped");
    }
}
