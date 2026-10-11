//! S4: the context door streams a turn of any size in bounded memory.
//!
//! The door joins the record's layers and the epoch's history, which can be
//! megabytes. This test gives it a history of many turns and measures what the
//! process holds at its peak while the answer is written and read, with a
//! counting allocator: the peak must stay a small fraction of the answer.
//! Memory, never a wall clock, is the measure.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use rung_gateway::Settings;
use rung_testkit::TempDir;
use serde_json::json;

struct Counting;

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(n: usize) {
    let now = CUR.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

// SAFETY: every call is forwarded unchanged to the system allocator; the
// counters only observe the sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        CUR.fetch_sub(l.size(), Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        if new >= l.size() {
            grow(new - l.size());
        } else {
            CUR.fetch_sub(l.size() - new, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

const TURNS: u64 = 300;
const MESSAGE_BYTES: usize = 12_000;

/// Read a response to its end without keeping it; its length.
async fn drain(url: &str) -> (u16, u64) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut r = client.get(url).send().await.unwrap();
    let status = r.status().as_u16();
    let mut n = 0u64;
    while let Some(c) = r.chunk().await.unwrap() {
        n += c.len() as u64;
    }
    (status, n)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_long_history_is_streamed_in_bounded_memory() {
    let root = TempDir::new("s4-mem");
    let registry = root.path().join("registry");
    let state = root.path().join("alpha/state");
    std::fs::create_dir_all(&registry).unwrap();
    std::fs::create_dir_all(state.join("record")).unwrap();
    std::fs::write(
        registry.join("alpha.json"),
        json!({"schema": 1, "id": "alpha", "name": "alpha", "state_dir": state,
               "config": null, "workspace": root.path().join("w"), "address": null,
               "pid": 1, "started": 1, "version": "0.2.3"})
        .to_string(),
    )
    .unwrap();
    let path = state.join("record/seg-00000001.ndjson");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    let mut n = 0u64;
    let mut put = |kind: &str, body: serde_json::Value| {
        n += 1;
        let mut m = body.as_object().cloned().unwrap();
        m.insert("seq".into(), n.into());
        m.insert("at".into(), (1_000 + n).into());
        m.insert("kind".into(), kind.into());
        writeln!(f, "{}", serde_json::Value::Object(m)).unwrap();
    };
    put(
        "pack.layers",
        json!({"cause": "start", "s_hash": "s1", "tools": [{"name": "t"}], "system": "sys"}),
    );
    put(
        "epoch.rollover",
        json!({"from": 0, "to": 1, "l_hash": "l1", "slow": "epoch", "first_turn": 1}),
    );
    let words = "an ordinary sentence about notes and builds ".repeat(MESSAGE_BYTES / 44);
    for t in 1..=TURNS + 1 {
        put("turn.started", json!({"turn": t, "epoch": 1}));
        put(
            "llm.call",
            json!({"turn": t, "call": 1, "epoch": 1, "prompt_tokens": 100, "cached_tokens": 50,
                   "prefix": {"s_hash": "s1", "l_hash": "l1"}}),
        );
        put(
            "turn.log",
            json!({"turn": t, "header": "h", "recall_text": null, "recall_hash": null, "recall": false,
                   "messages": [{"role": "assistant", "content": words},
                                {"role": "user", "content": words},
                                {"role": "assistant", "content": "done"}]}),
        );
        put("turn.ended", json!({"turn": t, "status": "completed"}));
    }
    drop(put);
    drop(f);
    let file_bytes = std::fs::metadata(&path).unwrap().len();
    assert!(file_bytes > 7_000_000, "{file_bytes}");

    let gw = rung_gateway::start(Settings {
        listen: "127.0.0.1:0".parse().unwrap(),
        app_dir: None,
        allowed_hosts: vec![],
        read_only_tokens: vec![],
        registry_dir: Some(registry),
        instances: vec![],
    })
    .await
    .unwrap();
    let url = format!(
        "http://{}/api/i/alpha/v1/turns/{}/context",
        gw.addr,
        TURNS + 1
    );
    let before = CUR.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let (status, got) = drain(&url).await;
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(before);
    eprintln!("context: answer {got} bytes, peak {peak}");
    assert_eq!(status, 200);
    assert!(
        got > file_bytes * 9 / 10,
        "only {got} bytes of {file_bytes}"
    );
    // an answer of ~7 MB held in a small fraction of it
    assert!(
        peak < 2_000_000,
        "peak {peak} bytes while writing a {got}-byte answer"
    );
}
