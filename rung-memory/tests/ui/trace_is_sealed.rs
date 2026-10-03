//! A `Trace` is measured by a ladder step. A caller cannot write down a cost
//! or a call count of its own.

use rung_memory::Trace;

fn main() {
    let _ = Trace {
        calls: 0,
        cost_usd: 0.0,
        latency_ms: 0,
    };
}
