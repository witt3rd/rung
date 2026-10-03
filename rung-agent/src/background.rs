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
    let argv = child_args(args, task_id)?;
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
        .args(argv);
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

/// Argv for the detached child. Every option that shapes the run must be
/// forwarded; one that cannot be is an error, never silently dropped (a
/// dropped `--tools none` would widen the child to the preset roster).
pub fn child_args(args: &Args, task_id: &str) -> Result<Vec<String>, String> {
    let mut v: Vec<String> = vec![
        "--task-id".into(),
        task_id.into(),
        "--toolset".into(),
        args.kind.as_str().into(),
        "--isolation".into(),
        args.isolation.as_str().into(),
    ];
    if let Some(t) = &args.tools {
        v.push("--tools".into());
        v.push(t.clone());
    }
    if args.json {
        v.push("--json".into());
    }
    if args.stream {
        v.push("--stream".into());
    }
    if let Some(n) = args.max_iterations {
        v.push("--max-iterations".into());
        v.push(n.to_string());
    }
    if let Some(s) = &args.system_prompt {
        v.push("--system-prompt".into());
        v.push(s.clone());
    }
    if let Some(u) = &args.user_prompt {
        v.push("--user-prompt".into());
        v.push(u.clone());
    }
    if let Some(m) = &args.memory {
        v.push("--memory".into());
        v.push(m.to_string());
    }
    for spec in &args.mcp {
        match spec {
            crate::mcp::McpSpec::Http { name, url, headers } if headers.is_empty() => {
                v.push("--mcp-http".into());
                v.push(format!("{name}={url}"));
            }
            other => {
                return Err(format!(
                    "background cannot forward MCP server '{}' (stdio or headers)",
                    other.name()
                ));
            }
        }
    }
    if let Some(p) = &args.prompt {
        v.push("--".into());
        v.push(p.clone());
    }
    Ok(v)
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

    #[test]
    fn child_keeps_empty_tools_roster_and_output_mode() {
        let a = Args::parse([
            "x",
            "--tools",
            "none",
            "--background",
            "--json",
            "--stream",
            "hi",
        ])
        .unwrap();
        let v = child_args(&a, "t1").unwrap();
        let i = v
            .iter()
            .position(|s| s == "--tools")
            .expect("--tools dropped");
        assert_eq!(v[i + 1], "none");
        assert!(v.contains(&"--json".to_string()));
        assert!(v.contains(&"--stream".to_string()));
        let back = Args::parse(std::iter::once("x".to_string()).chain(v)).unwrap();
        assert_eq!(back.tools.as_deref(), Some("none"));
    }

    #[test]
    fn child_refuses_unforwardable_mcp() {
        let mut a = Args::parse(["x", "--background", "hi"]).unwrap();
        a.mcp.push(crate::mcp::McpSpec::Stdio {
            name: "s".into(),
            command: "c".into(),
            args: vec![],
            env: vec![],
        });
        assert!(child_args(&a, "t").is_err());
        let dir = std::env::temp_dir().join(format!("rung-bg-refuse-{}", std::process::id()));
        let store = SessionStore::in_cwd(&dir);
        let r = spawn_child(Path::new("/nonexistent"), &a, &dir, "t", &store);
        assert!(r.is_err());
        assert!(!store.dir.join("t.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
