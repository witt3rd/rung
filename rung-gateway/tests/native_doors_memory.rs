//! H3: a page of any size is streamed, with bounded working memory.
//!
//! The record door has no cap on `limit`, so a page can be the whole record.
//! This test gives it one many megabytes long and measures what the process
//! holds at its peak while the page is written and read, with a counting
//! allocator: the peak must stay a small fraction of the page, not a multiple
//! of it. Memory, never a wall clock, is the measure.

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

const LINES: u64 = 500;
const LINE_BYTES: usize = 8_000;

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
async fn a_page_of_the_whole_record_is_streamed_in_bounded_memory() {
    let root = TempDir::new("h3-mem");
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
    // ~4 MB, written a line at a time
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(state.join("record/seg-00000001.ndjson")).unwrap(),
    );
    let words = "an ordinary sentence about notes and builds ".repeat(LINE_BYTES / 44);
    for i in 1..=LINES {
        let l =
            json!({"seq": i, "at": 1_000 + i, "kind": "note.written", "turn": i, "text": words});
        writeln!(f, "{l}").unwrap();
    }
    drop(f);
    let file_bytes = std::fs::metadata(state.join("record/seg-00000001.ndjson"))
        .unwrap()
        .len();
    assert!(file_bytes > 3_900_000, "{file_bytes}");

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
    let base = format!("http://{}/api/i/alpha/v1/record", gw.addr);

    for q in [
        "?limit=18446744073709551615",
        "?limit=18446744073709551615&order=desc",
        "?limit=1000000&after=100",
        "?limit=1000000&order=desc&offset=100",
    ] {
        let before = CUR.load(Ordering::Relaxed);
        PEAK.store(before, Ordering::Relaxed);
        let (status, got) = drain(&format!("{base}{q}")).await;
        let peak = PEAK.load(Ordering::Relaxed).saturating_sub(before);
        eprintln!("{q}: page {got} bytes, peak {peak}");
        assert_eq!(status, 200, "{q}");
        assert!(
            got > file_bytes * 8 / 10,
            "{q}: only {got} bytes of {file_bytes}"
        );
        // a page of ~6 MB held in under 1.5 MB: working memory does not grow with the page
        assert!(
            peak < 1_500_000,
            "{q}: peak {peak} bytes while writing a {got}-byte page"
        );
    }
}
