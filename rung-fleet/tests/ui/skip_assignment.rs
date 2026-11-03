//! `a-skipped-transition-does-not-compile`: a spawned task cannot be handed to
//! the activation without going through assignment.
use rung_fleet::lifecycle::{delegation, Spawn};
use rung_fleet::{Autonomy, Brief};

fn main() {
    let brief = Brief::new("o", "d", "p", Autonomy::Ask).unwrap();
    let spawned = delegation::Spawned::new(
        Spawn::new(brief, "w").unwrap(),
        delegation::Carry { task: "t".into() },
    );
    let _active = delegation::active(spawned);
}
