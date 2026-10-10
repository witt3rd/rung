//! H2: the instance registry and `rung-host ls`. A host registers itself
//! when it starts and the entry stays when it stops; `ls` joins each entry
//! with whether the host still lives and what its record's last line is,
//! so a dead instance is shown as Down and a stopped one as Stopped, never
//! dropped. Against the `rung-host` binary on the real clock with the mock
//! engine; the registry sits under `RUNG_HOME`, a scratch directory.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_rung-host");

struct Fleet {
    root: rung_host::sim::TempDir,
}

impl Fleet {
    fn new(name: &str) -> Self {
        let root = rung_host::sim::temp_dir_guard(name);
        std::fs::create_dir_all(root.path().join("home")).unwrap();
        Fleet { root }
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn sim(&self, state: &str, extra: &[&str]) -> Command {
        let mut c = Command::new(BIN);
        c.args(["sim", "--clock", "real", "--no-memory", "--state"])
            .arg(self.state(state))
            .args(extra)
            .env("RUNG_HOME", self.home())
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        c
    }

    fn start(&self, state: &str, name: &str) -> Child {
        self.sim(state, &["--name", name])
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn rung-host")
    }

    fn ls(&self, args: &[&str]) -> std::process::Output {
        Command::new(BIN)
            .arg("ls")
            .args(args)
            .env("RUNG_HOME", self.home())
            .output()
            .expect("rung-host ls")
    }

    /// `ls --json`, the entries by id.
    fn listed(&self) -> Vec<Value> {
        let out = self.ls(&["--json"]);
        assert!(out.status.success(), "ls failed: {out:?}");
        let v: Value = serde_json::from_slice(&out.stdout).expect("ls --json is JSON");
        v["instances"].as_array().expect("instances array").clone()
    }

    fn word(&self, id: &str) -> Option<String> {
        self.listed()
            .iter()
            .find(|e| e["id"] == id)
            .map(|e| e["word"].as_str().unwrap_or("").to_string())
    }

    fn wait_word(&self, id: &str, word: &str, secs: u64) {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(secs) {
            if self.word(id).as_deref() == Some(word) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("{id} never read {word}; ls says {:?}", self.word(id));
    }
}

fn signal(c: &Child, sig: libc::c_int) {
    // SAFETY: signalling our own child.
    unsafe {
        libc::kill(c.id() as libc::pid_t, sig);
    }
}

#[test]
fn start_registers_stop_keeps_and_a_killed_host_reads_down() {
    rung_host::sim::test_timeout(180);
    let f = Fleet::new("registry-three");
    let mut a = f.start("a", "alpha");
    let mut b = f.start("b", "beta");
    let mut c = f.start("c", "gamma");
    for id in ["alpha", "beta", "gamma"] {
        f.wait_word(id, "running", 60);
    }

    // Each entry is one small file, with every field the design names.
    let listed = f.listed();
    assert_eq!(listed.len(), 3);
    let alpha = listed.iter().find(|e| e["id"] == "alpha").unwrap();
    assert_eq!(alpha["name"], "alpha");
    assert_eq!(alpha["pid"].as_u64(), Some(a.id() as u64));
    assert_eq!(alpha["state_dir"], f.state("a").to_str().unwrap());
    assert!(alpha["workspace"].is_string());
    assert!(alpha["started"].as_i64().unwrap() > 0);
    assert!(alpha["version"].is_string());
    for k in ["config", "address"] {
        assert!(alpha.get(k).is_some(), "{k} is a field, null when absent");
    }
    assert!(f.home().join("instances/alpha.json").is_file());

    // kill -9 reads Down; a clean stop reads Stopped; the third lives on.
    signal(&b, libc::SIGKILL);
    signal(&c, libc::SIGTERM);
    b.wait().unwrap();
    c.wait().unwrap();
    f.wait_word("beta", "down", 30);
    f.wait_word("gamma", "stopped", 30);
    assert_eq!(f.word("alpha").as_deref(), Some("running"));
    assert_eq!(f.listed().len(), 3, "a dead or stopped instance is kept");
    assert!(f.home().join("instances/beta.json").is_file());
    assert!(f.home().join("instances/gamma.json").is_file());

    // The table names all three, each with its word.
    let text = String::from_utf8(f.ls(&[]).stdout).unwrap();
    for (id, word) in [("alpha", "running"), ("beta", "down"), ("gamma", "stopped")] {
        let row = text.lines().find(|l| l.contains(id)).unwrap_or_else(|| {
            panic!("no row for {id} in:\n{text}");
        });
        assert!(row.contains(word), "{id} row lacks {word}: {row}");
    }

    // The killed host's last record line is not a halt; the stopped one's is.
    let beta = f.listed().into_iter().find(|e| e["id"] == "beta").unwrap();
    assert_ne!(beta["last"]["kind"], "halted");
    let gamma = f.listed().into_iter().find(|e| e["id"] == "gamma").unwrap();
    assert_eq!(gamma["last"]["kind"], "halted");

    signal(&a, libc::SIGTERM);
    a.wait().unwrap();
    f.wait_word("alpha", "stopped", 30);
}

#[test]
fn a_restart_keeps_the_entry_and_a_second_live_host_with_the_id_is_refused() {
    rung_host::sim::test_timeout(180);
    let f = Fleet::new("registry-restart");
    let mut first = f.start("a", "alpha");
    f.wait_word("alpha", "running", 60);

    // Same id, other state directory, while the first lives: refused,
    // naming the holder, and the entry is untouched.
    let out = f
        .sim("other", &["--name", "alpha"])
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(2), "bad start; stderr: {err}");
    assert!(
        err.contains("alpha") && err.contains(&format!("pid {}", first.id())),
        "{err}"
    );
    let entry = f.listed().into_iter().find(|e| e["id"] == "alpha").unwrap();
    assert_eq!(entry["state_dir"], f.state("a").to_str().unwrap());

    // Same state under another name: refused; one state, one entry.
    signal(&first, libc::SIGKILL);
    first.wait().unwrap();
    f.wait_word("alpha", "down", 30);
    let out = f
        .sim("a", &["--name", "renamed"])
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(2), "stderr: {err}");
    assert!(
        err.contains("alpha"),
        "names the entry that owns the state: {err}"
    );
    assert_eq!(f.listed().len(), 1);

    // The same name on the same state is a restart: the entry is renewed.
    let mut again = f.start("a", "alpha");
    f.wait_word("alpha", "running", 60);
    // The host takes the lock, then renews the entry: wait for the renewal.
    let t = Instant::now();
    loop {
        let entry = f.listed().into_iter().find(|e| e["id"] == "alpha").unwrap();
        if entry["pid"].as_u64() == Some(again.id() as u64) {
            break;
        }
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "the entry was never renewed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    signal(&again, libc::SIGTERM);
    again.wait().unwrap();
}

#[test]
fn the_real_port_is_written_and_a_broken_entry_is_listed_not_dropped() {
    rung_host::sim::test_timeout(180);
    let f = Fleet::new("registry-port");
    let mut c = f.sim(
        "a",
        &[
            "--name",
            "alpha",
            "--acp-http",
            "127.0.0.1:0",
            "--acp-token-env",
            "owner=H2_TOKEN",
        ],
    );
    c.env("H2_TOKEN", "t").stderr(Stdio::null());
    let mut host = c.spawn().unwrap();
    f.wait_word("alpha", "running", 60);
    let t = Instant::now();
    let address = loop {
        let entry = f.listed().into_iter().find(|e| e["id"] == "alpha").unwrap();
        if let Some(a) = entry["address"].as_str() {
            break a.to_string();
        }
        assert!(t.elapsed() < Duration::from_secs(30), "no address written");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        !address.ends_with(":0"),
        "the real port, not the asked one: {address}"
    );
    let sock = address
        .trim_start_matches("http://")
        .trim_end_matches("/acp");
    std::net::TcpStream::connect(sock).expect("the written address answers");

    // A file that is not an entry is shown with its problem.
    let mut bad = std::fs::File::create(f.home().join("instances/ruined.json")).unwrap();
    bad.write_all(b"{ not json").unwrap();
    let listed = f.listed();
    assert_eq!(listed.len(), 2);
    let ruined = listed.iter().find(|e| e["id"] == "ruined").unwrap();
    assert_eq!(ruined["word"], "unreadable");
    assert!(ruined["problem"].is_string());

    signal(&host, libc::SIGTERM);
    host.wait().unwrap();
}

#[test]
fn run_registers_from_its_config_and_an_empty_registry_lists_nothing() {
    rung_host::sim::test_timeout(180);
    let f = Fleet::new("registry-run");
    let out = f.ls(&[]);
    assert!(out.status.success());
    assert!(f.listed().is_empty());

    let cfg = f.root.path().join("rung-host.yaml");
    let state = f.state("state");
    let workspace = f.state("work");
    std::fs::write(
        &cfg,
        format!(
            "name: Deep Thought\nstate: {}\nworkspace: {}\nengine:\n  kind: mock\nmemory: false\n",
            state.display(),
            workspace.display()
        ),
    )
    .unwrap();
    let status = Command::new(BIN)
        .args(["run", "--config", cfg.to_str().unwrap(), "--turns", "2"])
        .env("RUNG_HOME", f.home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    let e = f.listed().remove(0);
    assert_eq!(e["id"], "deep-thought");
    assert_eq!(e["name"], "Deep Thought");
    assert_eq!(e["config"], cfg.to_str().unwrap());
    assert_eq!(e["state_dir"], state.to_str().unwrap());
    assert_eq!(e["workspace"], workspace.to_str().unwrap());
    // A bounded run that ended on its own halted: Stopped, still listed.
    assert_eq!(e["word"], "stopped");
    let _: &Path = &cfg;
}
