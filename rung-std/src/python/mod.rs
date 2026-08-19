//! Stateful CPython sandbox — one strike is one ladder step.
//!
//! The guest is stock CPython in a child (never `exec` in this process). The
//! namespace pickles across strikes and across guest death. Isolation is
//! bubblewrap when present (`Jail::Auto`): host is read-only, the store is
//! writable, network is off, each strike has a wall-clock timeout.
//!
//! Stands alone: [`Sandbox::open`] then [`Sandbox::strike`]. The agent
//! consumes it as a [`Tool`](crate::tools::Tool) via [`Sandbox::as_tool`]
//! so a roster can be *only* Python (the single-tool strategy) or Python
//! plus the filesystem collection.
//!
//! The verb lives on the arrow (`the-law`): JSON write/read and jail spawn
//! happen in `step` / `retry`, not in constructing a verdict.

mod guest;
mod jail;

use crate::llm::ToolDefinition;
use crate::tools::{Tool, ToolCollection};
use rung::ladder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use jail::bwrap_ok;

/// How strictly the guest is isolated from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Jail {
    /// Use bubblewrap when `bwrap` is on PATH, else a bare child.
    #[default]
    Auto,
    /// Fail to spawn if bubblewrap is missing.
    Required,
    /// Bare `python3` child. Tests, or a trusted box.
    Off,
}

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Directory for `namespace.pkl`. Bound read-write in the jail.
    pub store: PathBuf,
    pub hammer: PathBuf,
    pub python: PathBuf,
    /// Extra path visible inside the jail (cwd of the guest when set).
    pub work_dir: Option<PathBuf>,
    pub work_dir_rw: bool,
    pub jail: Jail,
    pub strike_timeout: Duration,
    /// When false (default), the jail has `--unshare-net`.
    pub network: bool,
}

impl SandboxConfig {
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self {
            store: dir.join("store"),
            hammer: default_hammer(),
            python: PathBuf::from("python3"),
            work_dir: None,
            work_dir_rw: false,
            jail: Jail::Auto,
            strike_timeout: Duration::from_secs(30),
            network: false,
        }
    }
}

pub fn default_hammer() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("guest/hammer.py")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrikeRequest {
    pub id: String,
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Strike,
    Ping,
    Reset,
    Shutdown,
}

#[must_use = "StrikeReply carries the guest's output; dropping it silently loses stdout, value, and errors"]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrikeReply {
    pub id: String,
    pub ok: bool,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub error: Option<String>,
}

impl StrikeReply {
    pub fn display(&self) -> String {
        let out = self.stdout.trim();
        if !out.is_empty() {
            return out.to_string();
        }
        if self.value.is_null() {
            String::new()
        } else if let Some(s) = self.value.as_str() {
            s.to_string()
        } else {
            self.value.to_string()
        }
    }
}

impl StrikeRequest {
    pub fn strike(id: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op: Op::Strike,
            code: Some(code.into()),
        }
    }
    pub fn ping(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op: Op::Ping,
            code: None,
        }
    }
    pub fn reset(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op: Op::Reset,
            code: None,
        }
    }
    pub fn shutdown(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op: Op::Shutdown,
            code: None,
        }
    }
}

#[derive(Debug)]
pub enum SandboxError {
    MissingHammer(PathBuf),
    Spawn(String),
    JailUnavailable(String),
    GuestDead,
    Timeout { elapsed_secs: u64 },
    BadReply(String),
    Io(std::io::Error),
}

impl SandboxError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::GuestDead | Self::Spawn(_))
    }
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHammer(p) => write!(f, "hammer is not a file: {}", p.display()),
            Self::Spawn(e) => write!(f, "spawn: {e}"),
            Self::JailUnavailable(e) => write!(f, "jail: {e}"),
            Self::GuestDead => write!(f, "python guest died"),
            Self::Timeout { elapsed_secs } => {
                write!(f, "strike timed out after {elapsed_secs}s")
            }
            Self::BadReply(e) => write!(f, "bad guest JSON: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl From<std::io::Error> for SandboxError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Code to run. Payload of the entry rung.
#[derive(Debug, Clone)]
pub struct Strike {
    pub code: String,
}

/// Handle that owns a live guest. Clone shares the same process (and pickle).
#[derive(Clone)]
pub struct Sandbox {
    guest: Arc<Mutex<guest::Guest>>,
}

impl Sandbox {
    pub fn open(config: SandboxConfig) -> Result<Self, SandboxError> {
        Ok(Self {
            guest: Arc::new(Mutex::new(guest::Guest::open(config)?)),
        })
    }

    /// Drive the ladder once. Respawn-on-death is the recover edge.
    pub fn strike(&self, code: impl Into<String>) -> Result<StrikeReply, SandboxError> {
        let mut pending = pythonstrike::Pending::new(
            Strike { code: code.into() },
            pythonstrike::Carry {
                guest: self.guest.clone(),
            },
        );
        loop {
            match pythonstrike::step(pending) {
                Ok(pythonstrike::StepOutcome::Struck(s)) => return Ok(s.into_payload()),
                Ok(pythonstrike::StepOutcome::Fault(e)) => return Err(e.into_payload()),
                Err(failed) => pending = pythonstrike::retry(failed),
            }
        }
    }

    pub fn reset(&self) -> Result<StrikeReply, SandboxError> {
        self.guest
            .lock()
            .map_err(|_| SandboxError::GuestDead)?
            .reset()
    }

    /// One-tool collection for [`crate::tools::ToolRoster`].
    pub fn as_tool(&self) -> PythonTool {
        PythonTool {
            guest: self.guest.clone(),
        }
    }

    pub fn collection(&self) -> ToolCollection {
        let mut c = ToolCollection::new("python");
        c.admit(self.as_tool());
        c
    }
}

/// `python` tool the agent ladder dispatches. Code in, stdout/value out.
#[derive(Clone)]
pub struct PythonTool {
    guest: Arc<Mutex<guest::Guest>>,
}

impl std::fmt::Debug for PythonTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PythonTool")
    }
}

impl Tool for PythonTool {
    fn name(&self) -> &'static str {
        "python"
    }
    fn description(&self) -> &'static str {
        "Run Python in a persistent sandboxed interpreter. Names assigned in one call remain in the next. Print to return text; a final expression is returned as the value. Use pathlib/os; the work directory is visible. Do not use bash."
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Python to execute in the persistent guest"
                }
            },
            "required": ["code"]
        })
    }
    fn execute(&self, input: &serde_json::Value) -> Result<String, String> {
        let code = input["code"].as_str().ok_or("missing 'code'")?;
        let sandbox = Sandbox {
            guest: self.guest.clone(),
        };
        match sandbox.strike(code) {
            Ok(reply) if reply.ok => {
                let shown = reply.display();
                if shown.is_empty() {
                    Ok("(ok)".into())
                } else {
                    Ok(shown)
                }
            }
            Ok(reply) => Err(reply.error.unwrap_or_else(|| "strike failed".into())),
            Err(e) => Err(e.to_string()),
        }
    }
}

impl PythonTool {
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(self.name(), self.description(), self.input_schema())
    }
}

ladder!(PythonStrike {
    carry {
        guest: Arc<Mutex<guest::Guest>>,
    }

    Pending(Strike)
      => {
          Struck(StrikeReply)
          | Fault(SandboxError)
      }

    recover {
        retry: Failed(Pending) => Pending
    }
} impl {
    step = |pending| {
        let code = pending.payload.code.clone();
        let guest = pending.carry().guest.clone();
        let mut g = match guest.lock() {
            Ok(g) => g,
            Err(_) => {
                return Ok(StepOutcome::Fault(Fault::new(SandboxError::GuestDead)));
            }
        };
        match g.strike(&code) {
            Ok(reply) => Ok(StepOutcome::Struck(Struck::new(reply))),
            Err(e) if e.is_retryable() => {
                drop(g);
                Err(Failed { token: pending, error: e.to_string() })
            }
            Err(e) => Ok(StepOutcome::Fault(Fault::new(e))),
        }
    },

    retry = |f| {
        // Guest::strike already respawns on GuestDead. Re-enter with the
        // same code so a death mid-write is one more attempt.
        let carry = f.token.carry().clone();
        Pending::new(f.token.payload, carry)
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rung-py-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn cfg(jail: Jail) -> (PathBuf, SandboxConfig) {
        let dir = tmp();
        let mut c = SandboxConfig::in_dir(&dir);
        c.jail = jail;
        c.strike_timeout = Duration::from_secs(8);
        (dir, c)
    }

    fn cleanup(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hammer_self_test() {
        let st = std::process::Command::new("python3")
            .arg(default_hammer())
            .arg("--self-test")
            .status()
            .unwrap();
        assert!(st.success());
    }

    #[test]
    fn last_expression_is_the_value() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let r = sb.strike("2 + 2").unwrap();
        assert!(r.ok, "{r:?}");
        assert_eq!(r.value, serde_json::json!(4));
        cleanup(&dir);
    }

    #[test]
    fn print_goes_to_stdout() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let r = sb.strike("print('hi')").unwrap();
        assert!(r.ok);
        assert_eq!(r.display(), "hi");
        cleanup(&dir);
    }

    #[test]
    fn namespace_survives_a_strike() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let _ = sb.strike("x = 21").unwrap();
        let r = sb.strike("x * 2").unwrap();
        assert_eq!(r.value, serde_json::json!(42));
        cleanup(&dir);
    }

    #[test]
    fn namespace_survives_guest_death() {
        let (dir, cfg) = cfg(Jail::Off);
        let store = cfg.store.clone();
        {
            let sb = Sandbox::open(cfg.clone()).unwrap();
            let _ = sb.strike("x = 1").unwrap();
        }
        let mut cfg2 = SandboxConfig::in_dir(&dir);
        cfg2.store = store;
        cfg2.jail = Jail::Off;
        let sb = Sandbox::open(cfg2).unwrap();
        let r = sb.strike("x").unwrap();
        assert_eq!(r.value, serde_json::json!(1));
        cleanup(&dir);
    }

    #[test]
    fn python_error_is_a_failed_strike_not_a_dead_guest() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let r = sb.strike("1/0").unwrap();
        assert!(!r.ok);
        assert!(r.error.as_deref().unwrap_or("").contains("ZeroDivisionError"));
        let still = sb.strike("3").unwrap();
        assert_eq!(still.value, serde_json::json!(3));
        cleanup(&dir);
    }

    #[test]
    fn timeout_kills_a_runaway() {
        let (dir, mut cfg) = cfg(Jail::Off);
        cfg.strike_timeout = Duration::from_millis(400);
        let sb = Sandbox::open(cfg).unwrap();
        let err = sb.strike("import time\ntime.sleep(30)").unwrap_err();
        assert!(matches!(err, SandboxError::Timeout { .. }), "{err}");
        let r = sb.strike("1 + 1").unwrap();
        assert_eq!(r.value, serde_json::json!(2));
        cleanup(&dir);
    }

    #[test]
    fn tool_execute_round_trips() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let tool = sb.as_tool();
        let out = tool
            .execute(&serde_json::json!({"code": "print(2+2)"}))
            .unwrap();
        assert_eq!(out, "4");
        cleanup(&dir);
    }

    #[test]
    fn reset_drops_the_namespace() {
        let (dir, cfg) = cfg(Jail::Off);
        let sb = Sandbox::open(cfg).unwrap();
        let _ = sb.strike("x = 1").unwrap();
        let _ = sb.reset().unwrap();
        let r = sb.strike("x").unwrap();
        assert!(!r.ok);
        cleanup(&dir);
    }

    #[test]
    fn jailed_spawn_when_bwrap_exists() {
        if !bwrap_ok() {
            return;
        }
        let (dir, cfg) = cfg(Jail::Required);
        let sb = Sandbox::open(cfg).unwrap();
        let r = sb.strike("1 + 1").unwrap();
        assert_eq!(r.value, serde_json::json!(2));
        cleanup(&dir);
    }
}
