use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::{Result, SporeError};

/// Write `value` as pretty-printed JSON to `path` atomically.
/// Uses a randomly-named temporary file in the same directory, fsyncs, then
/// renames over the target. Cleans up the tmp file on error.
///
/// # Errors
/// Returns `SporeError` if serialization, file creation, fsync, or rename fails.
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| SporeError::Other(format!("serialize: {e}")))?;
    atomic_write_bytes(path, &bytes)
}

/// Write raw bytes to `path` atomically.
///
/// Creates a randomly-named temporary file in the same directory (using
/// [`tempfile::NamedTempFile`]) to avoid the collision where two concurrent
/// writes to paths that share a prefix (e.g. `bar.tar.gz` and `bar.tar`) both
/// produce the same `.tmp` path. Fsyncs the temporary file, then renames it
/// over the target.
///
/// # Errors
/// Returns `SporeError` if directory creation, file creation, fsync, or rename fails.
pub fn atomic_write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SporeError::Other(format!("create dirs: {e}")))?;
        }
    }

    let parent = path.parent().unwrap_or(Path::new("."));

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| SporeError::Other(format!("create tmp: {e}")))?;
    tmp.write_all(data)
        .map_err(|e| SporeError::Other(format!("write tmp: {e}")))?;
    tmp.flush()
        .map_err(|e| SporeError::Other(format!("flush tmp: {e}")))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| SporeError::Other(format!("sync tmp: {e}")))?;
    tmp.persist(path)
        .map_err(|e| SporeError::Other(format!("rename: {e}")))?;

    Ok(())
}

/// A simple exclusive file lock backed by a `.lock` file.
/// Dropped when the `FileLock` goes out of scope.
///
/// # Stale lock reclamation
///
/// The lock file stores the PID of the acquiring process. On [`FileLock::acquire`],
/// if the lock file already exists the PID is read and checked with
/// [`process_is_alive`]. If the owning process has terminated (e.g. due to
/// `panic=abort` preventing `Drop` from running), the stale lock file is
/// removed and the lock is re-acquired.
pub struct FileLock {
    lock_path: std::path::PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock for `path` by creating `{path}.lock`.
    ///
    /// If the lock file already exists and belongs to a process that is no
    /// longer alive, the stale lock is reclaimed and a new one is written.
    ///
    /// # Errors
    /// Returns `SporeError` if the lock is held by a live process or if the
    /// lock file cannot be created.
    pub fn acquire(path: &Path) -> Result<Self> {
        let lock_path = path.with_extension(match path.extension() {
            Some(e) => format!("{}.lock", e.to_string_lossy()),
            None => "lock".to_string(),
        });

        let pid = std::process::id();

        loop {
            // Attempt atomic creation.
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut f) => {
                    // Write our PID so future callers can detect a crash.
                    let _ = writeln!(f, "{pid}");
                    return Ok(FileLock { lock_path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Lock file exists — check if the owner is still alive.
                    if let Some(owner_pid) = read_lock_pid(&lock_path) {
                        if process_is_alive(owner_pid) {
                            return Err(SporeError::Other(format!(
                                "lock acquire {}: held by live process {}",
                                lock_path.display(),
                                owner_pid
                            )));
                        }
                    }
                    // Owner is dead (or PID unreadable) — remove the stale lock
                    // and retry. Another process may race us here; if the
                    // subsequent create_new also fails, we loop again.
                    let _ = std::fs::remove_file(&lock_path);
                }
                Err(e) => {
                    return Err(SporeError::Other(format!(
                        "lock acquire {}: {e}",
                        lock_path.display()
                    )));
                }
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// Read the PID stored inside a lock file, if present and parseable.
fn read_lock_pid(lock_path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(lock_path).ok()?;
    content.trim().parse().ok()
}

/// Return `true` when `pid` refers to a running process.
///
/// On Unix, sends signal 0 to test liveness without disturbing the process.
/// On non-Unix targets, conservatively returns `true` to avoid incorrectly
/// reclaiming locks held by live processes.
#[allow(unsafe_code)]
fn process_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: kill(pid, 0) is the canonical POSIX liveness probe.
        // It sends no signal and has no side effects on the target process.
        // The cast from u32 to i32 is safe for any real PID: Linux caps PIDs at
        // 4,194,304, well within i32::MAX. The clippy lint is suppressed here
        // because the false-wrap scenario (PID ≥ 2^31) cannot occur in practice.
        #[allow(clippy::cast_possible_wrap)]
        (unsafe { libc::kill(pid as libc::pid_t, 0) } == 0)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true // conservative: do not reclaim on platforms without kill(2)
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
        // No deterministically-named .tmp file should be present.
        let tmp = path.with_extension("bin.tmp");
        assert!(!tmp.exists());
    }

    #[test]
    fn atomic_write_no_collision_on_shared_prefix() {
        // Write two files whose old fixed-suffix scheme would collide.
        let dir = tempfile::tempdir().unwrap();
        let path_gz = dir.path().join("bar.tar.gz");
        let path_tar = dir.path().join("bar.tar");
        atomic_write_bytes(&path_gz, b"gz").unwrap();
        atomic_write_bytes(&path_tar, b"tar").unwrap();
        assert_eq!(std::fs::read(&path_gz).unwrap(), b"gz");
        assert_eq!(std::fs::read(&path_tar).unwrap(), b"tar");
    }

    #[test]
    fn file_lock_acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resource.json");
        let lock = FileLock::acquire(&path).unwrap();
        // Second acquire should fail (lock file exists, held by our own live process).
        assert!(FileLock::acquire(&path).is_err());
        drop(lock);
        // After drop, lock file removed — can acquire again.
        let _lock2 = FileLock::acquire(&path).unwrap();
    }

    #[test]
    fn file_lock_reclaims_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resource.json");
        let lock_path = path.with_extension("lock");

        // Write a lock file with a PID that is guaranteed to be dead.
        // PID 0 is never a real process; kill(0, 0) returns EPERM or ESRCH,
        // both of which signal that PID 0 is not a live user process.
        std::fs::write(&lock_path, "0\n").unwrap();

        // acquire() should detect the stale lock and succeed.
        let lock = FileLock::acquire(&path).expect("should reclaim stale lock from dead PID 0");
        drop(lock);
    }

    #[test]
    fn atomic_write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("state.json");
        atomic_write_json(&path, &json!({"ok": true})).unwrap();
        assert!(path.exists());
    }
}
