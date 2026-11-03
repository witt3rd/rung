//! `a-skipped-transition-does-not-compile`: an `Active` task cannot be built
//! from outside the ladder.
use rung_fleet::lifecycle::{delegation, Signal, Work};
use rung_fleet::{Autonomy, Brief};

fn main() {
    let brief = Brief::new("o", "d", "p", Autonomy::Ask).unwrap();
    let work = Work {
        brief,
        assignee: "w".into(),
        beats: 0,
        stalls: 0,
        hold: None,
        resolution: None,
        signal: Signal::Beat,
    };
    let _active = delegation::Active::new(work, delegation::Carry { task: "t".into() });
}
