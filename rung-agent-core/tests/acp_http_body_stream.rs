//! The ACP HTTP listener streams an admitted request body to a spool as it
//! arrives: a very large body arrives whole (no cap, no truncation) while the
//! process's memory stays flat. One test binary, one process, so the peak RSS
//! (`VmHWM`) is this listener's alone.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use rung_agent_core::acp_http::{
    GateVerdict, RouteBody, RouteGate, RouteReply, Routes, Serve, listen_bound,
};
use sha2::{Digest, Sha256};

const TOKEN: &str = "s3cret";
const BODY: u64 = 256 << 20;
const CHUNK: usize = 1 << 20;

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

async fn serve() -> SocketAddr {
    let routes: Routes = Arc::new(|req| {
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
            Some(reply(200, format!("{} {:x}", req.body.len(), h.finalize())))
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
    format!(
        "POST /big HTTP/1.1\r\nHost: x\r\nContent-Length: {len}\r\nConnection: close\r\n{}\r\n",
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
    let addr = serve().await;

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
        format!("{BODY} {sent_hash}"),
        "arrived whole, byte-identical"
    );

    let grown_mib = (hwm_kib() - before) / 1024;
    assert!(
        grown_mib < 64,
        "peak RSS grew {grown_mib} MiB for a {} MiB body",
        BODY >> 20
    );
}
