//! `a-brief-is-a-typed-contract`: a brief with a missing field does not compile.
use rung_fleet::Brief;

fn main() {
    let _b = Brief {
        objective: "o".into(),
        done_when: "d".into(),
        delivery: "p".into(),
    };
}
