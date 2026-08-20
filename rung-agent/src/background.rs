//! Detach a child of this binary. Nested `task` stays in-process (kernel
//! [`rung_std::tools::Spawn`] is synchronous); only the CLI run backgrounds.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::args::Args;
use crate::session::SessionStore;

pub const CHILD_ENV: &str = "RUNG_AGENT_CHILD";

#[derive(Debug, Clone)]
pub struct Launch {
    pub task_id: String,
    pub pid: u32,
    pub log: PathBuf,
}

pub fn spawn_child(
    exe: &Path,
    args: &Args,
    origin: &Path,
    task_id: &str,
    store: &SessionStore,
) -> Result<Launch, String> {
    std::fs::create_dir_all(&store.dir).map_err(|e| format!("sessions dir: {e}"))?;
    let log = store.dir.join(format!("{task_id}.log"));
    let file = File::create(&log).map_err(|e| format!("log: {e}"))?;
    let err = file.try_clone().map_err(|e| format!("log: {e}"))?;
    let mut cmd = Command::new(exe);
    cmd.current_dir(origin)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(err))
        .env(CHILD_ENV, "1")
        .arg("--task-id")
        .arg(task_id)
        .arg("--type")
        .arg(args.kind.as_str())
        .arg("--isolation")
        .arg(args.isolation.as_str());
    if let Some(n) = args.max_iterations {
        cmd.arg("--max-iterations").arg(n.to_string());
    }
    if let Some(p) = &args.prompt {
        cmd.arg("--");
        cmd.arg(p);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
    Ok(Launch {
        task_id: task_id.to_string(),
        pid: child.id(),
        log,
    })
}

pub fn in_child() -> bool {
    std::env::var_os(CHILD_ENV).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_env_name_is_stable() {
        assert_eq!(CHILD_ENV, "RUNG_AGENT_CHILD");
    }
}
