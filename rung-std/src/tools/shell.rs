//! Opt-in shell. Not part of the default filesystem collection.
//!
//! A call returns when `bash` exits, not when every process holding its
//! output closes it: stdout and stderr go to files, so a server started in
//! the background (`node app.js &`) no longer holds the call open forever.
//! A command still running at the deadline (`RUNG_SHELL_TIMEOUT_SECS`,
//! default 300) is killed with its whole process group, and the call says so.

use super::Tool;
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The deadline for one shell call when `RUNG_SHELL_TIMEOUT_SECS` is unset.
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;

#[derive(Debug)]
pub struct Shell;

fn timeout() -> Duration {
    let secs = std::env::var("RUNG_SHELL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// A scratch file for one stream of one call, removed when dropped.
struct Capture(PathBuf);

impl Capture {
    fn new(stream: &str) -> Result<(Self, std::fs::File), String> {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rung-shell-{}-{}-{stream}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let file = std::fs::File::create(&path).map_err(|e| format!("shell: {e}"))?;
        Ok((Self(path), file))
    }

    fn read(&self) -> String {
        std::fs::read(&self.0)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Run `command` under `bash -c`, returning when bash exits or `limit`
/// passes (then its process group is killed).
pub fn run(command: &str, limit: Duration) -> Result<String, String> {
    let (out, out_file) = Capture::new("stdout")?;
    let (err, err_file) = Capture::new("stderr")?;
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(out_file)
        .stderr(err_file)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("shell: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| format!("shell: {e}"))? {
            Some(status) => break Some(status),
            None if started.elapsed() >= limit => break None,
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    if status.is_none() {
        // The group is the command and everything it started.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
        let _ = child.wait();
    }
    let mut parts = Vec::new();
    let stdout = out.read();
    let stderr = err.read();
    if !stdout.trim().is_empty() {
        parts.push(stdout.trim().to_string());
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("[stderr]\n{}", stderr.trim()));
    }
    match status {
        Some(s) => parts.push(format!("[exit: {}]", s.code().unwrap_or(-1))),
        None => parts.push(format!(
            "[timed out after {}s: the command was still running and was killed with \
             everything it started. Start a long-running process (a server, a watcher) \
             in the background with its output redirected, e.g. `cmd > app.log 2>&1 &`, \
             then check on it.]",
            limit.as_secs()
        )),
    }
    Ok(parts.join("\n"))
}

impl Tool for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }
    fn description(&self) -> &'static str {
        "Execute a shell command via `bash -c`. Returns stdout, stderr, and exit code. \
         Returns when bash exits; a command still running after the time limit is killed."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to execute"}
            },
            "required": ["command"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let command = input["command"].as_str().ok_or("missing 'command'")?;
        if command.trim().is_empty() {
            return Err("shell: empty command".into());
        }
        let destructive = ["rm -rf /", "dd if=", "mkfs.", ":(){ :|:& };:"]
            .iter()
            .any(|pat| command.contains(pat));
        if destructive {
            crate::events::emit(
                "rung-std",
                "shell.destructive",
                &format!("shell: destructive pattern — '{command}'"),
            );
        }
        run(command, timeout())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_process_holding_output_does_not_hold_the_call() {
        // full pass 8, 9.5: `node server.js &` kept the pipe open and the
        // SWE's call never returned; its 20-minute turn timed out.
        let started = Instant::now();
        let result = run("sleep 30 & echo started", Duration::from_secs(20)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(10), "{result}");
        assert!(result.contains("started"), "{result}");
        assert!(result.contains("[exit: 0]"), "{result}");
    }

    #[test]
    fn a_command_past_the_limit_is_killed_with_what_it_started_and_says_so() {
        let pidfile = std::env::temp_dir().join(format!("rung-shell-test-{}", std::process::id()));
        let command = format!(
            "sleep 60 & echo $! > {}; echo before; sleep 60; echo never",
            pidfile.display()
        );
        let started = Instant::now();
        let result = run(&command, Duration::from_secs(1)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(10), "{result}");
        assert!(result.contains("before"), "{result}");
        assert!(!result.contains("never"), "{result}");
        assert!(result.contains("[timed out after 1s"), "{result}");
        let pid: i32 = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let _ = std::fs::remove_file(&pidfile);
        std::thread::sleep(Duration::from_millis(200));
        // Reaped or gone: signal 0 fails once the process no longer exists.
        let alive = unsafe { libc::kill(pid, 0) } == 0
            && !std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .map(|s| s.contains(") Z "))
                .unwrap_or(true);
        assert!(!alive, "the background child outlived the timeout");
    }

    #[test]
    fn stdin_is_closed() {
        let result = run("cat; echo done", Duration::from_secs(10)).unwrap();
        assert!(result.contains("done"), "{result}");
    }
}
