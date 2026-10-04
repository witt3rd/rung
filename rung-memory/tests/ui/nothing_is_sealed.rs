//! `Nothing` (the payload of `Empty`) cannot be assembled with a made-up
//! trace.

use rung_memory::{Nothing, Trace};

fn fabricate(trace: Trace) -> Nothing {
    Nothing { left_out: 0, trace }
}

fn main() {}
