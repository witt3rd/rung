//! webfetch returns the whole body, streamed: no cut, flat memory.
//! One test fn so VmHWM (process high-water RSS) is unpolluted.

use rung_std::tools::fetch_unguarded;
use std::hash::{DefaultHasher, Hasher};
use std::io::{Read, Write};
use std::net::TcpListener;

fn body(n: usize) -> Vec<u8> {
    // deterministic, non-repeating-ish ascii, lines so whitespace is kept
    let mut v = Vec::with_capacity(n);
    let mut x: u64 = 0x9E3779B97F4A7C15;
    while v.len() < n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.extend_from_slice(format!("{x:016x}\n").as_bytes());
    }
    v.truncate(n);
    v
}

fn hash(b: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    h.write(b);
    h.finish()
}

fn serve(n: usize) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut req = [0u8; 4096];
        let _ = s.read(&mut req);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n"
        );
        s.write_all(head.as_bytes()).unwrap();
        let chunk = body(1 << 20);
        let mut left = n;
        // stream from a small reused buffer so the server adds no RSS growth
        while left > 0 {
            let k = left.min(chunk.len());
            s.write_all(&chunk[..k]).unwrap();
            left -= k;
        }
    });
    format!("http://127.0.0.1:{port}/")
}

fn expected(n: usize) -> Vec<u8> {
    let chunk = body(1 << 20);
    let mut v = Vec::with_capacity(n);
    while v.len() < n {
        let k = (n - v.len()).min(chunk.len());
        v.extend_from_slice(&chunk[..k]);
    }
    v
}

fn hwm_kib() -> usize {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM:")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse().unwrap()
}

fn check(n: usize) -> usize {
    let want = expected(n);
    let (want_len, want_hash) = (want.len(), hash(&want));
    drop(want);
    let url = serve(n);
    let out = fetch_unguarded(&url).unwrap();
    let text = out.split_once("\n\n").unwrap().1;
    assert_eq!(text.len(), want_len, "whole body, no cut");
    assert_eq!(hash(text.as_bytes()), want_hash);
    assert!(!text.contains("[truncated]"));
    hwm_kib()
}

#[test]
fn whole_body_and_flat_memory() {
    const MIB: usize = 1 << 20;
    check(5 * MIB);
    let base = hwm_kib();
    let n = 50 * MIB;
    let peak = check(n);
    let grown = (peak.saturating_sub(base)) * 1024;
    // The returned String is the only copy of the body: growth stays within
    // ~1.5x body (amortised Vec growth); double buffering would exceed 2x.
    assert!(
        grown < n + n / 2 + 8 * MIB,
        "peak RSS grew {} MiB for a {} MiB body",
        grown / MIB,
        n / MIB
    );
}
