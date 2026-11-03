//! `a-brief-is-a-typed-contract`: a brief cannot be built by struct literal
//! (which would skip validation); `Brief::new` is the only constructor.
use rung_fleet::{Autonomy, Brief};

fn main() {
    let _b = Brief {
        objective: "".into(),
        done_when: "d".into(),
        delivery: "p".into(),
        autonomy: Autonomy::Ask,
    };
}
