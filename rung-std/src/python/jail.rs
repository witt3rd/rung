//! Spawn the CPython guest, optionally inside bubblewrap.

use super::{Jail, SandboxConfig, SandboxError};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub fn spawn(config: &SandboxConfig) -> Result<Child, SandboxError> {
    match config.jail {
        Jail::Off => spawn_bare(config),
        Jail::Required => spawn_jailed(config),
        Jail::Auto => spawn_jailed(config).or_else(|err| {
            if bwrap_ok() {
                Err(err)
            } else {
                spawn_bare(config)
            }
        }),
    }
}

pub fn bwrap_ok() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn spawn_bare(config: &SandboxConfig) -> Result<Child, SandboxError> {
    std::fs::create_dir_all(&config.store).map_err(SandboxError::Io)?;
    Command::new(&config.python)
        .arg(&config.hammer)
        .env("RUNG_PYTHON_STORE", &config.store)
        .env("PYTHONUNBUFFERED", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| SandboxError::Spawn(e.to_string()))
}

fn spawn_jailed(config: &SandboxConfig) -> Result<Child, SandboxError> {
    if !bwrap_ok() {
        return Err(SandboxError::JailUnavailable(
            "bwrap not found (install bubblewrap)".into(),
        ));
    }
    std::fs::create_dir_all(&config.store).map_err(SandboxError::Io)?;
    let store = abs(&config.store)?;
    let hammer = abs(&config.hammer)?;
    let python = config.python.clone();

    let mut cmd = Command::new("bwrap");
    cmd.args([
        "--die-with-parent",
        "--unshare-pid",
        "--ro-bind",
        "/",
        "/",
        "--dev",
        "/dev",
        "--proc",
        "/proc",
        "--tmpfs",
        "/tmp",
        "--bind",
    ]);
    cmd.arg(&store).arg(&store);
    if let Some(wd) = &config.work_dir {
        let wd = abs(wd)?;
        if config.work_dir_rw {
            cmd.arg("--bind");
        } else {
            cmd.arg("--ro-bind");
        }
        cmd.arg(&wd).arg(&wd);
        cmd.arg("--chdir").arg(&wd);
    } else {
        cmd.arg("--chdir").arg(&store);
    }
    if !config.network {
        cmd.arg("--unshare-net");
    }
    cmd.arg("--setenv")
        .arg("RUNG_PYTHON_STORE")
        .arg(&store)
        .arg("--setenv")
        .arg("PYTHONUNBUFFERED")
        .arg("1")
        .arg("--")
        .arg(&python)
        .arg(&hammer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    cmd.spawn()
        .map_err(|e| SandboxError::Spawn(format!("bwrap: {e}")))
}

fn abs(path: &Path) -> Result<std::path::PathBuf, SandboxError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(SandboxError::Io)
    }
}
