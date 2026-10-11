//! S4: the host's own `GET /v1/turns/{n}/context` door on its HTTP listener,
//! over real HTTP, for an instance that is stopped (the record of a finished
//! run is the only thing read). A sentinel is never put in an assertion
//! message.

mod common;

use std::sync::Arc;

use common::ctx::*;
use rung_agent_core::acp_http::{Serve, listen_bound};
use rung_host::acp::EventsDoor;
use rung_host::clock::{Clock, SimClock};
use rung_host::memory::MemoryHost;
use rung_host::redact::Redactor;
use rung_host::sim::{self, SIM_START};
use rung_host::stream::LiveBus;
use serde_json::Value;

const OWNER: &str = "door-token-owner-s4-7731";
const VIEW: &str = "door-token-viewer-s4-5521";
const SENT: &str = "plainsentinels4door-0123456789";

struct Served {
    base: String,
    turn: u64,
    dir: rung_host::sim::TempDir,
}

/// A finished run (three turns of five calls; the first is the owner's, and
/// memory recalls a long text with a secret in it), then a listener over its
/// state directory. No host holds the directory.
async fn served(tag: &str) -> Served {
    let guard = sim::temp_dir_guard(tag);
    let (mut sc, _script) = scripted(guard.path(), 5, 3);
    let clk: Arc<dyn Clock> = Arc::new(SimClock::new(SIM_START));
    sc.clock = Some(clk.clone());
    sc.engine = Some(Script::new(5, clk));
    sc.sources.push(owner_says(SIM_START, "what is the code"));
    // the record is written by a host that does not know the secret: the
    // door's own redactor is what is tested
    sc.memory_host = Some(Arc::new(MemoryHost::new(
        Arc::new(Big {
            text: long_recall(2_100, Some((11, SENT))),
        }),
        "s",
    )));
    let out = sim::run(sc);
    let turn = out
        .lines
        .iter()
        .find(|l| l.kind == "turn.log" && l.get("recall") == &serde_json::json!(true))
        .expect("a recall")
        .u64("turn");
    drop(out);
    let door = Arc::new(EventsDoor {
        state_dir: guard.path().to_path_buf(),
        bus: LiveBus::new(),
        red: Redactor::new().with_secret(SENT),
        env_unset: vec!["SOME_UNSET_KEY_ENV".into()],
        tokens: [OWNER.to_string(), VIEW.to_string()].into_iter().collect(),
    });
    let serve = Serve {
        name: "test".into(),
        authorize: Arc::new(|_| true),
        connect: Arc::new(|_, _| Box::pin(async {})),
        bind_bearer: false,
        routes: Some(door.routes()),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = listen_bound("127.0.0.1:0".into(), serve, move |a| {
            let _ = tx.send(a);
        })
        .await;
    });
    let addr = rx.await.unwrap();
    Served {
        base: format!("http://{addr}"),
        turn,
        dir: guard,
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

async fn get(s: &Served, path: &str, token: Option<&str>) -> (u16, String) {
    let mut r = http().get(format!("{}{path}", s.base));
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let resp = r.send().await.unwrap();
    (resp.status().as_u16(), resp.text().await.unwrap())
}

#[tokio::test]
async fn both_roles_read_the_context_of_a_stopped_instance_whole() {
    let s = served("ctx-door-roles").await;
    let path = format!("/v1/turns/{}/context?call=3", s.turn);
    let mut first: Option<Value> = None;
    for token in [OWNER, VIEW] {
        let (st, body) = get(&s, &path, Some(token)).await;
        assert_eq!(st, 200);
        let v: Value = serde_json::from_str(&body).expect("one JSON document");
        assert_eq!(v["turn"], s.turn);
        assert_eq!(v["call"], 3);
        assert_eq!(v["recorded"], true);
        let recall = band(&v, "recall")["text"].as_str().unwrap();
        assert!(recall.len() > 200_000 && recall.contains("rl02099"), "the recall, whole");
        assert!(recall.contains("[redacted]"));
        assert!(!body.contains(SENT), "the door shows the plain secret");
        assert_eq!(v["redaction"]["env_unset"][0], "SOME_UNSET_KEY_ENV");
        match &first {
            None => first = Some(v),
            Some(f) => assert_eq!(f, &v, "the same answer for both roles"),
        }
    }
    // and it is what the library gives for the same record
    let lib = door(s.dir.path(), s.turn, Some(3), &Redactor::new().with_secret(SENT)).unwrap();
    assert_eq!(first.unwrap()["bands"], lib["bands"]);
}

#[tokio::test]
async fn the_door_refuses_what_it_should() {
    let s = served("ctx-door-refuse").await;
    let p = |q: &str| format!("/v1/turns/{}/context{q}", s.turn);
    // no token, an unknown token
    assert_eq!(get(&s, &p(""), None).await.0, 401);
    assert_eq!(get(&s, &p(""), Some("not-a-token")).await.0, 401);
    // a call out of range, a turn that is not there: plain refusals
    let (st, body) = get(&s, &p("?call=99"), Some(OWNER)).await;
    assert_eq!(st, 404);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["error"], "no_such_call");
    let (st, body) = get(&s, "/v1/turns/999/context", Some(OWNER)).await;
    assert_eq!(st, 404);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["error"], "no_such_turn");
    // a query that is not as documented
    for q in ["?call=abc", "?call=1&call=2", "?after=3", "?call="] {
        assert_eq!(get(&s, &p(q), Some(OWNER)).await.0, 400, "{q}");
    }
    assert_eq!(get(&s, "/v1/turns/x/context", Some(OWNER)).await.0, 400);
    // a write is not a door here
    let resp = http()
        .post(format!("{}{}", s.base, p("")))
        .header("authorization", format!("Bearer {OWNER}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 405);
    // HEAD: the headers, no body
    let resp = http()
        .head(format!("{}{}", s.base, p("")))
        .header("authorization", format!("Bearer {VIEW}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
