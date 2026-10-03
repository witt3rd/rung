//! The fake world: owner and peer traffic (Poisson, plus bursts), world
//! facts, and a web of fast and slow pages. Every event is generated up
//! front from a seed, so a run is deterministic.

use std::collections::BTreeSet;

use serde_json::json;

use super::Rng;
use crate::clock::{Millis, SECOND};
use crate::inbox::{Fact, Item, ItemKind, Role, Source};
use crate::toolbox::WebReader;

#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub seed: u64,
    pub start: Millis,
    pub horizon: Millis,
    /// Owner messages per hour (Poisson).
    pub owner_per_hour: f64,
    /// Peer messages per hour, across `peers` peers.
    pub peer_per_hour: f64,
    pub peers: usize,
    /// Bursts: (at, count) — `count` peer messages within one second.
    pub bursts: Vec<(Millis, usize)>,
    /// World facts per hour; each sets one of `fact_keys` to a value.
    pub facts_per_hour: f64,
    pub fact_keys: Vec<String>,
}

impl WorldConfig {
    /// A quiet world: nothing arrives.
    pub fn quiet(seed: u64, start: Millis, horizon: Millis) -> Self {
        Self {
            seed,
            start,
            horizon,
            owner_per_hour: 0.0,
            peer_per_hour: 0.0,
            peers: 0,
            bursts: Vec::new(),
            facts_per_hour: 0.0,
            fact_keys: Vec::new(),
        }
    }
}

/// A source of scheduled world events.
#[derive(Debug)]
pub struct FakeWorld {
    events: Vec<Item>,
    next: usize,
}

fn poisson(rng: &mut Rng, start: Millis, horizon: Millis, per_hour: f64) -> Vec<Millis> {
    let mut out = Vec::new();
    if per_hour <= 0.0 {
        return out;
    }
    let mean_ms = 3_600_000.0 / per_hour;
    let mut t = start as f64;
    loop {
        t += -mean_ms * (1.0 - rng.f64()).ln();
        if t as Millis >= start + horizon {
            return out;
        }
        out.push(t as Millis);
    }
}

const WORDS: &[&str] = &[
    "build", "cache", "calendar", "draft", "review", "garden", "invoice", "paper", "release",
    "station", "weather", "music", "river", "lamp", "theory", "proof", "letter", "trip", "budget",
    "kernel", "sketch", "notebook", "harbor", "signal",
];

fn sentence(rng: &mut Rng, n: usize) -> String {
    (0..n)
        .map(|_| WORDS[rng.below(WORDS.len() as u64) as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

impl FakeWorld {
    pub fn new(cfg: &WorldConfig) -> Self {
        let mut rng = Rng::new(cfg.seed ^ 0x5eed_0f_0001);
        let mut events = Vec::new();
        for (k, at) in poisson(&mut rng, cfg.start, cfg.horizon, cfg.owner_per_hour)
            .into_iter()
            .enumerate()
        {
            let text = format!("owner asks: {}?", sentence(&mut rng, 6));
            events.push(Item::message(&format!("own-{k}"), Role::Owner, "owner", at, &text));
        }
        for (k, at) in poisson(&mut rng, cfg.start, cfg.horizon, cfg.peer_per_hour)
            .into_iter()
            .enumerate()
        {
            let p = rng.below(cfg.peers.max(1) as u64);
            let text = format!("peer {p} says: {}.", sentence(&mut rng, 8));
            events.push(Item::message(&format!("peer-{k}"), Role::Peer, &format!("peer:{p}"), at, &text));
        }
        for (b, (at, count)) in cfg.bursts.iter().enumerate() {
            for j in 0..*count {
                let p = rng.below(cfg.peers.max(1) as u64);
                let text = format!("burst {b} from peer {p}: {}.", sentence(&mut rng, 5));
                let t = at + (j as Millis * SECOND) / (*count as Millis).max(1);
                events.push(Item::message(&format!("burst-{b}-{j}"), Role::Peer, &format!("peer:{p}"), t, &text));
            }
        }
        if !cfg.fact_keys.is_empty() {
            for (k, at) in poisson(&mut rng, cfg.start, cfg.horizon, cfg.facts_per_hour)
                .into_iter()
                .enumerate()
            {
                let key = cfg.fact_keys[rng.below(cfg.fact_keys.len() as u64) as usize].clone();
                let value = if rng.f64() < 0.6 { "green" } else { "red" };
                events.push(Item {
                    id: format!("fact-{k}"),
                    kind: ItemKind::World,
                    role: Role::Host,
                    channel: "world".into(),
                    at,
                    due: None,
                    urgency: None,
                    firm: false,
                    text: format!("{key} is {value}"),
                    fact: Some(Fact {
                        key,
                        value: json!(value),
                    }),
                    control: None,
                });
            }
        }
        events.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
        Self { events, next: 0 }
    }

    /// Every event, in time order.
    pub fn events(&self) -> &[Item] {
        &self.events
    }
}

impl Source for FakeWorld {
    fn poll(&mut self, now: Millis, seen: &BTreeSet<String>) -> Vec<Item> {
        let mut out = Vec::new();
        while self.next < self.events.len() && self.events[self.next].at <= now {
            let e = self.events[self.next].clone();
            self.next += 1;
            if !seen.contains(&e.id) {
                out.push(e);
            }
        }
        out
    }

    fn next_at(&self) -> Option<Millis> {
        self.events.get(self.next).map(|e| e.at)
    }
}

/// The fake web: `slow://...` pages take ten minutes; anything else is
/// fast.
#[derive(Debug, Default)]
pub struct FakeWeb;

/// How long a slow page takes.
pub const SLOW_PAGE_MS: Millis = 10 * 60 * SECOND;

impl WebReader for FakeWeb {
    fn fetch(&self, url: &str) -> (Result<String, String>, Millis) {
        if url.starts_with("slow://") {
            (Ok(format!("the whole of {url}")), SLOW_PAGE_MS)
        } else {
            (Ok(format!("page {url}: a short text about {url}.")), 300)
        }
    }
}
