use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::{Result, SporeError};

/// Write `value` as pretty-printed JSON to `path` atomically.
/// Writes to `{path}.tmp`, fsyncs, then renames over the target.
/// Cleans up the tmp file on error.
///
/// # Errors
/// Returns `SporeError` if serialization, file creation, fsync, or rename fails.
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| SporeError::Other(format!("serialize: {e}")))?;
    atomic_write_bytes(path, &bytes)
}

/// Write raw bytes to `path` atomically.
/// Writes to `{path}.tmp`, fsyncs, then renames over the target.
/// Cleans up the tmp file on error.
///
/// # Errors
/// Returns `SporeError` if directory creation, file creation, fsync, or rename fails.
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SporeError::Other(format!("create dirs: {e}")))?;
        }
    }

    let tmp_path = {
        let mut p = path.to_path_buf();
        let ext = match p.extension() {
            Some(e) => format!("{}.tmp", e.to_string_lossy()),
            None => "tmp".to_string(),
        };
        p.set_extension(ext);
        p
    };

    let write_result = (|| {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| SporeError::Other(format!("create tmp: {e}")))?;
        file.write_all(bytes)
            .map_err(|e| SporeError::Other(format!("write tmp: {e}")))?;
        file.sync_all()
            .map_err(|e| SporeError::Other(format!("sync tmp: {e}")))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return write_result;
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        SporeError::Other(format!("rename: {e}"))
    })
}

/// A simple exclusive file lock backed by a `.lock` file.
/// Dropped when the `FileLock` goes out of scope.
pub struct FileLock {
    lock_path: std::path::PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock for `path` by creating `{path}.lock`.
    /// Returns an error if the lock file already exists.
    ///
    /// # Errors
    /// Returns `SporeError` if the lock file already exists or cannot be created.
    pub fn acquire(path: &Path) -> Result<Self> {
        let lock_path = path.with_extension(match path.extension() {
            Some(e) => format!("{}.lock", e.to_string_lossy()),
            None => "lock".to_string(),
        });
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| SporeError::Other(format!("lock acquire {}: {e}", lock_path.display())))?;
        Ok(FileLock { lock_path })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn atomic_write_json_produces_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let value = json!({"key": "value", "count": 42});
        atomic_write_json(&path, &value).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn atomic_write_bytes_leaves_no_tmp_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.bin");
        atomic_write_bytes(&path, b"hello world").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
        // Verify no .tmp file was left behind
        let tmp = path.with_extension("bin.tmp");
        assert!(!tmp.exists());
    }

    #[test]
    fn file_lock_acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resource.json");
        let lock = FileLock::acquire(&path).unwrap();
        // Second acquire should fail (lock file exists)
        assert!(FileLock::acquire(&path).is_err());
        drop(lock);
        // After drop, lock file removed — can acquire again
        let _lock2 = FileLock::acquire(&path).unwrap();
    }

    #[test]
    fn atomic_write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("state.json");
        atomic_write_json(&path, &json!({"ok": true})).unwrap();
        assert!(path.exists());
    }
}
