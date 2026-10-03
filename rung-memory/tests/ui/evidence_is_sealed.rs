//! `Evidence` is built only by the recall step, which never builds an empty
//! one. A caller cannot assemble evidence, empty or not.

use rung_memory::{Evidence, Trace};

fn fabricate(trace: Trace) -> Evidence {
    Evidence {
        items: Vec::new(),
        left_out: 0,
        trace,
    }
}

fn main() {}
