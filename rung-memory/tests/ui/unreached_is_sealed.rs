//! `Unreached` carries a trace and a reason; neither can be written by hand.

use rung_memory::{Trace, Unreached, Why};

fn fabricate(why: Why, trace: Trace) -> Unreached {
    Unreached { why, trace }
}

fn main() {}
