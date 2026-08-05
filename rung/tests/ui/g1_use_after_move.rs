//! `G1` — a transition consumes its input rung **by value**, so the caller
//! cannot use it afterwards. If the macro ever emitted `&self` or `Copy`, this
//! would compile and the guarantee would be gone with nothing to say so.

use rung::ladder;

struct Spec;
struct Job;

ladder!(Work {
    Start(Spec) => Active(Job) => { Done }
} impl {
    active = |_start| { Active::new(Job) },
    step = |_active| { Ok(StepOutcome::Done(Done::new())) },
});

fn main() {
    let start = work::Start::new(Spec);
    let _first = work::active(start);
    // `start` was consumed by the arrow above.
    let _second = work::active(start);
}
