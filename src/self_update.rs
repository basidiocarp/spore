//! Self-update from GitHub releases.
//!
//! Shared implementation for checking, downloading, and installing updates
//! from GitHub release assets. Used by mycelium, hyphae, and rhizome.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

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
    println!("Current version: v{current_version}");
    print!("Checking for updates... ");
    std::io::stdout().flush().ok();

    let latest = fetch_latest_release(binary_name, current_version, repo_url)
        .context("Failed to check for updates")?;
    let latest_tag = latest["tag_name"]
        .as_str()
        .context("Missing tag_name in GitHub API response")?;

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

    let asset_name =
        target_asset_name(binary_name).context("Unsupported platform for self-update")?;
    let download_url = find_asset_url(&latest["assets"], &asset_name)
        .with_context(|| format!("No release asset found for '{asset_name}'"))?;

    let current_exe = std::env::current_exe().context("Failed to locate current executable")?;

    println!("Downloading {asset_name}...");
    let archive_bytes = download_binary(binary_name, current_version, &download_url)
        .context("Failed to download update archive")?;

    println!("Extracting...");
    let binary_bytes = extract_binary(&archive_bytes, &asset_name, binary_name)
        .context("Failed to extract binary from archive")?;

    replace_binary(binary_name, &current_exe, &binary_bytes).context("Failed to replace binary")?;

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
        .context("Repository URL is not a github.com URL")?;
    let api_url = format!("https://api.github.com/repos/{repo_path}/releases/latest");

    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(&api_url)
        .header("User-Agent", &format!("{binary_name}/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("Failed to fetch latest release (check your internet connection)")?;

    let json: serde_json::Value = serde_json::from_reader(response.into_body().as_reader())
        .context("Invalid JSON from GitHub API")?;
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
        .call()
        .context("Download failed")?;

    let mut bytes = Vec::new();
    response
        .into_body()
        .as_reader()
        .read_to_end(&mut bytes)
        .context("Failed to read download response")?;

    if bytes.is_empty() {
        bail!("Downloaded binary is empty");
    }
    Ok(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Extract Binary
// ─────────────────────────────────────────────────────────────────────────────

fn extract_binary(archive_bytes: &[u8], asset_name: &str, binary_name: &str) -> Result<Vec<u8>> {
    use std::process::Command;

    let tmp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = tmp_dir.path().join(asset_name);

    std::fs::write(&archive_path, archive_bytes).context("Failed to write archive to temp file")?;

    let exe_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };

    if asset_name.ends_with(".tar.gz") {
        let status = Command::new("tar")
            .args(["xzf", &archive_path.to_string_lossy(), "-C"])
            .arg(tmp_dir.path())
            .status()
            .context("Failed to run tar (is it installed?)")?;
        if !status.success() {
            bail!("tar extraction failed with exit code {status}");
        }
    } else if asset_name.to_ascii_lowercase().ends_with(".zip") {
        let status = Command::new("unzip")
            .args(["-o", &*archive_path.to_string_lossy(), "-d"])
            .arg(tmp_dir.path())
            .status()
            .context("Failed to run unzip (is it installed?)")?;
        if !status.success() {
            bail!("unzip extraction failed with exit code {status}");
        }
    } else {
        return Ok(archive_bytes.to_vec());
    }

    let extracted = tmp_dir.path().join(&exe_name);
    std::fs::read(&extracted).with_context(|| {
        format!(
            "Binary '{exe_name}' not found in archive. Contents: {:?}",
            std::fs::read_dir(tmp_dir.path())
                .map(|entries| entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        )
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Replace Binary
// ─────────────────────────────────────────────────────────────────────────────

fn replace_binary(binary_name: &str, current_exe: &Path, binary_bytes: &[u8]) -> Result<()> {
    let parent = current_exe
        .parent()
        .context("Executable has no parent directory")?;
    let tmp_path = parent.join(format!(".{binary_name}-update.tmp"));

    let write_result = (|| -> Result<()> {
        let mut tmp = std::fs::File::create(&tmp_path).context("Failed to create temp file")?;
        tmp.write_all(binary_bytes)
            .context("Failed to write update to temp file")?;
        tmp.flush().context("Failed to flush temp file")?;
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
            .context("Failed to set executable permissions")?;
    }

    std::fs::rename(&tmp_path, current_exe).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow::anyhow!(
                "Permission denied replacing binary at {}. Try: sudo {binary_name} self-update",
                current_exe.display()
            )
        } else {
            anyhow::anyhow!("Failed to replace binary: {e}")
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
            assert!(n.ends_with(".tar.gz") || n.ends_with(".zip"));
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
