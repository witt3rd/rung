//! Retirement is reachable only from `Done`: a `Finished` cannot be forged.
use rung_fleet::lifecycle::{Completion, Finished};

fn main() {
    let _f = Finished { completion: Completion { summary: "x".into() } };
}
