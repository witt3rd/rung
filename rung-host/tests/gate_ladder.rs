//! G-o: the model ladder's listing filter. The host lists the router's
//! models at start and every six hours (keyless GETs against a scripted
//! router on loopback serving recorded-shape fixtures), keeps only the
//! configured rungs that are available and free, and walks only those —
//! through a rung's expiry, a failed listing, provider 429s that must skip
//! an unavailable rung, a platform 429, and a rung the router refuses for
//! the account (unavailable until the next listing). No live model, no key.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::*;
use rung_host::adapter::{AdapterConfig, AgentEngine};
use rung_host::clock::{Clock, DAY, HOUR, MINUTE, SimClock};
use rung_host::gates::{self, Served, request_turn};
use rung_host::ladder::HttpLister;
use rung_host::sim::http::{LoopbackProvider, Reply, Request};
use rung_host::sim::{self, SIM_START};
use serde_json::{Value, json};

const LADDER: [&str; 7] = [
    // Free until its expiration date (2026-10-05).
    "stealth/space-bunny-alpha",
    // Listed and free, but its only endpoint is down (status -2).
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "qwen/qwen3.8-27b:free",
    // Listed and free, but takes no tools.
    "google/gemma-4-31b-it:free",
    "nvidia/nemotron-3-super-120b-a12b:free",
    // Not listed at all.
    "test/not-listed:free",
    // Listed, not free.
    "test/paid-model",
];

/// 2026-10-04T20:00Z.
const START: i64 = SIM_START + DAY + 20 * HOUR;

fn fixture(name: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/openrouter")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn the_ladder_lists_filters_and_walks_only_available_free_rungs() {
    sim::test_timeout(900);
    let listing = fixture("models.json");
    let endpoints: BTreeMap<String, Value> =
        serde_json::from_value(fixture("endpoints.json")).unwrap();
    let clock = Arc::new(SimClock::new(START));
    let c2 = clock.clone();
    let (l2, e2) = (listing.clone(), endpoints.clone());
    let mut model_gets = 0usize;
    let mut qwen_refused = false;
    let provider = LoopbackProvider::start(move |r: &Request<'_>| {
        if r.method == "GET" {
            let path = r.path.strip_prefix("/api/v1").unwrap_or(r.path);
            if path == "/models" {
                model_gets += 1;
                // The second listing (the first 6-hourly refresh) fails.
                if model_gets == 2 {
                    return Reply::json(502, &json!({"error": {"message": "bad gateway"}}));
                }
                return Reply::json(200, &l2);
            }
            if let Some(id) = path
                .strip_prefix("/models/")
                .and_then(|p| p.strip_suffix("/endpoints"))
            {
                return match e2.get(id) {
                    Some(v) => Reply::json(200, v),
                    None => Reply::json(404, &json!({"error": {"message": "no such model"}})),
                };
            }
            return Reply::json(404, &json!({"error": {"message": "not found"}}));
        }
        // Each completion takes two simulated minutes.
        c2.advance(2 * MINUTE);
        let model = r.body["model"].as_str().unwrap_or("").to_string();
        let turn = request_turn(r.body).unwrap_or(0);
        if model == LADDER[0] && turn == 5 {
            return Reply::provider_429("Stealth", 1);
        }
        // After the expiry, the second rung's upstream refuses once.
        if model == LADDER[2] && c2.now() > START + 7 * HOUR && !qwen_refused {
            qwen_refused = true;
            return Reply::provider_429("Alibaba", 1);
        }
        if turn == 40 {
            return Reply::platform_429(20, c2.now() + 3 * MINUTE, 1);
        }
        // A turn on the second standing rung is refused on every attempt:
        // the step-down lands on the rung the router refuses below.
        if model == LADDER[2] && turn == 250 {
            return Reply::provider_429("Alibaba", 1);
        }
        // The router will not route this rung for the account (its data
        // policy): unavailable until the next listing.
        if model == LADDER[4] {
            return Reply::json(
                404,
                &json!({"error": {"code": 404,
                    "message": "0 endpoints out of 1 requested are available matching your guardrail restrictions and data policy.",
                    "metadata": {"ineligibility_reasons": [{"reason": "zdr-violation-by-guardrail", "endpoint_count": 1}]}}}),
            );
        }
        let served = Served {
            model: model.clone(),
            prompt: r.body.to_string().len() as u64 / 4,
            cached: 0,
            cache_write: 0,
            completion: 9,
            cost_usd: 0.0,
        };
        Reply::completion("Upstream", Some("nothing pulls right now"), &[], served)
    });

    let mut sc = scenario("gate-o", 37);
    sc.start = START;
    sc.clock = Some(clock.clone());
    sc.config.engine = "agent".into();
    sc.config.ladder = LADDER.iter().map(|s| s.to_string()).collect();
    sc.memory = false;
    sc.until = Some(START + 13 * HOUR);
    let workspace = sc.dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let engine = AgentEngine::new(
        AdapterConfig::new(&provider.url, "test-key-not-a-secret", &workspace),
        clock.clone(),
    )
    .expect("engine");
    sc.engine = Some(Arc::new(engine));
    sc.lister = Some(Arc::new(HttpLister::new(&provider.url)));
    let out = sim::run(sc);
    let seen = provider.seen();
    let ladder: Vec<String> = LADDER.iter().map(|s| s.to_string()).collect();
    let g = gates::g_o(&out.lines, &seen, &listing, &endpoints, &ladder);
    assert_gate(&g);
    // The engine half of the walk still holds (G-n's 429 plans).
    let exited = !matches!(&out.why, rung_host::stop::Why::Stopped { by } if by == "limit");
    assert!(!exited, "the host stopped by itself: {:?}", out.why);
}
