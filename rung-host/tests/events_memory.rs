//! H5: resuming clients hold memory for what they have not yet consumed, not
//! for what they have.
//!
//! A hundred clients replay one record from its first line at once, each at
//! its own pace. With a counting allocator, the most the process ever holds
//! beyond where it began must stay a small multiple of one client's working
//! set (a read chunk and a line), not a hundred copies of the record. Memory
//! is the measure, never a wall clock.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use rung_host::redact::Redactor;
use rung_host::stream::{Events, Frame, Options};
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

const LINES: u64 = 2_000;
const CLIENTS: usize = 100;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hundred_resumers_hold_memory_for_what_they_have_not_consumed() {
    let dir = TempDir::new("h5-mem");
    let rec = dir.path().join("record");
    std::fs::create_dir_all(&rec).unwrap();
    let mut f = std::fs::File::create(rec.join("seg-00000001.ndjson")).unwrap();
    for n in 1..=LINES {
        let l = json!({"seq": n, "at": 1000 + n, "kind": "note.written", "text": format!("line {n:06} {}", "x".repeat(500))});
        writeln!(f, "{l}").unwrap();
    }
    drop(f);
    let size = std::fs::metadata(rec.join("seg-00000001.ndjson"))
        .unwrap()
        .len() as usize;
    let state = dir.path().to_path_buf();

    let base = CUR.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let mut tasks = Vec::new();
    for _ in 0..CLIENTS {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
            let mut ev = Events::open(&state, Options::new(Redactor::new()))
                .await
                .unwrap();
            let mut n = 0u64;
            while let Some(fr) = ev.next().await {
                if matches!(fr, Frame::Record(_)) {
                    n += 1;
                }
            }
            n
        }));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap(), LINES);
    }
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    // a client's working set is a chunk and a line or two; copies of the
    // record would be CLIENTS * size
    let bound = CLIENTS * 512 * 1024;
    assert!(
        peak < bound,
        "{CLIENTS} resumers peaked at {peak} bytes over a {size}-byte record (bound {bound}; \
         a copy of the record each would be {})",
        CLIENTS * size
    );
    assert!(
        peak < CLIENTS * size / 4,
        "memory grew with the record, not with what is unread"
    );
}
