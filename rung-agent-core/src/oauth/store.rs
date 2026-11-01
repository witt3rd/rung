//! Where a login lives: machine-local, mode 0600, never in a repo.
//!
//! ```text
//! $RUNG_HOME/oauth/            (default ~/.rung/oauth, mode 0700)
//!   <provider>--<profile>.json   the login (mode 0600)
//!   <provider>--<profile>.lock   the cross-process lock (mode 0600)
//! ```
//!
//! `~/.rung/auth.yaml` stays what it is, a flat provider-to-key map that the
//! driver parses as strings only: a structured value in it would blank the
//! whole file for the driver. A login is structured (a long-lived token, a
//! minted one and its expiry), so it has its own file next to it.
//!
//! A writer replaces the file whole (write a sibling, `rename`), so a reader
//! sees the old login or the new one, never half of one.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The profile used when none is named.
pub const DEFAULT_PROFILE: &str = "default";

/// A short-lived token minted from a login, with when it stops being good.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Minted {
    pub token: String,
    /// Seconds since the Unix epoch.
    pub expires_at: u64,
}

impl fmt::Debug for Minted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Minted")
            .field("token", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// One stored login: a provider, a profile, and what the provider gave.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Login {
    pub provider: String,
    pub profile: String,
    /// The long-lived token the provider issued at login.
    pub refresh: String,
    /// The latest short-lived token minted from it, shared by every process
    /// that uses this login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<Minted>,
    /// An enterprise domain this login was made against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enterprise_domain: Option<String>,
    /// When the login was made (seconds since the epoch).
    pub created_at: u64,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("provider", &self.provider)
            .field("profile", &self.profile)
            .field("refresh", &"[redacted]")
            .field("access", &self.access)
            .field("enterprise_domain", &self.enterprise_domain)
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// The directory of logins.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

/// A profile or provider name is a path component: letters, digits, `.`,
/// `_`, `-`, not starting with `.`, at most 64 long. This keeps a name from
/// reaching outside the store, and `--` out of it (the separator).
pub fn valid_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "'{name}' is not a valid name (letters, digits, '.', '_', '-'; no '--'; at most 64)"
        ))
    }
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$RUNG_HOME/oauth`, else `$HOME/.rung/oauth`.
    pub fn default_dir_from(getenv: impl Fn(&str) -> Option<String>) -> PathBuf {
        let set = |k: &str| {
            getenv(k)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        match set("RUNG_HOME") {
            Some(h) => PathBuf::from(h),
            None => PathBuf::from(set("HOME").unwrap_or_else(|| ".".into())).join(".rung"),
        }
        .join("oauth")
    }

    /// The store the environment names.
    pub fn from_env() -> Self {
        Self::new(Self::default_dir_from(|k| std::env::var(k).ok()))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn login_path(&self, provider: &str, profile: &str) -> PathBuf {
        self.dir.join(format!("{provider}--{profile}.json"))
    }

    fn lock_path(&self, provider: &str, profile: &str) -> PathBuf {
        self.dir.join(format!("{provider}--{profile}.lock"))
    }

    fn ensure_dir(&self) -> Result<(), String> {
        if self.dir.is_dir() {
            return Ok(());
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
            .map_err(|e| format!("oauth store {}: {e}", self.dir.display()))
    }

    /// The login, or `None` when there is none.
    pub fn load(&self, provider: &str, profile: &str) -> Result<Option<Login>, String> {
        valid_name(provider)?;
        valid_name(profile)?;
        let path = self.login_path(provider, profile);
        match fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("oauth login {}: {e}", path.display())),
            // The parse error names the file, never its text: the text is a secret.
            Ok(body) => serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| format!("oauth login {}: not a valid login ({e})", path.display())),
        }
    }

    /// Replace the login whole, at mode 0600.
    pub fn save(&self, login: &Login) -> Result<(), String> {
        valid_name(&login.provider)?;
        valid_name(&login.profile)?;
        self.ensure_dir()?;
        let path = self.login_path(&login.provider, &login.profile);
        let body = serde_json::to_vec_pretty(login).map_err(|e| format!("oauth login: {e}"))?;
        self.write_aside(&path, &body)
    }

    /// Write `body` to `path` (inside the store) whole and at mode 0600: a
    /// sibling file first, then `rename`.
    pub fn write_aside(&self, path: &Path, body: &[u8]) -> Result<(), String> {
        self.ensure_dir()?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("oauth store: bad path")?;
        let tmp = self.dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let write = || -> std::io::Result<()> {
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            // `mode` applies at creation only; a leftover file keeps its own.
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
            f.write_all(body)?;
            f.sync_all()?;
            fs::rename(&tmp, path)
        };
        write().map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("oauth store {}: {e}", path.display())
        })
    }

    /// Delete the login. `Ok(false)` when there was none.
    pub fn remove(&self, provider: &str, profile: &str) -> Result<bool, String> {
        valid_name(provider)?;
        valid_name(profile)?;
        let path = self.login_path(provider, profile);
        // The model list was fetched with this login; it goes with it.
        let _ = fs::remove_file(self.dir.join(format!("{provider}--{profile}.models.json")));
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("oauth login {}: {e}", path.display())),
        }
    }

    /// Every stored `(provider, profile)`, sorted.
    pub fn list(&self) -> Result<Vec<(String, String)>, String> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("oauth store {}: {e}", self.dir.display())),
        };
        let mut out: Vec<(String, String)> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let stem = name.strip_suffix(".json")?;
                let (provider, profile) = stem.split_once("--")?;
                (valid_name(provider).is_ok() && valid_name(profile).is_ok())
                    .then(|| (provider.to_string(), profile.to_string()))
            })
            .collect();
        out.sort();
        Ok(out)
    }

    /// Hold this login's cross-process lock until the guard drops. Every
    /// process that mints for one login takes it, so one of them mints and
    /// the rest read the result. The kernel releases it if its holder dies.
    pub fn lock(&self, provider: &str, profile: &str) -> Result<Lock, String> {
        valid_name(provider)?;
        valid_name(profile)?;
        self.ensure_dir()?;
        let path = self.lock_path(provider, profile);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(|e| format!("oauth lock {}: {e}", path.display()))?;
        // SAFETY: `file` owns a valid descriptor for the whole call.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(format!(
                "oauth lock {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(Lock { _file: file })
    }
}

/// The held lock; closing the file releases it.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}
