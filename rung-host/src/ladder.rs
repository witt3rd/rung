//! The model ladder's listing filter.
//!
//! The ladder is configuration: model ids, best first. At start and every
//! six hours the host lists the router's models (a keyless GET of
//! `{base}/models`, and `{base}/models/{id}/endpoints` for each configured
//! rung still standing) and keeps only the rungs that are available and
//! free. A rung is available when it is listed, its `expiration_date` has
//! not begun (an expiry date is the first day it is gone), its prompt and
//! completion prices are zero, it takes `tools`, and one of its endpoints
//! has a status of at least 0. The verdicts are one `ladder.listed` record
//! line; the governor walks only available rungs (step-down skips the
//! others, a probe goes to the nearest available one above), and a listing
//! that takes the current rung away switches to the best available one at
//! once. A listing that fails (the router unreachable, a non-2xx answer)
//! keeps the previous verdicts and is tried again in 15 minutes; it never
//! stops the host.
//!
//! [`OPENROUTER_FREE_LADDER`] is the free ladder the operator ruled for a
//! first live substrate. It is a default for configuration, not a
//! hard-coded walk: the filter decides what of it is usable.

use std::time::Duration;

use serde_json::{Value, json};

use crate::clock::{HOUR, MINUTE, Millis};

/// The free ladder, best first. `stealth/space-bunny-alpha` is listed with
/// an expiration date; the filter drops it from that day on.
pub const OPENROUTER_FREE_LADDER: [&str; 5] = [
    "stealth/space-bunny-alpha",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "qwen/qwen3.8-27b:free",
    "google/gemma-4-31b-it:free",
    "nvidia/nemotron-3-super-120b-a12b:free",
];

/// How often a successful listing is refreshed.
pub const REFRESH_MS: Millis = 6 * HOUR;
/// How soon a failed listing is tried again.
pub const RETRY_MS: Millis = 15 * MINUTE;

/// Where the listing comes from.
pub trait Lister: Send + Sync {
    /// The `/models` answer.
    fn models(&self) -> Result<Value, String>;
    /// The `/models/{id}/endpoints` answer; `Ok(None)` when the router
    /// does not know the model (404).
    fn endpoints(&self, id: &str) -> Result<Option<Value>, String>;
}

/// Lists over HTTP, with no key: the listing is public.
#[derive(Debug, Clone)]
pub struct HttpLister {
    base: String,
    timeout: Duration,
}

impl HttpLister {
    /// `base` is the route's API base, e.g. `https://openrouter.ai/api/v1`.
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            timeout: Duration::from_secs(30),
        }
    }

    fn get(&self, path: &str) -> Result<Option<Value>, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| e.to_string())?;
        let url = format!("{}{path}", self.base);
        let resp = client.get(&url).send().map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&status) {
            return Err(format!("GET {path}: HTTP {status}"));
        }
        let text = resp.text().map_err(|e| e.to_string())?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("GET {path}: {e}"))
    }
}

impl Lister for HttpLister {
    fn models(&self) -> Result<Value, String> {
        self.get("/models")?
            .ok_or_else(|| "GET /models: HTTP 404".to_string())
    }

    fn endpoints(&self, id: &str) -> Result<Option<Value>, String> {
        self.get(&format!("/models/{id}/endpoints"))
    }
}

/// A router's "no endpoint for you" refusal: a 404 whose error names why the
/// model's endpoints were excluded for this account (data policy,
/// guardrails). Its reason codes, sorted, or `None` for any other answer.
pub fn refusal(status: u16, body: &str) -> Option<Vec<String>> {
    if status != 404 {
        return None;
    }
    let body: Value = serde_json::from_str(body).ok()?;
    let reasons = body["error"]["metadata"]["ineligibility_reasons"].as_array()?;
    let mut out: Vec<String> = reasons
        .iter()
        .filter_map(|r| r["reason"].as_str().map(String::from))
        .collect();
    out.sort();
    out.dedup();
    if out.is_empty() {
        out.push("unspecified".into());
    }
    Some(out)
}

/// What one keyed probe of a model found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probed {
    /// The router served it.
    Routes,
    /// The router refused it for this account, with its reasons.
    Refused(Vec<String>),
    /// Anything else (a rate limit, a 5xx, a transport error): says nothing
    /// about the account's policy.
    Unknown(String),
}

/// Asks the router, with the account's key, whether it will route a model.
pub trait Prober: Send + Sync {
    fn probe(&self, model: &str) -> Probed;
}

/// One tiny keyed chat request per model: one user word, at most one
/// output token, no tools.
pub struct HttpProber {
    base: String,
    key: String,
    timeout: Duration,
}

impl std::fmt::Debug for HttpProber {
    // The key is never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpProber")
            .field("base", &self.base)
            .field("has_key", &!self.key.is_empty())
            .finish()
    }
}

impl HttpProber {
    pub fn new(base: &str, key: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            key: key.to_string(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl Prober for HttpProber {
    fn probe(&self, model: &str) -> Probed {
        let client = match reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
        {
            Ok(c) => c,
            Err(e) => return Probed::Unknown(e.to_string()),
        };
        let body = json!({"model": model, "stream": false, "max_tokens": 1,
                          "messages": [{"role": "user", "content": "OK"}]});
        let sent = client
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send();
        let resp = match sent {
            Ok(r) => r,
            // reqwest's error names the URL, never the headers.
            Err(e) => return Probed::Unknown(e.to_string()),
        };
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            return Probed::Routes;
        }
        let text = resp.text().unwrap_or_default();
        match refusal(status, &text) {
            Some(reasons) => Probed::Refused(reasons),
            None => Probed::Unknown(format!("HTTP {status}")),
        }
    }
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date.
fn days_from_civil(date: &str) -> Option<i64> {
    let parts: Vec<i64> = date
        .get(..10)?
        .split('-')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [y, m, d] = parts[..] else { return None };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn is_zero(v: &Value) -> bool {
    v.as_str()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .or_else(|| v.as_f64())
        == Some(0.0)
}

/// The verdict on one listed model before its endpoints are asked:
/// `Err(why)` when it is out.
fn listed_verdict(listing: &Value, model: &str, now: Millis) -> Result<(), &'static str> {
    let m = listing["data"]
        .as_array()
        .and_then(|d| d.iter().find(|m| m["id"] == model))
        .ok_or("not_listed")?;
    if let Some(day) = m["expiration_date"].as_str().and_then(days_from_civil)
        && now >= day * 24 * HOUR
    {
        return Err("expired");
    }
    if !(is_zero(&m["pricing"]["prompt"]) && is_zero(&m["pricing"]["completion"])) {
        return Err("not_free");
    }
    let tools = m["supported_parameters"]
        .as_array()
        .is_some_and(|p| p.iter().any(|x| x == "tools"));
    if !tools {
        return Err("no_tools");
    }
    Ok(())
}

/// A router in the listing (OpenRouter's `openrouter/free`: tokenizer
/// `Router`) picks a model per request and lists no endpoints of its own;
/// it stands on the listing's other checks and the keyed probe tests it.
fn is_router(listing: &Value, model: &str) -> bool {
    listing["data"].as_array().is_some_and(|d| {
        d.iter()
            .any(|m| m["id"] == model && m["architecture"]["tokenizer"] == "Router")
    })
}

fn endpoint_up(e: &Value) -> bool {
    e["data"]["endpoints"].as_array().is_some_and(|eps| {
        eps.iter()
            .any(|x| x["status"].as_i64().is_some_and(|s| s >= 0))
    })
}

/// List and judge `ladder` at `now`: the body of a `ladder.listed` line.
/// `previous` is the last verdicts (per rung, available or not), kept when
/// the listing fails.
pub fn list(lister: &dyn Lister, ladder: &[String], previous: &[bool], now: Millis) -> Value {
    let judged: Result<Vec<(bool, &'static str)>, String> = (|| {
        let listing = lister.models()?;
        if listing["data"].as_array().is_none() {
            return Err("GET /models: no data array".to_string());
        }
        let mut out = Vec::with_capacity(ladder.len());
        for model in ladder {
            match listed_verdict(&listing, model, now) {
                Err(why) => out.push((false, why)),
                Ok(()) if is_router(&listing, model) => out.push((true, "router")),
                Ok(()) => match lister.endpoints(model)? {
                    Some(e) if endpoint_up(&e) => out.push((true, "ok")),
                    _ => out.push((false, "endpoint_down")),
                },
            }
        }
        Ok(out)
    })();
    let rungs = |v: &[(bool, String)]| -> Vec<Value> {
        ladder
            .iter()
            .zip(v)
            .enumerate()
            .map(|(i, (m, (a, w)))| json!({"rung": i, "model": m, "available": a, "why": w}))
            .collect()
    };
    match judged {
        Ok(v) => {
            let v: Vec<(bool, String)> = v.into_iter().map(|(a, w)| (a, w.to_string())).collect();
            let available = v.iter().filter(|x| x.0).count();
            json!({"ok": true, "rungs": rungs(&v), "available": available,
                   "next_at": now + REFRESH_MS})
        }
        Err(e) => {
            // Keep what was known; before any listing, every rung stands.
            let v: Vec<(bool, String)> = (0..ladder.len())
                .map(|i| {
                    let a = previous.get(i).copied().unwrap_or(true);
                    (a, if a { "kept" } else { "kept_unavailable" }.to_string())
                })
                .collect();
            let available = v.iter().filter(|x| x.0).count();
            json!({"ok": false, "error": e, "rungs": rungs(&v), "available": available,
                   "next_at": now + RETRY_MS})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn a_prober_never_prints_its_key_and_reads_only_a_named_refusal() {
        let p = HttpProber::new("http://x/", "sk-secret-value");
        let s = format!("{p:?}");
        assert!(
            !s.contains("sk-secret-value") && s.contains("has_key: true"),
            "{s}"
        );
        let body = r#"{"error":{"metadata":{"ineligibility_reasons":[{"reason":"b"},{"reason":"a"},{"reason":"a"}]}}}"#;
        assert_eq!(refusal(404, body), Some(vec!["a".into(), "b".into()]));
        assert_eq!(refusal(429, body), None);
        assert_eq!(
            refusal(404, r#"{"error":{"message":"no such model"}}"#),
            None
        );
    }

    struct Fixed {
        listing: Result<Value, String>,
        endpoints: BTreeMap<String, Value>,
    }

    impl Lister for Fixed {
        fn models(&self) -> Result<Value, String> {
            self.listing.clone()
        }
        fn endpoints(&self, id: &str) -> Result<Option<Value>, String> {
            Ok(self.endpoints.get(id).cloned())
        }
    }

    fn model(id: &str, prompt: &str, params: &[&str], exp: Option<&str>) -> Value {
        json!({"id": id, "pricing": {"prompt": prompt, "completion": "0"},
               "supported_parameters": params, "expiration_date": exp})
    }

    fn up(status: i64) -> Value {
        json!({"data": {"endpoints": [{"status": status}]}})
    }

    fn lister() -> Fixed {
        Fixed {
            listing: Ok(json!({"data": [
                model("a", "0", &["tools"], Some("2026-10-05")),
                model("b", "0", &["tools"], None),
                model("c", "0", &["max_tokens"], None),
                model("d", "0.000001", &["tools"], None),
                model("e", "0", &["tools"], None),
            ]})),
            endpoints: BTreeMap::from([
                ("a".to_string(), up(0)),
                ("b".to_string(), up(0)),
                ("e".to_string(), up(-2)),
            ]),
        }
    }

    fn ladder() -> Vec<String> {
        ["a", "b", "c", "d", "e", "f"].map(String::from).to_vec()
    }

    fn whys(v: &Value) -> Vec<(bool, String)> {
        v["rungs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["available"].as_bool().unwrap(),
                    r["why"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    /// 2026-10-04T23:59:59.999Z and 2026-10-05T00:00Z.
    const BEFORE: Millis = 1_791_158_399_999;
    const ON: Millis = 1_791_158_400_000;
    const DAY_MS: Millis = 24 * HOUR;

    #[test]
    fn each_rung_gets_its_verdict() {
        let v = list(&lister(), &ladder(), &[], BEFORE);
        assert_eq!(v["ok"], true);
        assert_eq!(
            whys(&v),
            [
                (true, "ok"),
                (true, "ok"),
                (false, "no_tools"),
                (false, "not_free"),
                (false, "endpoint_down"),
                (false, "not_listed"),
            ]
            .map(|(a, w)| (a, w.to_string()))
        );
        assert_eq!(v["available"], 2);
        assert_eq!(v["next_at"], BEFORE + REFRESH_MS);
    }

    #[test]
    fn an_expiry_date_is_the_first_day_gone() {
        let v = list(&lister(), &ladder(), &[], ON);
        assert_eq!(whys(&v)[0], (false, "expired".to_string()));
        assert_eq!(days_from_civil("2026-10-05"), Some(ON / (24 * HOUR)));
    }

    #[test]
    fn a_failed_listing_keeps_the_previous_verdicts() {
        let mut l = lister();
        l.listing = Err("GET /models: HTTP 502".into());
        let v = list(
            &l,
            &ladder(),
            &[false, true, false, false, false, false],
            ON,
        );
        assert_eq!(v["ok"], false);
        assert_eq!(v["next_at"], ON + RETRY_MS);
        let a: Vec<bool> = whys(&v).into_iter().map(|x| x.0).collect();
        assert_eq!(a, [false, true, false, false, false, false]);
        // Before any listing, every rung stands.
        let v = list(&l, &ladder(), &[], ON);
        assert_eq!(v["available"], 6);
    }

    #[test]
    fn a_refusal_without_named_reasons_is_unspecified_and_malformed_bodies_are_not_refusals() {
        let empty = r#"{"error":{"metadata":{"ineligibility_reasons":[]}}}"#;
        assert_eq!(refusal(404, empty), Some(vec!["unspecified".into()]));
        let unnamed = r#"{"error":{"metadata":{"ineligibility_reasons":[{"x":1}]}}}"#;
        assert_eq!(refusal(404, unnamed), Some(vec!["unspecified".into()]));
        assert_eq!(refusal(404, "not json"), None);
        assert_eq!(refusal(404, r#"{"error":{"metadata":{}}}"#), None);
        assert_eq!(refusal(500, empty), None);
    }

    #[test]
    fn a_price_is_free_only_when_it_parses_to_zero() {
        for free in [json!("0"), json!("0.0"), json!(" 0 "), json!(0), json!(0.0)] {
            assert!(is_zero(&free), "{free}");
        }
        for paid in [
            json!("0.000001"),
            json!("-1"),
            json!("free"),
            json!(""),
            json!(null),
            json!(1),
        ] {
            assert!(!is_zero(&paid), "{paid}");
        }
        // A model with no pricing at all is not free.
        let l = json!({"data": [{"id": "m", "supported_parameters": ["tools"]}]});
        assert_eq!(listed_verdict(&l, "m", ON), Err("not_free"));
        // A free prompt with a paid completion is not free either.
        let l = json!({"data": [{"id": "m", "supported_parameters": ["tools"],
            "pricing": {"prompt": "0", "completion": "0.5"}}]});
        assert_eq!(listed_verdict(&l, "m", ON), Err("not_free"));
    }

    #[test]
    fn expiry_dates_are_read_as_civil_days_and_unreadable_ones_never_expire() {
        assert_eq!(days_from_civil("1970-01-01"), Some(0));
        assert_eq!(days_from_civil("1970-01-02"), Some(1));
        // A leap day exists in 2028 and the day after it is the next day.
        let feb29 = days_from_civil("2028-02-29").unwrap();
        assert_eq!(days_from_civil("2028-03-01"), Some(feb29 + 1));
        // A timestamp's time of day is ignored: only the date counts.
        assert_eq!(days_from_civil("2026-10-05T12:00:00Z"), Some(ON / DAY_MS));
        for bad in [
            "",
            "2026-10",
            "2026-13-01",
            "2026-00-10",
            "2026-10-32",
            "abcd-ef-gh",
        ] {
            assert_eq!(days_from_civil(bad), None, "{bad}");
        }
        let mut l = lister();
        l.listing = Ok(json!({"data": [model("a", "0", &["tools"], Some("soon"))]}));
        l.endpoints = BTreeMap::from([("a".to_string(), up(0))]);
        let v = list(&l, &["a".to_string()], &[], ON * 10);
        assert_eq!(whys(&v), [(true, "ok".to_string())]);
    }

    #[test]
    fn an_endpoint_is_up_at_status_zero_and_down_below_it() {
        assert!(endpoint_up(&up(0)));
        assert!(endpoint_up(&up(1)));
        assert!(!endpoint_up(&up(-1)));
        assert!(!endpoint_up(&json!({"data": {"endpoints": []}})));
        assert!(!endpoint_up(&json!({"data": {}})));
        assert!(!endpoint_up(
            &json!({"data": {"endpoints": [{"status": "ok"}]}})
        ));
        // One live endpoint among dead ones is enough.
        let mixed = json!({"data": {"endpoints": [{"status": -5}, {"status": 0}]}});
        assert!(endpoint_up(&mixed));
    }

    #[test]
    fn a_listed_rung_the_router_has_no_endpoints_for_is_down() {
        let mut l = lister();
        l.endpoints.clear();
        let v = list(&l, &ladder(), &[], BEFORE);
        assert_eq!(whys(&v)[0], (false, "endpoint_down".to_string()));
        assert_eq!(v["available"], 0);
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn a_free_router_with_no_endpoints_of_its_own_stands() {
        let mut router = model("r", "0", &["tools"], None);
        router["architecture"] = json!({"tokenizer": "Router"});
        let l = Fixed {
            listing: Ok(json!({"data": [router, model("m", "0", &["tools"], None)]})),
            endpoints: BTreeMap::from([("r".to_string(), json!({"data": {"endpoints": []}}))]),
        };
        let ladder = ["m", "r"].map(String::from).to_vec();
        let v = list(&l, &ladder, &[], BEFORE);
        assert_eq!(
            whys(&v),
            [(false, "endpoint_down"), (true, "router")].map(|(a, w)| (a, w.to_string()))
        );
    }

    #[test]
    fn a_listing_without_a_data_array_fails_and_keeps_the_previous_verdicts() {
        let mut l = lister();
        l.listing = Ok(json!({"error": "nope"}));
        let v = list(&l, &ladder(), &[true, false], ON);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "GET /models: no data array");
        assert_eq!(v["next_at"], ON + RETRY_MS);
        let w = whys(&v);
        assert_eq!(w[0], (true, "kept".to_string()));
        assert_eq!(w[1], (false, "kept_unavailable".to_string()));
        // Rungs beyond the previous verdicts stand.
        assert_eq!(w[5], (true, "kept".to_string()));
    }

    #[test]
    fn an_endpoint_lookup_that_fails_fails_the_whole_listing() {
        struct Flaky;
        impl Lister for Flaky {
            fn models(&self) -> Result<Value, String> {
                Ok(json!({"data": [model("a", "0", &["tools"], None)]}))
            }
            fn endpoints(&self, _: &str) -> Result<Option<Value>, String> {
                Err("GET /models/a/endpoints: HTTP 503".into())
            }
        }
        let v = list(&Flaky, &["a".to_string()], &[false], ON);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "GET /models/a/endpoints: HTTP 503");
        assert_eq!(whys(&v), [(false, "kept_unavailable".to_string())]);
    }
}
