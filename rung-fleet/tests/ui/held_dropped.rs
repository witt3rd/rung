//! `held-and-stalled-are-recoverable`: a `Held` or `Stalled` verdict is never
//! dropped silently: each must be used (resumed or restarted).
#![deny(unused_must_use)]
use rung_fleet::lifecycle::delegation;

fn drops_held(h: delegation::Held) {
    h;
}

fn drops_stalled(s: delegation::Stalled) {
    s;
}

fn main() {
    let _ = (drops_held, drops_stalled);
}
