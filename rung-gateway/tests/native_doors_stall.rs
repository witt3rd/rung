//! H3b: clients that stall do not exhaust the blocking pool.
//!
//! A record page is written by one blocking task that waits whenever its
//! client is not taking chunks. With enough clients that connect and then read
//! nothing, those waits fill every blocking-pool slot and every other request
//! starves behind them. The fix is a deadline on the stall: a client that
//! takes no chunk for the deadline is dropped and its slot is freed, while a
//! client that keeps reading is never cut off, however large the page or long
//! the read. The deadline here is short and injected; nothing else is timed.

use std::io::Write;
use std::time::Duration;

use rung_gateway::{Settings, StaticRegistry, Tuning};
use rung_testkit::TempDir;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// More than the blocking pool's 512 slots.
const STALLED: usize = 520;
/// The injected stall deadline.
const DEADLINE: Duration = Duration::from_millis(1500);
/// ~12 MB: far more than the socket buffers of a client that reads nothing.
const LINES: u64 = 1_500;
const LINE_BYTES: usize = 8_000;

/// The 520-client test saturates the machine: the others wait for it, so
/// their own timing is not starved.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct World {
    _root: TempDir,
    base: String,
    addr: std::net::SocketAddr,
    file_bytes: u64,
}

async fn world(deadline: Duration) -> World {
    let root = TempDir::new("h3b-stall");
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
    let seg = state.join("record/seg-00000001.ndjson");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&seg).unwrap());
    let words = "an ordinary sentence about notes and builds ".repeat(LINE_BYTES / 44);
    for i in 1..=LINES {
        let l =
            json!({"seq": i, "at": 1_000 + i, "kind": "note.written", "turn": i, "text": words});
        writeln!(f, "{l}").unwrap();
    }
    drop(f);
    let file_bytes = std::fs::metadata(&seg).unwrap().len();
    let settings = Settings {
        listen: "127.0.0.1:0".parse().unwrap(),
        app_dir: None,
        allowed_hosts: vec![],
        read_only_tokens: vec![],
        registry_dir: Some(registry),
        instances: vec![],
    };
    let gw = rung_gateway::start_tuned(
        settings,
        std::sync::Arc::new(StaticRegistry(vec![])),
        Tuning {
            stall_deadline: deadline,
            ..Tuning::default()
        },
    )
    .await
    .unwrap();
    let addr = gw.addr;
    std::mem::forget(gw); // runs for the life of the test process
    World {
        _root: root,
        base: format!("http://{addr}/api/i/alpha/v1"),
        addr,
        file_bytes,
    }
}

/// A client that asks for the whole record and then reads nothing.
async fn stalled_client(addr: std::net::SocketAddr) -> tokio::net::TcpStream {
    let sock = tokio::net::TcpSocket::new_v4().unwrap();
    sock.set_recv_buffer_size(4096).unwrap();
    let mut s = sock.connect(addr).await.unwrap();
    let req = format!(
        "GET /api/i/alpha/v1/record?limit=18446744073709551615 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    s
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_readers_do_not_starve_other_requests() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let w = world(DEADLINE).await;
    let mut held = Vec::new();
    for _ in 0..STALLED {
        held.push(stalled_client(w.addr).await);
    }
    // let the requests reach the server before the one that must complete
    tokio::time::sleep(Duration::from_millis(300)).await;

    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let got = tokio::time::timeout(
        Duration::from_secs(30),
        client.get(format!("{}/summary", w.base)).send(),
    )
    .await;
    let r = got
        .expect("a normal request starved behind stalled clients")
        .unwrap();
    assert_eq!(r.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_client_sees_a_cut_page_never_a_complete_looking_one() {
    let _turn = ONE_AT_A_TIME.lock().await;
    let deadline = Duration::from_millis(300);
    let w = world(deadline).await;
    let mut s = stalled_client(w.addr).await;
    // well past the deadline: the page was begun, filled the way to the
    // client, and the client was dropped
    tokio::time::sleep(deadline * 10).await;
    let mut got = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(20), s.read_to_end(&mut got))
        .await
        .expect("a dropped client's connection was not closed");
    assert!(!got.is_empty(), "it was being answered");
    assert!(
        !got.ends_with(b"0\r\n\r\n"),
        "a dropped client got a complete-looking answer"
    );
    assert!((got.len() as u64) < w.file_bytes, "the page was cut short");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_but_live_reader_of_a_large_page_is_never_cut_off() {
    let _turn = ONE_AT_A_TIME.lock().await;
    // a short deadline: the read below takes several times as long as it, in
    // total, but takes a chunk well inside it every time
    let w = world(Duration::from_millis(400)).await;
    let mut s = tokio::net::TcpStream::connect(w.addr).await.unwrap();
    let req = format!(
        "GET /api/i/alpha/v1/record?limit=18446744073709551615 HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        w.addr
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let t0 = std::time::Instant::now();
    let mut got = Vec::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = s.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert!(
        t0.elapsed() > Duration::from_millis(400) * 2,
        "the read must outlast the deadline several times over ({:?})",
        t0.elapsed()
    );
    assert!(got.ends_with(b"0\r\n\r\n"), "the page ended complete");
    assert!(got.len() as u64 > w.file_bytes * 8 / 10);
}
