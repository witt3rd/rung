//! `follow` against a scripted in-process stub: resume from the last record
//! seen, back off while the gateway is away, start over on a cursor from
//! another instance, and stop when told.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rung_client::{Client, Event, Follow, Frame};

/// One scripted answer: a status and the body to send before closing.
type Script = Vec<(u16, String)>;

/// The `Last-Event-ID` each connection sent, in order.
type Seen = Arc<Mutex<Vec<Option<String>>>>;

fn stub(script: Script) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let log = seen.clone();
    thread::spawn(move || {
        let mut script = script.into_iter();
        for conn in listener.incoming().flatten() {
            let mut reader = BufReader::new(conn.try_clone().unwrap());
            let mut last = None;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':')
                    && k.eq_ignore_ascii_case("last-event-id")
                {
                    last = Some(v.trim().to_string());
                }
            }
            log.lock().unwrap().push(last);
            // past the script the stub answers 503: the gateway is away
            let (status, body) = script.next().unwrap_or((503, String::new()));
            let mut conn = conn;
            let _ = write!(
                conn,
                "HTTP/1.1 {status} X\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{body}"
            );
        }
    });
    (base, seen)
}

fn record(n: u64) -> String {
    format!("event: record\nid: {n}\ndata: {{\"seq\":{n},\"at\":0,\"kind\":\"k\"}}\n\n")
}

const OPEN: &str =
    "event: open\ndata: {\"contract\":1,\"after\":0,\"live\":true,\"partials\":false}\n\n";

fn fast() -> Follow {
    Follow {
        first: Duration::from_millis(30),
        max: Duration::from_millis(120),
        healthy: Duration::from_secs(3600),
    }
}

/// Run `follow` until `until` says so; return every event.
fn run(base: &str, after: Option<u64>, until: impl Fn(&[Event]) -> bool) -> Vec<Event> {
    let client = Client::new(base).unwrap();
    let stop = AtomicBool::new(false);
    let mut got = Vec::new();
    rung_client::follow(&client, "chief-a", after, fast(), &stop, |e| {
        got.push(e);
        !until(&got)
    });
    got
}

fn seqs(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Frame(Frame::Record { seq, .. }) => Some(*seq),
            _ => None,
        })
        .collect()
}

fn lost(events: &[Event]) -> Vec<Duration> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Lost { retry_in, .. } => Some(*retry_in),
            _ => None,
        })
        .collect()
}

#[test]
fn a_dropped_connection_resumes_after_the_last_record_seen() {
    let (base, seen) = stub(vec![
        (
            200,
            format!("{OPEN}{}{}{}", record(1), record(2), record(3)),
        ),
        (200, format!("{OPEN}{}{}", record(4), record(5))),
    ]);
    let got = run(&base, None, |g| seqs(g).len() == 5);
    assert_eq!(
        seqs(&got),
        [1, 2, 3, 4, 5],
        "in order, none missed, none twice"
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], None, "the first connection names no cursor");
    assert_eq!(
        seen[1].as_deref(),
        Some("3"),
        "the second resumes after record 3"
    );
    assert!(got.iter().any(|e| matches!(e, Event::Lost { .. })));
}

#[test]
fn a_start_cursor_is_sent_and_the_back_off_doubles_to_its_ceiling() {
    // the gateway is away: refused over and over, then it answers
    let mut script: Script = (0..5).map(|_| (503, String::new())).collect();
    script.push((200, format!("{OPEN}{}", record(8))));
    let (base, seen) = stub(script);
    let got = run(&base, Some(7), |g| seqs(g) == [8]);
    let waits = lost(&got);
    let ms: Vec<u128> = waits.iter().map(|w| w.as_millis()).collect();
    assert_eq!(
        &ms[..5],
        [30, 60, 120, 120, 120],
        "doubles, then holds at the max"
    );
    let seen = seen.lock().unwrap();
    assert!(seen.iter().all(|s| s.as_deref() == Some("7")), "{seen:?}");
}

#[test]
fn a_cursor_ahead_of_the_record_starts_over() {
    let (base, seen) = stub(vec![
        (
            409,
            r#"{"error":{"code":"cursor_ahead","message":"ahead"}}"#.into(),
        ),
        (200, format!("{OPEN}{}", record(1))),
    ]);
    let got = run(&base, Some(99), |g| seqs(g) == [1]);
    assert_eq!(seqs(&got), [1]);
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].as_deref(), Some("99"));
    assert_eq!(seen[1], None, "after a 409 the cursor is dropped");
}

#[test]
fn the_end_frame_is_passed_on_and_the_follow_goes_on_for_a_restart() {
    let (base, seen) = stub(vec![
        (
            200,
            format!(
                "{OPEN}{}event: end\ndata: {{\"reason\":\"stopped\",\"last_seq\":1,\"problem\":null}}\n\n",
                record(1)
            ),
        ),
        (200, format!("{OPEN}{}", record(2))),
    ]);
    let got = run(&base, None, |g| seqs(g).len() == 2);
    assert!(
        got.iter()
            .any(|e| matches!(e, Event::Frame(Frame::End(end)) if end.reason == "stopped"))
    );
    assert_eq!(seen.lock().unwrap()[1].as_deref(), Some("1"));
}

#[test]
fn a_stop_flag_ends_a_follow_that_is_waiting() {
    let (base, _) = stub(vec![]); // every connection is refused with 503
    let client = Client::new(&base).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let h = thread::spawn(move || {
        let slow = Follow {
            first: Duration::from_secs(30),
            max: Duration::from_secs(30),
            healthy: Duration::from_secs(3600),
        };
        rung_client::follow(&client, "chief-a", None, slow, &flag, |_| true);
    });
    thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::Relaxed);
    let t = std::time::Instant::now();
    h.join().unwrap();
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "the stop was seen within a slice"
    );
}

#[test]
fn a_bad_instance_id_is_reported_once_and_ends_the_follow() {
    let (base, _) = stub(vec![]);
    let client = Client::new(&base).unwrap();
    let stop = AtomicBool::new(false);
    let mut events = Vec::new();
    rung_client::follow(&client, "a/b", None, fast(), &stop, |e| {
        events.push(e);
        true
    });
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(matches!(&events[0], Event::Lost { why, .. } if why.contains("not an instance id")));
}
