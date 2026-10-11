//! S1: the gateway's queue write path does not hold a request body.
//!
//! A body is generated as it is sent (never held) over loopback, to the real
//! gateway in this process. Refused on its headers, or on the instance's state
//! (a stopped instance), it is not read; accepted, it is streamed to the host
//! as it arrives. Peak resident memory (`VmHWM`) is read from `/proc`. One test,
//! so the process peak is the gateway's alone.

use std::convert::Infallible;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rung_gateway::{Instance, Settings, StaticRegistry, Tuning};
use rung_host::statelock::StateLock;
use rung_testkit::TempDir;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const OWNER_KEY: &str = "plainownerkeys1mem-0123456789-zzz";
/// The size of the generated body: a gateway that held it would hold this much
/// at least. (The box is shared; the proof is the ratio, not the size.)
const BODY: u64 = 256 << 20;

fn peak_rss() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let kb: u64 = s
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap();
    kb * 1024
}

/// Declare `len` bytes and generate zeros until the gateway closes or all are
/// sent. Returns the status code and the body bytes taken.
async fn post_generated(addr: &str, id: &str, len: u64, extra: &str) -> (u16, u64) {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let head = format!(
        "POST /api/i/{id}/v1/queue HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n{extra}Connection: close\r\n\r\n"
    );
    s.write_all(head.as_bytes()).await.unwrap();
    let chunk = vec![b' '; 64 * 1024];
    let mut sent = 0u64;
    while sent < len {
        let n = chunk.len().min((len - sent) as usize);
        match s.write(&chunk[..n]).await {
            Ok(0) | Err(_) => break,
            Ok(w) => sent += w as u64,
        }
    }
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    while let Ok(Ok(n)) = tokio::time::timeout(Duration::from_secs(30), s.read(&mut tmp)).await {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let status = String::from_utf8_lossy(&buf)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, sent)
}

/// A host's door that drains the body as it comes and counts it.
async fn draining_host() -> (String, Arc<AtomicU64>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let taken = Arc::new(AtomicU64::new(0));
    let t2 = taken.clone();
    tokio::spawn(async move {
        loop {
            let (sock, _) = l.accept().await.unwrap();
            let taken = t2.clone();
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req: Request<hyper::body::Incoming>| {
                    let taken = taken.clone();
                    async move {
                        let mut body = req.into_body();
                        while let Some(Ok(f)) = body.frame().await {
                            if let Some(d) = f.data_ref() {
                                taken.fetch_add(d.len() as u64, Ordering::SeqCst);
                            }
                        }
                        Ok::<_, Infallible>(
                            Response::builder()
                                .status(201)
                                .header("content-type", "application/json")
                                .body(Full::new(Bytes::from(
                                    json!({"id": "k", "repeat": false}).to_string(),
                                )))
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(sock), svc)
                    .await;
            });
        }
    });
    (addr, taken)
}

#[tokio::test]
async fn a_huge_body_is_never_held_by_the_gateway() {
    let root = TempDir::new("s1gw-mem");
    let registry = root.path().join("registry");
    std::fs::create_dir_all(&registry).unwrap();
    let (host_addr, taken) = draining_host().await;
    let mut locks = Vec::new();
    for id in ["alpha", "stopped"] {
        let state = root.path().join(id).join("state");
        std::fs::create_dir_all(state.join("record")).unwrap();
        let entry = json!({"schema": 1, "id": id, "name": id, "state_dir": state, "config": null,
            "workspace": root.path().join(id), "address": host_addr, "pid": 1, "started": 1,
            "version": "0"});
        std::fs::write(registry.join(format!("{id}.json")), entry.to_string()).unwrap();
        let mut f = std::fs::File::create(state.join("record/seg-00000001.ndjson")).unwrap();
        writeln!(
            f,
            "{}",
            json!({"seq": 1, "at": 1, "kind": "host.start", "config": {}})
        )
        .unwrap();
        if id == "alpha" {
            locks.push(StateLock::acquire(&state).unwrap());
        }
    }
    let settings = Settings {
        listen: "127.0.0.1:0".parse().unwrap(),
        app_dir: None,
        allowed_hosts: vec![],
        read_only_tokens: vec!["view-token-mem-0123456789".into()],
        registry_dir: Some(registry),
        instances: vec![Instance {
            id: "alpha".into(),
            name: "alpha".into(),
            url: format!("http://{host_addr}"),
            key: OWNER_KEY.into(),
        }],
    };
    let registry = Arc::new(StaticRegistry(settings.instances.clone()));
    let gw = rung_gateway::start_tuned(settings, registry, Tuning::default())
        .await
        .unwrap();
    let addr = gw.addr.to_string();
    let before = peak_rss();

    // a running instance takes the body, streamed through to the host
    let (status, sent) = post_generated(&addr, "alpha", BODY, "").await;
    assert_eq!(status, 201, "a running instance takes the streamed body");
    assert_eq!(sent, BODY);
    assert_eq!(
        taken.load(Ordering::SeqCst),
        BODY,
        "the host took all of it"
    );

    // refused on the instance's state (not running): the body is not read
    let (status, sent) = post_generated(&addr, "stopped", BODY, "").await;
    assert_eq!(status, 409);
    assert!(
        sent < 64 << 20,
        "the gateway took {sent} bytes of a refused body"
    );

    // refused on its headers: the body is not read
    let (status, sent) = post_generated(
        &addr,
        "alpha",
        BODY,
        "Authorization: Basic Zm9vOmJhcg==\r\n",
    )
    .await;
    assert_eq!(status, 401);
    assert!(
        sent < 64 << 20,
        "the gateway took {sent} bytes of a refused body"
    );
    let ro = "Authorization: Bearer view-token-mem-0123456789\r\n";
    let (status, sent) = post_generated(&addr, "alpha", BODY, ro).await;
    assert_eq!(status, 403);
    assert!(
        sent < 64 << 20,
        "the gateway took {sent} bytes of a refused body"
    );
    let (status, sent) =
        post_generated(&addr, "alpha", BODY, "Origin: http://evil.example\r\n").await;
    assert_eq!(status, 403);
    assert!(
        sent < 64 << 20,
        "the gateway took {sent} bytes of a refused body"
    );

    let grown = peak_rss().saturating_sub(before);
    assert!(
        grown < 96 << 20,
        "the gateway's peak memory grew by {grown} bytes for {BODY}-byte bodies"
    );
}
