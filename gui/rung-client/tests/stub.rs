//! The client against an in-process stub gateway: what it sends (the bearer),
//! what it reads (both row kinds), and how each failure reads.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use rung_client::{Client, Error};

/// What the stub saw: request line path and `authorization` header.
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// Serve `routes` (path, status, body) on a loopback port until the process
/// ends; every other path is a 404.
fn stub(routes: Vec<(&'static str, u16, &'static str)>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let (routes, log) = (routes.clone(), log.clone());
            thread::spawn(move || {
                let mut reader = BufReader::new(conn.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut auth = None;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':')
                        && k.eq_ignore_ascii_case("authorization")
                    {
                        auth = Some(v.trim().to_string());
                    }
                }
                log.lock().unwrap().push((path.clone(), auth));
                let (status, body) = routes
                    .iter()
                    .find(|(p, _, _)| *p == path)
                    .map(|(_, s, b)| (*s, *b))
                    .unwrap_or((404, r#"{"error":{"message":"no such door"}}"#));
                let mut conn = conn;
                let _ = write!(
                    conn,
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    (base, seen)
}

const LIST: &str = r#"{"instances":[
  {"id":"chief-main","name":"chief-main","word":"running","source":"registry","reachable":true,
   "summary":{"state":"Working","doing":"Committed to p: t","needs_you":false}},
  {"id":"steward-a","name":"steward-a","word":"down","source":"registry","reachable":true,
   "summary":{"state":"Down","doing":"Not running; no record","needs_you":true}},
  {"id":"far","name":"Far","source":"config","reachable":false,"summary":null,"error":"the instance did not answer in time"}
 ],"registry_error":null}"#;

#[test]
fn health_and_the_list_are_read() {
    let (base, _) = stub(vec![
        ("/api/health", 200, r#"{"ok":true}"#),
        ("/api/instances", 200, LIST),
    ]);
    let c = Client::new(&format!("{base}/")).unwrap();
    assert!(c.health().unwrap().ok);

    let list = c.instances().unwrap();
    assert_eq!(list.registry_error, None);
    let rows: Vec<_> = list
        .instances
        .iter()
        .map(|i| (i.display_name(), i.state_word(), i.doing(), i.needs_you()))
        .collect();
    assert_eq!(
        rows,
        [
            ("chief-main", "Working", "Committed to p: t", false),
            ("steward-a", "Down", "Not running; no record", true),
            (
                "Far",
                "unreachable",
                "the instance did not answer in time",
                false
            ),
        ]
    );
}

#[test]
fn the_token_rides_as_a_bearer_and_only_then() {
    let (base, seen) = stub(vec![("/api/health", 200, r#"{"ok":true}"#)]);
    Client::new(&base).unwrap().health().unwrap();
    Client::new(&base)
        .unwrap()
        .with_token("view-token-1")
        .health()
        .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].1, None);
    assert_eq!(seen[1].1.as_deref(), Some("Bearer view-token-1"));
}

#[test]
fn an_error_status_carries_the_gateways_words_not_the_token() {
    let (base, _) = stub(vec![(
        "/api/instances",
        401,
        r#"{"error":{"code":"unauthorized","message":"that token is not accepted"}}"#,
    )]);
    let c = Client::new(&base).unwrap().with_token("view-token-2");
    let e = c.instances().unwrap_err();
    assert_eq!(
        e,
        Error::Status {
            code: 401,
            message: "that token is not accepted".into()
        }
    );
    assert!(!e.to_string().contains("view-token-2"));
    // an unknown door: the stub's 404
    assert!(matches!(c.health(), Err(Error::Status { code: 404, .. })));
}

#[test]
fn a_wrong_shape_is_a_decode_error() {
    let (base, _) = stub(vec![("/api/instances", 200, r#"{"instances":"many"}"#)]);
    assert!(matches!(
        Client::new(&base).unwrap().instances(),
        Err(Error::Decode(_))
    ));
}

#[test]
fn nothing_listening_is_a_transport_error() {
    // bind, learn the port, close: nothing listens there now
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let e = Client::new(&format!("http://127.0.0.1:{port}"))
        .unwrap()
        .with_token("view-token-3")
        .health()
        .unwrap_err();
    assert!(matches!(e, Error::Transport(_)), "{e}");
    assert!(!e.to_string().contains("view-token-3"));
}
