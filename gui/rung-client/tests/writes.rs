//! The owner's calls against an in-process stub: what each one sends (method,
//! path, body, bearer) and how a refusal reads.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use rung_client::{Client, Error, Intent};

/// What the stub saw: method, path, content-type, authorization, body.
type Seen = Arc<Mutex<Vec<(String, String, Option<String>, Option<String>, String)>>>;

/// Answer every request with `status` and `body`.
fn stub(status: u16, body: &'static str) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut reader = BufReader::new(conn.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut parts = line.split_whitespace();
            let (method, path) = (
                parts.next().unwrap_or("").to_string(),
                parts.next().unwrap_or("").to_string(),
            );
            let (mut len, mut ctype, mut auth) = (0usize, None, None);
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    let v = v.trim().to_string();
                    match k.to_ascii_lowercase().as_str() {
                        "content-length" => len = v.parse().unwrap_or(0),
                        "content-type" => ctype = Some(v),
                        "authorization" => auth = Some(v),
                        _ => {}
                    }
                }
            }
            let mut b = vec![0u8; len];
            reader.read_exact(&mut b).unwrap();
            log.lock()
                .unwrap()
                .push((method, path, ctype, auth, String::from_utf8(b).unwrap()));
            let mut conn = conn;
            let _ = write!(
                conn,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (base, seen)
}

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn each_call_sends_the_documented_request() {
    let (base, seen) = stub(
        200,
        r#"{"id":"k-1","repeat":false,"moved":true,"withdrawn":true,"stopping":true,"released":true}"#,
    );
    let c = Client::new(&base).unwrap();
    assert_eq!(
        c.queue_add("chief-a", "hi \"there\"", Intent::Steer, "k-1")
            .unwrap()
            .id,
        "k-1"
    );
    assert!(
        c.queue_move("chief-a", "an item/1", Some("q-2"), "mv-1")
            .unwrap()
            .moved
    );
    assert!(c.queue_move("chief-a", "q-3", None, "mv-2").unwrap().moved);
    assert!(c.queue_cancel("chief-a", "q 4").unwrap().withdrawn);
    assert!(c.stop("chief-a").unwrap().stopping);
    assert!(c.release("chief-a", Some("done here")).unwrap().released);
    assert!(c.release("chief-a", None).unwrap().released);

    let s = seen.lock().unwrap();
    let row = |i: usize| (s[i].0.as_str(), s[i].1.as_str());
    assert_eq!(row(0), ("POST", "/api/i/chief-a/v1/queue"));
    assert_eq!(s[0].2.as_deref(), Some("application/json"));
    assert_eq!(
        json(&s[0].4),
        json(r#"{"text":"hi \"there\"","key":"k-1","intent":"steer"}"#)
    );
    assert_eq!(
        row(1),
        ("POST", "/api/i/chief-a/v1/queue/an%20item%2F1/move")
    );
    assert_eq!(json(&s[1].4), json(r#"{"before":"q-2","key":"mv-1"}"#));
    assert_eq!(
        json(&s[2].4),
        json(r#"{"before":null,"key":"mv-2"}"#),
        "the end of its group is null"
    );
    assert_eq!(row(3), ("DELETE", "/api/i/chief-a/v1/queue/q%204"));
    assert_eq!(s[3].4, "");
    assert_eq!(row(4), ("POST", "/api/i/chief-a/v1/stop"));
    assert_eq!(s[4].4, "", "a stop has no body");
    assert_eq!(row(5), ("POST", "/api/i/chief-a/v1/release"));
    assert_eq!(json(&s[5].4), json(r#"{"reason":"done here"}"#));
    assert_eq!(s[6].4, "");
    // the owner is a client with no token: no bearer goes out
    assert!(s.iter().all(|r| r.3.is_none()));
}

#[test]
fn a_token_goes_as_a_bearer_and_a_refusal_reads_in_the_gateways_words() {
    let (base, seen) = stub(
        409,
        r#"{"error":"not_running","message":"the host is stopping: start it first"}"#,
    );
    let c = Client::new(&base).unwrap().with_token("some-token-77");
    let e = c
        .queue_add("chief-a", "x", Intent::Next, "k-2")
        .unwrap_err();
    assert_eq!(
        e,
        Error::Status {
            code: 409,
            message: "the host is stopping: start it first".into()
        }
    );
    assert_eq!(
        seen.lock().unwrap()[0].3.as_deref(),
        Some("Bearer some-token-77")
    );
    assert!(!e.to_string().contains("some-token-77"));
}

#[test]
fn a_bad_instance_id_sends_nothing() {
    let (base, seen) = stub(200, "{}");
    let c = Client::new(&base).unwrap();
    assert!(matches!(c.stop("a/b"), Err(Error::BadId(_))));
    assert!(matches!(c.queue("", None), Err(Error::BadId(_))));
    assert!(matches!(c.queue_cancel("..%2f", "x"), Err(Error::BadId(_))));
    assert!(seen.lock().unwrap().is_empty());
}
