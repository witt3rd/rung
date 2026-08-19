//! Live CPython child. The verb (write JSON, read JSON) lives on strike.

use super::jail;
use super::{SandboxConfig, SandboxError, StrikeReply, StrikeRequest};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::mpsc;
use std::time::Duration;

pub struct Guest {
    config: SandboxConfig,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_id: u64,
}

impl std::fmt::Debug for Guest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Guest")
            .field("store", &self.config.store)
            .field("alive", &self.child.is_some())
            .finish()
    }
}

impl Guest {
    pub fn open(config: SandboxConfig) -> Result<Self, SandboxError> {
        if !config.hammer.is_file() {
            return Err(SandboxError::MissingHammer(config.hammer.clone()));
        }
        std::fs::create_dir_all(&config.store).map_err(SandboxError::Io)?;
        let mut guest = Self {
            config,
            child: None,
            stdin: None,
            stdout: None,
            next_id: 1,
        };
        guest.ensure()?;
        Ok(guest)
    }

    pub fn store(&self) -> &PathBuf {
        &self.config.store
    }

    pub fn strike(&mut self, code: &str) -> Result<StrikeReply, SandboxError> {
        let id = self.next_id();
        self.request(StrikeRequest::strike(id, code))
    }

    pub fn reset(&mut self) -> Result<StrikeReply, SandboxError> {
        let id = self.next_id();
        self.request(StrikeRequest::reset(id))
    }

    pub fn ping(&mut self) -> Result<StrikeReply, SandboxError> {
        let id = self.next_id();
        self.request(StrikeRequest::ping(id))
    }

    fn next_id(&mut self) -> String {
        let id = self.next_id;
        self.next_id += 1;
        id.to_string()
    }

    fn request(&mut self, req: StrikeRequest) -> Result<StrikeReply, SandboxError> {
        self.ensure()?;
        match self.write_read(&req) {
            Ok(reply) => Ok(reply),
            Err(err) if err.is_retryable() => {
                self.reap();
                self.ensure()?;
                self.write_read(&req)
            }
            Err(err) => Err(err),
        }
    }

    fn write_read(&mut self, req: &StrikeRequest) -> Result<StrikeReply, SandboxError> {
        let stdin = self.stdin.as_mut().ok_or(SandboxError::GuestDead)?;
        let line = serde_json::to_string(req).expect("request is always valid JSON");
        stdin
            .write_all(line.as_bytes())
            .map_err(|_| SandboxError::GuestDead)?;
        stdin.write_all(b"\n").map_err(|_| SandboxError::GuestDead)?;
        stdin.flush().map_err(|_| SandboxError::GuestDead)?;

        let mut stdout = self.stdout.take().ok_or(SandboxError::GuestDead)?;
        let timeout = self.config.strike_timeout;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let res = stdout.read_line(&mut line).map(|_| line);
            let _ = tx.send((res, stdout));
        });
        match rx.recv_timeout(timeout) {
            Ok((Ok(line), stdout)) => {
                self.stdout = Some(stdout);
                if line.is_empty() {
                    return Err(SandboxError::GuestDead);
                }
                serde_json::from_str(line.trim()).map_err(|e| SandboxError::BadReply(e.to_string()))
            }
            Ok((Err(_), stdout)) => {
                self.stdout = Some(stdout);
                Err(SandboxError::GuestDead)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.reap();
                Err(SandboxError::Timeout {
                    elapsed_secs: timeout.as_secs(),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(SandboxError::GuestDead),
        }
    }

    fn ensure(&mut self) -> Result<(), SandboxError> {
        if self.alive() {
            return Ok(());
        }
        self.reap();
        let mut child = jail::spawn(&self.config)?;
        let stdin = child.stdin.take().ok_or(SandboxError::GuestDead)?;
        let stdout = child.stdout.take().ok_or(SandboxError::GuestDead)?;
        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.child = Some(child);
        let ready = StrikeRequest::ping("0");
        let _pong = self.write_read(&ready)?;
        Ok(())
    }

    fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(child) => child.try_wait().ok().flatten().is_none(),
            None => false,
        }
    }

    fn reap(&mut self) {
        self.stdin = None;
        self.stdout = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        if self.alive() {
            if let Some(stdin) = self.stdin.as_mut() {
                let line =
                    serde_json::to_string(&StrikeRequest::shutdown("0")).unwrap_or_default();
                let _ = writeln!(stdin, "{line}");
                let _ = stdin.flush();
            }
            if let Some(child) = &mut self.child {
                let deadline = std::time::Instant::now() + Duration::from_millis(200);
                while std::time::Instant::now() < deadline {
                    if child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        self.reap();
    }
}
