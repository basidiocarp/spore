use crate::types::{Tool, ToolInfo};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

// Only successful probes are stored. A missing key means "not yet found or
// last probe failed" — callers re-probe immediately on cache miss rather than
// returning a cached None. This prevents long-lived daemons from permanently
// losing tools that were installed after startup.
static DISCOVERY_CACHE: OnceLock<Mutex<HashMap<Tool, ToolInfo>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<Tool, ToolInfo>> {
    DISCOVERY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Discover a specific ecosystem tool in PATH.
///
/// Successful probes are cached for the lifetime of the process. Failed probes
/// are not cached: each miss re-probes the filesystem so that tools installed
/// after startup become visible without restarting the daemon.
///
/// Note: in tests that need reproducible discovery results, set up PATH before
/// the test runs or use `probe_uncached` / `clear_cache_for_test` directly.
#[must_use]
pub fn discover(tool: Tool) -> Option<ToolInfo> {
    // Fast path: return cached success.
    if let Ok(guard) = cache().lock() {
        if let Some(info) = guard.get(&tool) {
            return Some(info.clone());
        }
    }

    // Cache miss: probe without holding the lock (probe may block for up to 5 s).
    let info = probe(tool)?;

    // Store successful result.
    if let Ok(mut guard) = cache().lock() {
        guard.insert(tool, info.clone());
    }

    Some(info)
}

/// Probe the given tool without consulting the cache.
///
/// Useful in tests that need a fresh check against the current PATH. Note that
/// calling [`discover`] after this will still return the already-cached value;
/// use this function directly if you need an uncached result.
#[cfg(test)]
#[must_use]
pub fn probe_uncached(tool: Tool) -> Option<ToolInfo> {
    probe(tool)
}

/// Clear a specific tool's entry from the discovery cache.
///
/// For use in sequential tests that need to reset state between runs.
#[cfg(test)]
pub fn clear_cache_for_test(tool: Tool) {
    if let Ok(mut guard) = cache().lock() {
        guard.remove(&tool);
    }
}

/// Discover all ecosystem tools in PATH.
#[must_use]
pub fn discover_all() -> Vec<ToolInfo> {
    Tool::all().iter().filter_map(|&t| discover(t)).collect()
}

fn probe(tool: Tool) -> Option<ToolInfo> {
    let binary_path = which::which(tool.binary_name()).ok()?;

    // Spawn the child process and read its version output in a background thread.
    // The child is spawned here so we can kill it on timeout.
    let mut child = Command::new(&binary_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();

    // Background thread reads the child's stdout and sends it to the channel.
    thread::spawn(move || {
        let mut buf = String::new();
        let mut reader = std::io::BufReader::new(stdout);
        if reader.read_line(&mut buf).is_ok() {
            let _ = tx.send(buf);
        }
    });

    // Wait up to 5 seconds for the output.
    // If timeout occurs, kill the child so it doesn't hang indefinitely.
    if let Ok(output) = rx.recv_timeout(Duration::from_secs(5)) {
        let version = parse_version(&output).unwrap_or_default();
        Some(ToolInfo {
            tool,
            binary_path,
            version,
        })
    } else {
        // Timeout: kill the child to prevent resource leaks.
        let _ = child.kill();
        None
    }
}

fn parse_version(output: &str) -> Option<String> {
    // Expect format: "tool_name X.Y.Z" or just "X.Y.Z"
    let first_line = output.lines().next()?;
    let version_part = first_line.split_whitespace().last()?;
    // Basic semver-ish check
    if version_part.contains('.') {
        Some(version_part.to_string())
    } else {
        Some(first_line.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_with_name() {
        assert_eq!(parse_version("mycelium 0.8.0"), Some("0.8.0".to_string()));
    }

    #[test]
    fn test_parse_version_bare() {
        assert_eq!(parse_version("0.1.0"), Some("0.1.0".to_string()));
    }

    #[test]
    fn test_parse_version_empty() {
        assert_eq!(parse_version(""), None);
    }
}
