//! S4: `GET /api/i/{id}/v1/turns/{n}/context` for a native instance, read from
//! its own state directory: a stopped instance answers, both roles read, the
//! text is whole and redacted, an old record answers what it has. The state
//! directories are written by hand; nothing starts a host. A sentinel is never
//! put in an assertion message.

use rung_gateway::Settings;
use rung_testkit::TempDir;
use serde_json::{Value, json};

const READ_TOKEN: &str = "view-token-canary-s4-5521";
/// A sentinel with a known key shape: found by the redactor with no help.
const SHAPED: &str = "sk-SENTINELs4shaped0123456789abcdef";
const T0: i64 = 1_791_208_186_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn line(seq: u64, kind: &str, body: Value) -> String {
    let mut m = body.as_object().cloned().unwrap_or_default();
    m.insert("seq".into(), seq.into());
    m.insert("at".into(), (T0 + seq as i64).into());
    m.insert("kind".into(), kind.into());
    format!("{}\n", Value::Object(m))
}

fn user(text: &str) -> Value {
    json!({"role": "user", "content": text})
}

fn assistant(text: &str) -> Value {
    json!({"role": "assistant", "content": text})
}

/// Two turns of two calls each. With `layers`, the record has the request
/// layers; without, it is a record from before they were kept.
fn story(layers: bool, recall: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    let mut put = |k: &str, b: Value| {
        n += 1;
        out.push_str(&line(n, k, b));
    };
    put("host.start", json!({"pid": 1, "config": {}}));
    if layers {
        put(
            "pack.layers",
            json!({"cause": "start", "s_hash": "s1",
                   "tools": [{"name": "note", "description": "keep a note",
                              "input_schema": {"type": "object"}}],
                   "system": "the system text"}),
        );
        put(
            "epoch.rollover",
            json!({"from": 0, "to": 1, "cause": "start", "l1": "epoch 1", "first_turn": 1,
                   "l_hash": "l1", "slow": "the epoch layer"}),
        );
    } else {
        put(
            "epoch.rollover",
            json!({"from": 0, "to": 1, "cause": "start", "l1": "epoch 1", "first_turn": 1}),
        );
    }
    for t in 1..=2u64 {
        put(
            "turn.started",
            json!({"turn": t, "epoch": 1, "turn_kind": "free"}),
        );
        for c in 1..=2u64 {
            put(
                "llm.call",
                json!({"turn": t, "call": c, "epoch": 1, "model_requested": "m",
                       "model_served": "m", "provider": "p",
                       "prompt_tokens": 1000 + c * 100, "cached_tokens": 900,
                       "cache_write_tokens": 0, "completion_tokens": 5,
                       "prefix": {"s_hash": "s1", "l_hash": "l1", "log_len_bytes": 1,
                                  "expected_cached_tokens": 800}}),
            );
        }
        let mut tl = json!({"turn": t, "header": format!("[turn {t}]"), "recall": layers && t == 2,
                            "messages": [assistant("looking"), user("found"), assistant("done")]});
        if layers {
            // the recall once, with the hash of what is stored
            if t == 2 {
                tl["recall_text"] = recall.into();
                tl["recall_hash"] = rung_host::canon::hash(recall.as_bytes()).into();
            } else {
                tl["recall_text"] = Value::Null;
                tl["recall_hash"] = Value::Null;
            }
        }
        put("turn.log", tl);
        put("turn.ended", json!({"turn": t, "status": "completed"}));
    }
    out
}

struct Fleet {
    root: TempDir,
}

impl Fleet {
    fn new(tag: &str, record: &str) -> Self {
        let root = TempDir::new(tag);
        let registry = root.path().join("registry");
        let state = root.path().join("alpha/state");
        std::fs::create_dir_all(&registry).unwrap();
        std::fs::create_dir_all(state.join("record")).unwrap();
        std::fs::write(
            registry.join("alpha.json"),
            json!({"schema": 1, "id": "alpha", "name": "alpha", "state_dir": state,
                   "config": null, "workspace": root.path().join("w"), "address": null,
                   "pid": 1, "started": T0, "version": "0.2.3"})
            .to_string(),
        )
        .unwrap();
        std::fs::write(state.join("record/seg-00000001.ndjson"), record).unwrap();
        Fleet { root }
    }

    async fn serve(&self) -> (String, rung_gateway::Running) {
        let running = rung_gateway::start(Settings {
            listen: "127.0.0.1:0".parse().unwrap(),
            app_dir: None,
            allowed_hosts: vec![],
            read_only_tokens: vec![READ_TOKEN.into()],
            registry_dir: Some(self.root.path().join("registry")),
            instances: vec![],
        })
        .await
        .unwrap();
        (format!("http://{}", running.addr), running)
    }
}

async fn send(rb: reqwest::RequestBuilder) -> (u16, String) {
    let r = rb.send().await.unwrap();
    (r.status().as_u16(), r.text().await.unwrap())
}

fn band<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["band"] == name)
        .unwrap_or_else(|| panic!("no {name} band"))
}

fn long_recall() -> String {
    let mut lines: Vec<String> = (0..2_100)
        .map(|i| format!("rl{i:05} {}", "x".repeat(91)))
        .collect();
    lines[5] = format!("rl00005 the key is {SHAPED} {}", "y".repeat(40));
    lines.join("\n")
}

/// A stopped instance (no host holds its state) answers for both roles, whole,
/// with a secret in the recall shown as `[redacted]`.
#[tokio::test]
async fn both_roles_read_the_context_of_a_stopped_instance() {
    let f = Fleet::new("s4-ctx", &story(true, &long_recall()));
    let (base, _gw) = f.serve().await;
    let url = format!("{base}/api/i/alpha/v1/turns/2/context?call=2");
    let owner = send(client().get(&url)).await;
    let reader = send(client().get(&url).bearer_auth(READ_TOKEN)).await;
    assert_eq!(owner.0, 200);
    assert_eq!(owner, reader, "both roles read the same answer");
    let v: Value = serde_json::from_str(&owner.1).unwrap();
    assert_eq!(v["recorded"], true);
    assert_eq!(v["provider"]["prompt_tokens"], 1200);
    assert_eq!(v["provider"]["cached_tokens"], 900);
    let names: Vec<&str> = v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["band"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "tools",
            "system",
            "epoch",
            "history",
            "message",
            "recall",
            "this_turn"
        ]
    );
    assert_eq!(band(&v, "system")["text"], "the system text");
    assert_eq!(band(&v, "epoch")["text"], "the epoch layer");
    assert_eq!(band(&v, "tools")["items"][0]["name"], "note");
    assert_eq!(band(&v, "history")["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        band(&v, "this_turn")["messages"].as_array().unwrap().len(),
        2
    );
    let recall = band(&v, "recall")["text"].as_str().unwrap();
    assert!(recall.len() > 200_000, "the recall is whole");
    assert!(recall.contains("rl02099"));
    assert!(recall.contains("[redacted]"));
    assert!(!owner.1.contains(SHAPED), "the door shows the key");
}

/// A record from before the layers answers with what it has, not an error.
#[tokio::test]
async fn an_old_record_answers_that_it_predates() {
    let f = Fleet::new("s4-ctx-old", &story(false, ""));
    let (base, _gw) = f.serve().await;
    let (st, body) =
        send(client().get(format!("{base}/api/i/alpha/v1/turns/2/context?call=2"))).await;
    assert_eq!(st, 200);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["recorded"], false);
    assert_eq!(v["note"], "this turn predates context recording");
    let names: Vec<&str> = v["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["band"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["history", "message", "this_turn"]);
    assert_eq!(v["provider"]["prompt_tokens"], 1200);
}

#[tokio::test]
async fn the_door_refuses_what_it_should() {
    let f = Fleet::new("s4-ctx-refuse", &story(true, "recalled"));
    let (base, _gw) = f.serve().await;
    let at = |p: &str| format!("{base}/api/i/{p}");
    // a call out of range, a turn, an instance that is not there
    for (path, status, code) in [
        ("alpha/v1/turns/1/context?call=3", 404, "no_such_call"),
        ("alpha/v1/turns/1/context?call=0", 404, "no_such_call"),
        ("alpha/v1/turns/9/context", 404, "no_such_turn"),
        ("nobody/v1/turns/1/context", 404, "no_such_instance"),
        ("alpha/v1/turns/x/context", 400, "bad_request"),
        ("alpha/v1/turns/1/context?call=a", 400, "bad_request"),
        ("alpha/v1/turns/1/context?call=1&call=2", 400, "bad_request"),
        ("alpha/v1/turns/1/context?limit=1", 400, "bad_request"),
    ] {
        let (st, body) = send(client().get(at(path))).await;
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!((st, v["error"].as_str().unwrap()), (status, code), "{path}");
    }
    // an unknown token; a bad Host; a write by either role
    let (st, _) = send(
        client()
            .get(at("alpha/v1/turns/1/context"))
            .bearer_auth("nope"),
    )
    .await;
    assert_eq!(st, 401);
    let (st, body) = send(
        client()
            .get(at("alpha/v1/turns/1/context"))
            .header("host", "evil.example"),
    )
    .await;
    assert_eq!(st, 403);
    assert!(body.contains("bad_host"));
    let (st, body) = send(
        client()
            .post(at("alpha/v1/turns/1/context"))
            .bearer_auth(READ_TOKEN),
    )
    .await;
    assert_eq!(st, 403);
    assert!(body.contains("read_only"));
    let (st, _) = send(client().post(at("alpha/v1/turns/1/context"))).await;
    assert_eq!(st, 405);
    drop(f);
}
