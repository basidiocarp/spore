//! Self-update from GitHub releases.
//!
//! Shared implementation for checking, downloading, and installing updates
//! from GitHub release assets. Used by mycelium, hyphae, and rhizome.

use std::io::{Read, Write};
use std::path::Path;

use crate::error::{Result, SporeError};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Validate that `binary_name` is a plain filename with no path separators
/// or other dangerous characters that could escape the install directory.
///
/// # Errors
///
/// Returns an error if `binary_name` is empty, contains `/` or `\`, or equals `.` or `..`.
fn validate_binary_name(binary_name: &str) -> Result<()> {
    if binary_name.is_empty() {
        return Err(SporeError::Other(
            "binary_name must not be empty".to_string(),
        ));
    }
    if binary_name.contains('/') || binary_name.contains('\\') {
        return Err(SporeError::Other(format!(
            "binary_name '{binary_name}' must not contain path separators"
        )));
    }
    // Prevent relative traversal components such as "." or "..".
    if binary_name == "." || binary_name == ".." {
        return Err(SporeError::Other(format!(
            "binary_name '{binary_name}' is not a valid filename"
        )));
    }
    Ok(())
}

/// Check for updates and optionally download the latest release from GitHub.
///
/// # Arguments
///
/// * `binary_name` — Name of the binary (e.g. "mycelium", "hyphae", "rhizome")
/// * `current_version` — Current semver version (without leading "v")
/// * `repo_url` — Full GitHub repo URL (e.g. `https://github.com/basidiocarp/mycelium`)
/// * `check_only` — If true, only check for updates without downloading
///
/// # Errors
///
/// Returns an error if the network request fails, the binary cannot be downloaded
/// or extracted, or the current binary cannot be replaced.
pub fn run(
    binary_name: &str,
    current_version: &str,
    repo_url: &str,
    check_only: bool,
) -> Result<()> {
    validate_binary_name(binary_name)?;

    println!("Current version: v{current_version}");
    print!("Checking for updates... ");
    std::io::stdout().flush().ok();

    let latest = fetch_latest_release(binary_name, current_version, repo_url)
        .map_err(|_| SporeError::Network("Failed to check for updates".to_string()))?;
    let latest_tag = latest["tag_name"].as_str().ok_or_else(|| {
        SporeError::Network("Missing tag_name in GitHub API response".to_string())
    })?;

    let latest_version = latest_tag.trim_start_matches('v');
    println!("Latest version: {latest_tag}");

    if latest_version == current_version {
        println!("Already up to date.");
        return Ok(());
    }

    println!("Update available: v{current_version} → {latest_tag}");

    if check_only {
        println!("Run `{binary_name} self-update` to install.");
        return Ok(());
    }

    let asset_name = target_asset_name(binary_name)
        .ok_or_else(|| SporeError::Other("Unsupported platform for self-update".to_string()))?;
    let download_url = find_asset_url(&latest["assets"], &asset_name)
        .ok_or_else(|| SporeError::Network(format!("No release asset found for '{asset_name}'")))?;

    let current_exe = std::env::current_exe()
        .map_err(|_| SporeError::Other("Failed to locate current executable".to_string()))?;

    println!("Downloading {asset_name}...");
    let archive_bytes = download_binary(binary_name, current_version, &download_url)
        .map_err(|_| SporeError::Network("Failed to download update archive".to_string()))?;

    println!("Extracting...");
    let binary_bytes = extract_binary(&archive_bytes, &asset_name, binary_name)
        .map_err(|_| SporeError::Other("Failed to extract binary from archive".to_string()))?;

    replace_binary(binary_name, &current_exe, &binary_bytes)
        .map_err(|_| SporeError::Other("Failed to replace binary".to_string()))?;

    println!("Updated to {latest_tag}. Run `{binary_name} --version` to confirm.");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Fetch Latest Release
// ─────────────────────────────────────────────────────────────────────────────

/// Fetch the latest release metadata from GitHub API.
///
/// # Errors
///
/// Returns an error if the network request fails or the response is invalid JSON.
pub fn fetch_latest_release(
    binary_name: &str,
    current_version: &str,
    repo_url: &str,
) -> Result<serde_json::Value> {
    let repo_path = repo_url
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")
        .ok_or_else(|| SporeError::Network("Repository URL is not a github.com URL".to_string()))?;
    let api_url = format!("https://api.github.com/repos/{repo_path}/releases/latest");

    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(&api_url)
        .header("User-Agent", &format!("{binary_name}/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|_| {
            SporeError::Network(
                "Failed to fetch latest release (check your internet connection)".to_string(),
            )
        })?;

    let json: serde_json::Value =
        serde_json::from_reader(response.into_body().as_reader()).map_err(SporeError::Json)?;
    Ok(json)
}

// ─────────────────────────────────────────────────────────────────────────────
// Target Asset Name
// ─────────────────────────────────────────────────────────────────────────────

/// Determine the expected release asset name for the current platform.
///
/// Returns `None` on unsupported OS/arch combinations.
#[must_use]
pub fn target_asset_name(binary_name: &str) -> Option<String> {
    let (os_suffix, ext) = match std::env::consts::OS {
        "macos" => ("apple-darwin", ".tar.gz"),
        "linux" => ("unknown-linux-musl", ".tar.gz"),
        "windows" => ("pc-windows-msvc", ".zip"),
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    Some(format!("{binary_name}-{arch}-{os_suffix}{ext}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Find Asset URL
// ─────────────────────────────────────────────────────────────────────────────

/// Search GitHub release assets for a matching download URL.
#[must_use]
pub fn find_asset_url(assets: &serde_json::Value, name: &str) -> Option<String> {
    assets.as_array()?.iter().find_map(|asset| {
        let asset_name = asset["name"].as_str()?;
        if asset_name == name {
            asset["browser_download_url"].as_str().map(String::from)
        } else {
            None
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Download Binary
// ─────────────────────────────────────────────────────────────────────────────

fn download_binary(binary_name: &str, current_version: &str, url: &str) -> Result<Vec<u8>> {
    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(url)
        .header("User-Agent", &format!("{binary_name}/{current_version}"))
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|_| SporeError::Network("Download failed".to_string()))?;

    let mut bytes = Vec::new();
    response
        .into_body()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|_| SporeError::Network("Failed to read download response".to_string()))?;

    if bytes.is_empty() {
        return Err(SporeError::Other("Downloaded binary is empty".to_string()));
    }
    Ok(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Extract Binary
// ─────────────────────────────────────────────────────────────────────────────

fn extract_binary(archive_bytes: &[u8], asset_name: &str, binary_name: &str) -> Result<Vec<u8>> {
    use std::process::Command;

    let tmp_dir = tempfile::tempdir()
        .map_err(|_| SporeError::Other("Failed to create temp directory".to_string()))?;
    let archive_path = tmp_dir.path().join(asset_name);

    std::fs::write(&archive_path, archive_bytes)
        .map_err(|_| SporeError::Other("Failed to write archive to temp file".to_string()))?;

    let exe_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };

    if asset_name.ends_with(".tar.gz") {
        let status = Command::new("tar")
            .args(["xzf"])
            .arg(archive_path.as_os_str())
            .arg("-C")
            .arg(tmp_dir.path())
            .status()
            .map_err(|_| SporeError::Other("Failed to run tar (is it installed?)".to_string()))?;
        if !status.success() {
            return Err(SporeError::Other(format!(
                "tar extraction failed with exit code {status}"
            )));
        }
    } else if asset_name.to_ascii_lowercase().ends_with(".zip") {
        let status = Command::new("unzip")
            .args(["-o"])
            .arg(archive_path.as_os_str())
            .arg("-d")
            .arg(tmp_dir.path())
            .status()
            .map_err(|_| SporeError::Other("Failed to run unzip (is it installed?)".to_string()))?;
        if !status.success() {
            return Err(SporeError::Other(format!(
                "unzip extraction failed with exit code {status}"
            )));
        }
    } else {
        return Ok(archive_bytes.to_vec());
    }

    let extracted = tmp_dir.path().join(&exe_name);
    std::fs::read(&extracted).map_err(|_| {
        let contents = std::fs::read_dir(tmp_dir.path())
            .map(|entries| {
                entries
                    .filter_map(std::result::Result::ok)
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        SporeError::Other(format!(
            "Binary '{exe_name}' not found in archive. Contents: {contents:?}"
        ))
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Replace Binary
// ─────────────────────────────────────────────────────────────────────────────

fn replace_binary(binary_name: &str, current_exe: &Path, binary_bytes: &[u8]) -> Result<()> {
    let parent = current_exe
        .parent()
        .ok_or_else(|| SporeError::Other("Executable has no parent directory".to_string()))?;
    let tmp_path = parent.join(format!(".{binary_name}-update.tmp"));

    let write_result = (|| -> Result<()> {
        let mut tmp = std::fs::File::create(&tmp_path)
            .map_err(|_| SporeError::Other("Failed to create temp file".to_string()))?;
        tmp.write_all(binary_bytes)
            .map_err(|_| SporeError::Other("Failed to write update to temp file".to_string()))?;
        tmp.flush()
            .map_err(|_| SporeError::Other("Failed to flush temp file".to_string()))?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|_| SporeError::Other("Failed to set executable permissions".to_string()))?;
    }

    std::fs::rename(&tmp_path, current_exe).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            SporeError::Other(format!(
                "Permission denied replacing binary at {}. Try: sudo {binary_name} self-update",
                current_exe.display()
            ))
        } else {
            SporeError::Other(format!("Failed to replace binary: {e}"))
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_asset_name_known_platform() {
        let name = target_asset_name("mycelium");
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos" | "linux" | "windows", "x86_64" | "aarch64") => {
                assert!(name.is_some());
                let n = name.unwrap();
                assert!(n.starts_with("mycelium-"));
                assert!(n.contains(std::env::consts::ARCH));
            }
            _ => {
                assert!(name.is_none());
            }
        }
    }

    #[test]
    fn test_target_asset_name_format() {
        let name = target_asset_name("hyphae");
        if let Some(n) = name {
            assert!(n.starts_with("hyphae-"));
            assert!(n.contains('-'));
            assert!(
                n.ends_with(".tar.gz")
                    || std::path::Path::new(&n)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            );
        }
    }

    #[test]
    fn test_find_asset_url_present() {
        let assets = serde_json::json!([
            {"name": "mycelium-x86_64-apple-darwin.tar.gz", "browser_download_url": "https://example.com/mycelium.tar.gz"},
            {"name": "mycelium-x86_64-unknown-linux-musl.tar.gz", "browser_download_url": "https://example.com/mycelium-linux.tar.gz"},
        ]);
        let url = find_asset_url(&assets, "mycelium-x86_64-apple-darwin.tar.gz");
        assert_eq!(url, Some("https://example.com/mycelium.tar.gz".to_string()));
    }

    #[test]
    fn test_find_asset_url_missing() {
        let assets = serde_json::json!([
            {"name": "mycelium-x86_64-unknown-linux-musl.tar.gz", "browser_download_url": "https://example.com/mycelium-linux.tar.gz"},
        ]);
        let url = find_asset_url(&assets, "mycelium-x86_64-apple-darwin.tar.gz");
        assert!(url.is_none());
    }

    #[test]
    fn test_find_asset_url_empty() {
        let assets = serde_json::json!([]);
        let url = find_asset_url(&assets, "anything");
        assert!(url.is_none());
    }

    #[test]
    fn test_find_asset_url_not_array() {
        let assets = serde_json::json!(null);
        let url = find_asset_url(&assets, "anything");
        assert!(url.is_none());
    }
}
