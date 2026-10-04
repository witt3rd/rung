//! The constructor that measures a `Trace` is private: only a ladder step
//! can call it.

use rung_memory::Trace;

fn main() {
    let t: Trace = Trace::of(1, 0.0, std::time::Instant::now());
    let _ = t;
}
