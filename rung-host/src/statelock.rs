//! One live host per state directory.
//!
//! A host takes an exclusive advisory lock (`flock`) on `host.lock` in its
//! state directory before it touches anything else there, and holds it for
//! its whole life. The kernel drops the lock when the holder dies, however
//! it dies, so a crash never wedges the directory and a leftover file is
//! never a stale lock: only a live holder can refuse a start. The file also
//! carries the holder's pid, so the refusal can name it.
//!
//! The lock file is never removed (removing it would let two hosts lock two
//! different files of the same name). It is opened close-on-exec, so a child
//! the host spawns cannot keep the directory locked after the host is gone.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The lock file's name in the state directory.
pub const FILE: &str = "host.lock";

/// The hold on a state directory; released when dropped or when the
/// process ends.
#[derive(Debug)]
pub struct StateLock {
    _file: File,
    dir: PathBuf,
}

/// Why a state directory could not be locked.
#[derive(Debug)]
pub enum LockError {
    /// A live host holds it (its pid, when it could be read).
    Held {
        dir: PathBuf,
        pid: Option<u32>,
    },
    Io {
        dir: PathBuf,
        err: std::io::Error,
    },
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::Held {
                dir,
                pid: Some(pid),
            } => write!(
                f,
                "state directory {} is in use by a live host (pid {pid}); one host per state",
                dir.display()
            ),
            LockError::Held { dir, pid: None } => write!(
                f,
                "state directory {} is in use by a live host (pid unknown); one host per state",
                dir.display()
            ),
            LockError::Io { dir, err } => {
                write!(f, "state directory {}: lock: {err}", dir.display())
            }
        }
    }
}

impl std::error::Error for LockError {}

impl From<LockError> for std::io::Error {
    fn from(e: LockError) -> Self {
        match e {
            LockError::Io { err, .. } => err,
            held => std::io::Error::new(std::io::ErrorKind::AddrInUse, held.to_string()),
        }
    }
}

impl StateLock {
    /// Lock `dir` (created when absent) for this process, or say who holds
    /// it. Writes nothing but the lock file itself.
    pub fn acquire(dir: &Path) -> Result<StateLock, LockError> {
        let io = |err| LockError::Io {
            dir: dir.to_path_buf(),
            err,
        };
        std::fs::create_dir_all(dir).map_err(io)?;
        let path = dir.join(FILE);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io)?;
        // SAFETY: flock on a descriptor this function owns.
        let got = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if got != 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::WouldBlock {
                return Err(io(err));
            }
            return Err(LockError::Held {
                dir: dir.to_path_buf(),
                pid: holder(&mut file),
            });
        }
        file.set_len(0).map_err(io)?;
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        writeln!(file, "{}", std::process::id()).map_err(io)?;
        file.flush().map_err(io)?;
        Ok(StateLock {
            _file: file,
            dir: dir.to_path_buf(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// The pid the holder wrote. The holder writes it just after it locks, so a
/// read that finds the file empty waits a moment for it.
fn holder(file: &mut File) -> Option<u32> {
    let t = Instant::now();
    loop {
        let mut s = String::new();
        if file.seek(SeekFrom::Start(0)).is_ok()
            && file.read_to_string(&mut s).is_ok()
            && let Ok(pid) = s.trim().parse()
        {
            return Some(pid);
        }
        if t.elapsed() > Duration::from_secs(1) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_is_refused_naming_this_process_and_frees_on_drop() {
        let g = crate::sim::temp_dir_guard("statelock-unit");
        let a = StateLock::acquire(g.path()).unwrap();
        match StateLock::acquire(g.path()) {
            Err(LockError::Held { pid, .. }) => assert_eq!(pid, Some(std::process::id())),
            other => panic!("expected Held, got {other:?}"),
        }
        drop(a);
        StateLock::acquire(g.path()).unwrap();
    }

    #[test]
    fn a_leftover_lock_file_from_a_dead_host_is_not_a_lock() {
        let g = crate::sim::temp_dir_guard("statelock-stale");
        std::fs::write(g.path().join(FILE), "999999\n").unwrap();
        let l = StateLock::acquire(g.path()).unwrap();
        let s = std::fs::read_to_string(g.path().join(FILE)).unwrap();
        assert_eq!(s.trim(), std::process::id().to_string());
        drop(l);
    }
}
