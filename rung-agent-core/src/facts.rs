//! Per-model facts (context window, max output, price) from the router's
//! model listing, with a local override file and a last-good disk cache.
//!
//! Facts are data only: numbers keyed by model id. The cache and override
//! files are read under a strict schema (unknown fields refused); a
//! malformed file is ignored, never trusted. The listing itself is read
//! leniently (a router adds fields freely) but only the fact numbers are
//! taken from it. Order of precedence: override file, fresh listing, last
//! good cache. Nothing here holds an address, header or credential.
//!
//! ```text
//! $XDG_CONFIG_HOME/rung/model-facts.json   # override: {"models": {"id": {"context_window": N}}}
//! $XDG_CACHE_HOME/rung/model-facts.json    # last-good cache, written by this module
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What is known of one model. Absent means unknown, never zero.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output: Option<u64>,
    /// USD per million prompt tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_prompt_per_mtok: Option<f64>,
    /// USD per million completion tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_completion_per_mtok: Option<f64>,
}

/// The facts table: model id to facts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactsTable {
    pub models: BTreeMap<String, ModelFacts>,
}

impl FactsTable {
    pub fn get(&self, model: &str) -> Option<&ModelFacts> {
        self.models.get(model)
    }

    /// The context window of `model`, when listed and non-zero.
    pub fn window(&self, model: &str) -> Option<u64> {
        self.get(model)
            .and_then(|f| f.context_window)
            .filter(|w| *w > 0)
    }

    /// A host epoch budget: `fraction` of the model's window, else `fallback`.
    pub fn epoch_budget(&self, model: &str, fraction: f64, fallback: usize) -> usize {
        match self.window(model) {
            Some(w) => ((w as f64) * fraction.clamp(0.0, 1.0)) as usize,
            None => fallback,
        }
    }

    /// Entries of `over` replace those of `self`, fact by fact where set.
    pub fn overlaid(mut self, over: &FactsTable) -> FactsTable {
        for (id, o) in &over.models {
            let e = self.models.entry(id.clone()).or_default();
            e.context_window = o.context_window.or(e.context_window);
            e.max_output = o.max_output.or(e.max_output);
            e.price_prompt_per_mtok = o.price_prompt_per_mtok.or(e.price_prompt_per_mtok);
            e.price_completion_per_mtok =
                o.price_completion_per_mtok.or(e.price_completion_per_mtok);
        }
        self
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|x| x.is_finite() && *x >= 0.0)
}

/// Facts from a router listing (`{"data": [{"id", "context_length",
/// "top_provider": {"max_completion_tokens"}, "pricing": {"prompt",
/// "completion"}}]}`, prices in USD per token). Entries without an id are
/// skipped; a field that is not a plain non-negative number is dropped.
pub fn parse_listing(body: &str) -> Result<FactsTable, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("model listing: {e}"))?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("model listing: no data array")?;
    let mut table = FactsTable::default();
    for m in data {
        let Some(id) = m
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let whole = |x: Option<&Value>| x.and_then(number).map(|n| n as u64).filter(|n| *n > 0);
        let per_mtok = |x: Option<&Value>| x.and_then(number).map(|n| n * 1_000_000.0);
        table.models.insert(
            id.to_string(),
            ModelFacts {
                context_window: whole(m.get("context_length")),
                max_output: whole(m.pointer("/top_provider/max_completion_tokens")),
                price_prompt_per_mtok: per_mtok(m.pointer("/pricing/prompt")),
                price_completion_per_mtok: per_mtok(m.pointer("/pricing/completion")),
            },
        );
    }
    Ok(table)
}

/// Read a strict-schema table file; `None` when missing or invalid.
pub fn read_table(path: &Path) -> Option<FactsTable> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_cache(path: &Path, table: &FactsTable) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(body) = serde_json::to_string(table)
        && std::fs::write(&tmp, body).is_ok()
    {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Build the table: a fresh listing (written to the cache) or, when
/// `fetch` fails or yields no model, the last-good cache; the override
/// file applied over either.
pub fn build(
    fetch: impl FnOnce() -> Result<String, String>,
    cache: &Path,
    override_file: &Path,
) -> FactsTable {
    let base = match fetch().and_then(|b| parse_listing(&b)) {
        Ok(t) if !t.models.is_empty() => {
            write_cache(cache, &t);
            t
        }
        _ => read_table(cache).unwrap_or_default(),
    };
    match read_table(override_file) {
        Some(o) => base.overlaid(&o),
        None => base,
    }
}

fn xdg(var: &str, sub: &str) -> PathBuf {
    match std::env::var(var).ok().filter(|s| !s.trim().is_empty()) {
        Some(d) => PathBuf::from(d).join("rung"),
        None => std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(sub).join("rung"))
            .unwrap_or_else(|_| PathBuf::from(".rung")),
    }
}

fn fetch_listing(base_url: &str, api_key: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(format!("{}/models", base_url.trim_end_matches('/')));
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("model listing: status {}", resp.status()));
    }
    resp.text().map_err(|e| e.to_string())
}

/// The process's table, built once from the configured endpoint. Blocking
/// on first use (one listing request, ten second bound).
pub fn global(base_url: &str, api_key: &str) -> &'static FactsTable {
    static TABLE: OnceLock<FactsTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        build(
            || fetch_listing(base_url, api_key),
            &xdg("XDG_CACHE_HOME", ".cache").join("model-facts.json"),
            &xdg("XDG_CONFIG_HOME", ".config").join("model-facts.json"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"{"data":[
      {"id":"a/m","context_length":200000,"extra":{"url":"x"},
       "top_provider":{"max_completion_tokens":8192},
       "pricing":{"prompt":"0.000003","completion":"0.000015"}},
      {"id":"b/free","context_length":0,"pricing":{"prompt":"-1"}},
      {"context_length":5}]}"#;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rung-facts-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn listing_parses_facts_and_skips_junk() {
        let t = parse_listing(LISTING).unwrap();
        assert_eq!(t.models.len(), 2);
        let f = t.get("a/m").unwrap();
        assert_eq!(f.context_window, Some(200_000));
        assert_eq!(f.max_output, Some(8192));
        assert!((f.price_prompt_per_mtok.unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(t.window("b/free"), None);
        assert!(parse_listing("{}").is_err());
    }

    #[test]
    fn failed_listing_uses_last_good_cache() {
        let d = tmp("cache");
        let (c, o) = (d.join("c.json"), d.join("o.json"));
        let t = build(|| Ok(LISTING.into()), &c, &o);
        assert_eq!(t.window("a/m"), Some(200_000));
        let t = build(|| Err("down".into()), &c, &o);
        assert_eq!(t.window("a/m"), Some(200_000));
        let t = build(|| Ok("junk".into()), &c, &o);
        assert_eq!(t.window("a/m"), Some(200_000));
    }

    #[test]
    fn override_wins_and_strict_schema_refuses_extras() {
        let d = tmp("over");
        let (c, o) = (d.join("c.json"), d.join("o.json"));
        std::fs::write(
            &o,
            r#"{"models":{"a/m":{"context_window":1000},"z":{"context_window":9}}}"#,
        )
        .unwrap();
        let t = build(|| Ok(LISTING.into()), &c, &o);
        assert_eq!(t.window("a/m"), Some(1000));
        assert_eq!(t.get("a/m").unwrap().max_output, Some(8192));
        assert_eq!(t.window("z"), Some(9));
        std::fs::write(
            &o,
            r#"{"models":{"a/m":{"context_window":1,"base_url":"http://evil"}}}"#,
        )
        .unwrap();
        let t = build(|| Ok(LISTING.into()), &c, &o);
        assert_eq!(t.window("a/m"), Some(200_000));
    }

    #[test]
    fn epoch_budget_reads_window_or_falls_back() {
        let t = parse_listing(LISTING).unwrap();
        assert_eq!(t.epoch_budget("a/m", 0.5, 128_000), 100_000);
        assert_eq!(t.epoch_budget("unknown", 0.5, 128_000), 128_000);
    }
}
