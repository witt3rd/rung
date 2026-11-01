//! The client against the real `rung-gateway` binary in front of two real
//! `rung-host` processes (a chief and a steward, mock engine: no network, no
//! key), with the read-only role.
//!
//! The binaries come from the main workspace, which this one does not build:
//! build them first (`cargo build -p rung-gateway -p rung-host` at the
//! repository root) or point `RUNG_BIN_DIR` at a directory that holds them.
//! A missing binary fails the test with that instruction; it never skips.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rung_client::{Client, Error};

const TOKEN_ENV: &str = "RUNG_CLIENT_TEST_VIEW_TOKEN";
const TOKEN: &str = "view-token-canary-client-4417";
const GUARD: Duration = Duration::from_secs(30);

fn bin(name: &str) -> PathBuf {
    let dir = std::env::var_os("RUNG_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
    let p = dir.join(name);
    assert!(
        p.exists(),
        "{} is missing: run `cargo build -p rung-gateway -p rung-host` at the repository root, or set RUNG_BIN_DIR",
        p.display()
    );
    p
}

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    /// Unique per call: the tests of this file run side by side.
    fn new() -> Scratch {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rung-client-gw-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A child this test started; killed on drop.
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_host(root: &Path, home: &Path, name: &str) -> Proc {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("inbox")).unwrap();
    std::fs::create_dir_all(dir.join("workspace")).unwrap();
    let cfg = dir.join("rung-host.yaml");
    std::fs::write(
        &cfg,
        format!(
            "name: {name}\nstate: {d}/state\nworkspace: {d}/workspace\n\
             inbox: {d}/inbox\nstop_file: {d}/STOP\nengine:\n  kind: mock\nmemory: false\nidle_time_s: 0\n",
            d = dir.display()
        ),
    )
    .unwrap();
    Proc(
        Command::new(bin("rung-host"))
            .args(["run", "--config"])
            .arg(&cfg)
            .env("RUNG_HOME", home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start rung-host"),
    )
}

/// Start the gateway on an OS-chosen port; read the address it prints.
fn start_gateway(root: &Path, home: &Path) -> (Proc, String) {
    let cfg = root.join("gateway.yaml");
    std::fs::write(
        &cfg,
        format!(
            "listen: 127.0.0.1:0\nregistry_dir: {}\nread_only_token_envs: [{TOKEN_ENV}]\n",
            home.join("instances").display()
        ),
    )
    .unwrap();
    let mut child = Command::new(bin("rung-gateway"))
        .args(["--config"])
        .arg(&cfg)
        .env(TOKEN_ENV, TOKEN)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start rung-gateway");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or_else(|| panic!("the gateway printed no address: {line:?}"))
        .to_string();
    (Proc(child), format!("http://{addr}"))
}

#[test]
fn a_chief_and_a_steward_behind_a_real_gateway() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let _chief = start_host(&scratch.0, &home, "chief-main");
    let _steward = start_host(&scratch.0, &home, "steward-alpha");
    let (_gw, base) = start_gateway(&scratch.0, &home);

    let client = Client::new(&base).unwrap().with_token(TOKEN);
    assert!(client.health().expect("health").ok);

    // the hosts register themselves as they start: wait until both read
    let end = Instant::now() + GUARD;
    let list = loop {
        let list = client.instances().expect("the list");
        if list.instances.len() == 2 && list.instances.iter().all(|i| i.summary.is_some()) {
            break list;
        }
        assert!(
            Instant::now() < end,
            "the hosts never showed in the list: {list:?}"
        );
        std::thread::sleep(Duration::from_millis(200));
    };

    assert_eq!(list.registry_error, None);
    let ids: Vec<&str> = list.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["chief-main", "steward-alpha"], "ordered by id");
    for i in &list.instances {
        assert_eq!(i.word.as_deref(), Some("running"), "{}", i.id);
        assert_eq!(i.source.as_deref(), Some("registry"));
        assert!(i.reachable);
        // the words are the host's own, not the client's
        let s = i.summary.as_ref().unwrap();
        assert_eq!(i.state_word(), s.state.as_deref().unwrap());
        assert!(!i.state_word().is_empty());
        assert!(!i.doing().is_empty(), "{}: a host says what it does", i.id);
    }
}

#[test]
fn a_wrong_token_is_refused_in_the_gateways_words() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let (_gw, base) = start_gateway(&scratch.0, &home);
    // no token reads as the owner (the tailnet is the wall); a wrong one does not
    assert!(Client::new(&base).unwrap().health().unwrap().ok);
    let e = Client::new(&base)
        .unwrap()
        .with_token("not-the-token")
        .health()
        .unwrap_err();
    assert!(
        matches!(
            e,
            Error::Status {
                code: 401 | 403,
                ..
            }
        ),
        "{e}"
    );
    assert!(!e.to_string().contains("not-the-token"));
}
