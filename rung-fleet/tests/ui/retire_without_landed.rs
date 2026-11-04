//! `unlanded-work-is-never-retired`: a `Finished`, which retirement consumes,
//! cannot be made from a `Done` alone; it takes proof the work landed.
use rung_fleet::lifecycle::{delegation, Finished};

fn main() {}

fn retire(done: delegation::Done) {
    let _f = Finished::from_done(done);
}
