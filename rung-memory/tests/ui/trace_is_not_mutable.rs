//! A measured `Trace` cannot be edited after the fact.

use rung_memory::Trace;

fn rewrite(mut t: Trace) {
    t.cost_usd = 0.0;
    t.calls = 0;
}

fn main() {}
