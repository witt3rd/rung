//! Test-only helpers shared by the product crates.
//!
//! [`TempDir`] is the one way a test makes a scratch directory: it is created
//! under the system temp root and removed on drop (including on panic), so a
//! test run leaves nothing behind. CI runs `scripts/test_tmp_clean.sh`, which runs
//! the tests with a private temp root and fails if any entry survives in it.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A scratch directory removed (recursively) when dropped.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create `<temp>/rung-<tag>-<pid>-<nanos>-<seq>`.
    pub fn new(tag: &str) -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("rung-{tag}-{}-{nanos}-{seq}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<std::ffi::OsStr> for TempDir {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path.as_os_str()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_on_drop() {
        let d = TempDir::new("testkit");
        let p = d.to_path_buf();
        std::fs::write(p.join("f"), "x").unwrap();
        assert!(p.exists());
        drop(d);
        assert!(!p.exists());
    }
}
