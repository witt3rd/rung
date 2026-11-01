//! A `Spawn` cannot be built by struct literal either.
use rung_fleet::lifecycle::Spawn;
use rung_fleet::{Autonomy, Brief};

fn main() {
    let brief = Brief::new("o", "d", "p", Autonomy::Ask).unwrap();
    let _s = Spawn { brief, assignee: "".into() };
}
