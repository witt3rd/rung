//! Retain takes a `Turnover`, and a `Turnover` is built only by
//! `Turnover::of`, which takes a `Completion`. A turn that was unverified,
//! unchecked, truncated or failed holds none, so it cannot become memory.

use rung_agent::memory::Turnover;

fn fabricate(observation: rung_memory::Observation) -> Turnover {
    Turnover { observation }
}

fn main() {}
