//! H1: a state directory has one live host. A second host started on the
//! same state is refused, with the bad-start exit code, naming the holder,
//! and writes nothing into the record. Against the `rung-host` binary on
//! the real clock with the mock engine.

use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rung_host::record::Record;

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

fn sim(state: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.args(["sim", "--clock", "real", "--no-memory", "--state"])
        .arg(state);
    c
}

/// A spawned host that dies with its owner: dropped on a pass, a failed
/// assertion or a panic, it never outlives the test. Only this child is
/// killed, by its own handle.
struct Host(Child);

impl Host {
    fn spawn(c: &mut Command) -> Host {
        Host(c.spawn().unwrap())
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // Kill by this child's own handle, then reap it. Both are harmless
        // when the test already stopped and waited on it.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Deref for Host {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for Host {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

fn lines(state: &Path) -> Vec<rung_host::record::Line> {
    Record::read_dir(state.join("record")).unwrap_or_default()
}

fn wait_started(state: &Path) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(60) {
        if lines(state).iter().any(|l| l.kind == "host.start") {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the first host never started");
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only probes for existence.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

fn term(c: &mut Child) -> Option<i32> {
    // SAFETY: signalling our own child.
    unsafe {
        libc::kill(c.id() as libc::pid_t, libc::SIGTERM);
    }
    c.wait().unwrap().code()
}

#[test]
fn a_host_is_killed_when_its_test_panics() {
    let guard = rung_host::sim::temp_dir_guard("state-lock-guard");
    let dir = guard.path().to_path_buf();
    let pid = std::sync::atomic::AtomicU32::new(0);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let first = Host::spawn(sim(&dir).stdout(Stdio::null()));
        pid.store(first.id(), std::sync::atomic::Ordering::SeqCst);
        wait_started(&dir);
        panic!("a failing test");
    }));
    assert!(r.is_err());
    let pid = pid.load(std::sync::atomic::Ordering::SeqCst);
    assert_ne!(pid, 0);
    // Reaped by the guard, so it is gone, not a zombie.
    let t = Instant::now();
    while alive(pid) && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let gone = !alive(pid);
    if !gone {
        // Do not leak the red run's host: kill only this pid.
        // SAFETY: our own child from this test.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(gone, "the host outlived its panicking test");
}

#[test]
fn a_second_host_on_the_same_state_is_refused_and_names_the_holder() {
    let guard = rung_host::sim::temp_dir_guard("state-lock");
    let dir = guard.path().to_path_buf();
    let mut first = Host::spawn(sim(&dir).stdout(Stdio::null()));
    wait_started(&dir);
    let before = lines(&dir).len();

    // Bounded: a second host that is not refused would run for ever.
    let mut child = Host::spawn(sim(&dir).stdout(Stdio::null()).stderr(Stdio::piped()));
    let t = Instant::now();
    let refused = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if t.elapsed() > Duration::from_secs(20) {
            child.kill().unwrap();
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    let status = child.wait().unwrap();
    if !refused {
        panic!("the second host was not refused; it ran; stderr: {err}");
    }
    assert_eq!(status.code(), Some(2), "bad-start code; stderr: {err}");
    assert!(
        err.contains(&format!("pid {}", first.id())),
        "the refusal names the holder; stderr: {err}"
    );
    assert!(
        err.contains(dir.to_str().unwrap()),
        "the refusal names the state directory; stderr: {err}"
    );

    // The first host is untouched and the second wrote nothing.
    assert_eq!(term(&mut first), Some(0));
    let all = lines(&dir);
    assert!(all.len() >= before);
    assert_eq!(
        all.iter().filter(|l| l.kind == "host.start").count(),
        1,
        "only one host ever woke on this state"
    );
    for (i, l) in all.iter().enumerate() {
        assert_eq!(l.seq, i as u64 + 1, "the record stays gapless");
    }
    assert_eq!(all.last().unwrap().kind, "halted");
}

#[test]
fn the_lock_dies_with_the_process() {
    let guard = rung_host::sim::temp_dir_guard("state-lock-kill");
    let dir = guard.path().to_path_buf();
    let mut first = Host::spawn(sim(&dir).stdout(Stdio::null()));
    wait_started(&dir);
    first.kill().unwrap();
    first.wait().unwrap();

    // A killed host leaves no lock behind: the next start wakes on it.
    let mut again = Host::spawn(sim(&dir).stdout(Stdio::null()));
    let t = Instant::now();
    while lines(&dir)
        .iter()
        .filter(|l| l.kind == "host.start")
        .count()
        < 2
    {
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "the restart never woke"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(term(&mut again), Some(0));
}

#[test]
fn the_configured_start_is_refused_on_a_held_state_too() {
    let guard = rung_host::sim::temp_dir_guard("state-lock-run");
    let dir = guard.path().join("state");
    // This process is the live holder.
    let held = rung_host::statelock::StateLock::acquire(&dir).unwrap();
    let cfg = guard.path().join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "state: {}\nengine:\n  kind: mock\nmemory: false\n",
            dir.display()
        ),
    )
    .unwrap();
    let out = Command::new(BIN)
        .args(["run", "--config"])
        .arg(&cfg)
        .args(["--turns", "1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(2), "stderr: {err}");
    assert!(
        err.contains(&format!("pid {}", std::process::id())),
        "{err}"
    );
    assert!(!dir.join("record").exists(), "nothing was written: {err}");
    assert!(!dir.join("memory").exists());
    drop(held);
}
