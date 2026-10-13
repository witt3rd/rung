//! The ACP HTTP listener streams an admitted request body to a spool as it
//! arrives: a very large body arrives whole (no cap, no truncation) while the
//! process's memory stays flat. One test binary, one process, so the peak RSS
//! (`VmHWM`) is this listener's alone.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use rung_agent_core::acp_http::{
    GateVerdict, RouteBody, RouteGate, RouteReply, Routes, Serve, listen_bound,
};
use sha2::{Digest, Sha256};

const TOKEN: &str = "s3cret";
const BODY: u64 = 256 << 20;
const CHUNK: usize = 1 << 20;

/// The two 256 MiB tests never overlap: peak RSS is the process's, so the
/// `/acp` test (which holds its body whole) must not run inside the flat-memory
/// test's window.
static BIG: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn hwm_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .expect("VmHWM")
}

fn reply(status: u16, body: String) -> RouteReply {
    RouteReply {
        status,
        headers: vec![],
        body: RouteBody::Bytes(body.into_bytes()),
    }
}

async fn serve(spool: PathBuf) -> SocketAddr {
    let dir = spool.clone();
    let routes: Routes = Arc::new(move |req| {
        let dir = dir.clone();
        Box::pin(async move {
            // hash what arrived, streaming it back off the spool
            let mut r = req.body.reader();
            let mut h = Sha256::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = r.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            let in_dir = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|e| std::fs::read_link(e.ok()?.path()).ok())
                .any(|l| l.starts_with(&dir) && l.to_string_lossy().contains("rung-body-"));
            Some(reply(
                200,
                format!("{} {:x} {in_dir}", req.body.len(), h.finalize()),
            ))
        })
    });
    let gate: RouteGate = Arc::new(|head| {
        let ok = head
            .headers
            .iter()
            .any(|(k, v)| k == "authorization" && v == &format!("Bearer {TOKEN}"));
        if ok {
            GateVerdict::Admit
        } else {
            GateVerdict::Refuse(reply(401, "no".into()))
        }
    });
    let serve = Serve {
        name: "test".into(),
        authorize: Arc::new(|_| true),
        connect: Arc::new(|_, _| Box::pin(async {})),
        bind_bearer: false,
        routes: Some(routes),
        route_gate: Some(gate),
        spool_dir: Some(spool),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = listen_bound("127.0.0.1:0".into(), serve, move |a| {
            let _ = tx.send(a);
        })
        .await;
    });
    rx.await.unwrap()
}

fn head(len: u64, auth: bool) -> String {
    head_with(len, auth, "Connection: close\r\n")
}

fn head_with(len: u64, auth: bool, extra: &str) -> String {
    format!(
        "POST /big HTTP/1.1\r\nHost: x\r\nContent-Length: {len}\r\n{extra}{}\r\n",
        if auth {
            format!("Authorization: Bearer {TOKEN}\r\n")
        } else {
            String::new()
        }
    )
}

fn status_and_body(mut s: TcpStream) -> (u16, String) {
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).to_string();
    let status = text.split(' ').nth(1).unwrap().parse().unwrap();
    (
        status,
        text.split("\r\n\r\n").nth(1).unwrap_or("").to_string(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_256_mib_body_arrives_whole_with_flat_memory() {
    let _big = BIG.lock().await;
    let tmp = rung_testkit::TempDir::new("spool-test");
    let spool = tmp.path().to_path_buf();
    let addr = serve(spool.clone()).await;

    // no credential: refused before the body is read (none is ever sent)
    let refused = tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(head(BODY, false).as_bytes()).unwrap();
        status_and_body(s)
    })
    .await
    .unwrap();
    assert_eq!(refused.0, 401);

    let before = hwm_kib();
    let (sent_hash, answer) = tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(head(BODY, true).as_bytes()).unwrap();
        let mut h = Sha256::new();
        let mut chunk = vec![0u8; CHUNK];
        let mut x: u64 = 0x9e3779b97f4a7c15;
        for _ in 0..(BODY as usize / CHUNK) {
            for b in chunk.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *b = x as u8;
            }
            h.update(&chunk);
            s.write_all(&chunk).unwrap();
        }
        let (st, body) = status_and_body(s);
        assert_eq!(st, 200);
        (format!("{:x}", h.finalize()), body)
    })
    .await
    .unwrap();
    assert_eq!(
        answer,
        format!("{BODY} {sent_hash} true"),
        "arrived whole, byte-identical, spooled in the configured dir"
    );

    let grown_mib = (hwm_kib() - before) / 1024;
    assert!(
        grown_mib < 64,
        "peak RSS grew {grown_mib} MiB for a {} MiB body",
        BODY >> 20
    );
}

/// A body past the in-memory threshold is spooled in the configured dir, not
/// `temp_dir()` itself: the handler sees the open spool file under that dir.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_spool_lands_in_the_configured_dir() {
    let tmp = rung_testkit::TempDir::new("state-test");
    let spool = tmp.path().join("spool");
    std::fs::create_dir_all(&spool).unwrap();
    let addr = serve(spool.clone()).await;
    let len: usize = 3 << 20;
    let answer = tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(head(len as u64, true).as_bytes()).unwrap();
        s.write_all(&vec![7u8; len]).unwrap();
        let (st, body) = status_and_body(s);
        assert_eq!(st, 200);
        body
    })
    .await
    .unwrap();
    let mut parts = answer.split(' ');
    assert_eq!(parts.next(), Some(len.to_string().as_str()));
    assert_eq!(
        parts.nth(1),
        Some("true"),
        "spool file open in the configured dir"
    );
}

/// A spool that cannot be written fails the request with a 500 that closes the
/// connection, so the unread rest of the body is not taken for the next request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_spool_write_failure_answers_500_and_closes() {
    let tmp = rung_testkit::TempDir::new("spool-fail-test");
    let addr = serve(tmp.path().join("missing")).await;
    let len: usize = 3 << 20;
    let raw = tokio::task::spawn_blocking(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        // keep-alive: only the server's own `connection: close` can end it
        s.write_all(head_with(len as u64, true, "").as_bytes())
            .unwrap();
        // the server may cut the upload off once it has answered
        let _ = s.write_all(&vec![7u8; len]);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        String::from_utf8_lossy(&out).to_lowercase()
    })
    .await
    .unwrap();
    assert!(raw.starts_with("http/1.1 500"), "{raw}");
    assert!(raw.contains("connection: close"), "{raw}");
}

/// A 256 MiB JSON-RPC body on `/acp` is parsed in time near linear in its size
/// (the parse reads through a buffer, not call by call over the chunk list). It
/// is answered 400 (no connection id) only once parsed whole; unbuffered it
/// took minutes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_256_mib_acp_body_parses_in_near_linear_time() {
    let _big = BIG.lock().await;
    let tmp = rung_testkit::TempDir::new("acp-parse-test");
    let addr = serve(tmp.path().to_path_buf()).await;
    let started = std::time::Instant::now();
    let (st, _) = tokio::task::spawn_blocking(move || {
        let (pre, post) = (
            br#"{"jsonrpc":"2.0","id":1,"method":"session/prompt","params":{"pad":""#.as_slice(),
            br#""}}"#.as_slice(),
        );
        let len = pre.len() as u64 + BODY + post.len() as u64;
        let mut s = TcpStream::connect(addr).unwrap();
        let h = format!(
            "POST /acp HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
        );
        s.write_all(h.as_bytes()).unwrap();
        s.write_all(pre).unwrap();
        let chunk = vec![b'x'; CHUNK];
        for _ in 0..(BODY as usize / CHUNK) {
            s.write_all(&chunk).unwrap();
        }
        s.write_all(post).unwrap();
        status_and_body(s)
    })
    .await
    .unwrap();
    assert_eq!(st, 400);
    let took = started.elapsed().as_secs();
    // linear is a few seconds even unoptimized; the unbuffered parse took ~90 s
    assert!(took < 40, "a 256 MiB /acp body took {took} s");
}
