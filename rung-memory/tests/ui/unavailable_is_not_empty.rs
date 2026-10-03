//! A recall's outcome has three arms. A match that handles found and empty
//! and forgets unavailable does not compile: a store that could not be read
//! is never taken for one that holds nothing.

use rung_memory::recall::StepOutcome;

fn records(o: StepOutcome) -> usize {
    match o {
        StepOutcome::Found(f) => f.into_payload().items().len(),
        StepOutcome::Empty(_) => 0,
    }
}

fn main() {}
