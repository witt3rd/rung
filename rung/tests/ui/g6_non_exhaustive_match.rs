//! `G6` — `StepOutcome` is an enum and every match site must be exhaustive.
//! Adding a summand breaks every eliminator at compile time, which is what
//! makes an "exactly n" vocabulary hold (`adding-a-summand-breaks-every-eliminator`).

use rung::ladder;

struct Spec;
struct Job;
struct Report;

ladder!(Work {
    Start(Spec) => Active(Job) => { Done(Report) | Abandoned }
} impl {
    active = |_start| { Active::new(Job) },
    step = |_active| { Ok(StepOutcome::Done(Done::new(Report))) },
});

fn main() {
    let active = work::active(work::Start::new(Spec));
    match work::step(active) {
        Ok(work::StepOutcome::Done(_)) => {}
        Err(_) => {}
        // `Abandoned` is not handled.
    }
}
