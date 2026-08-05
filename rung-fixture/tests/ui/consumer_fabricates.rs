//! A consumer crate trying to build a mid-ladder rung. `Active::new` is private
//! to the emitted module, which lives in another crate — so the seal crosses.

fn main() {
    let _ = rung_fixture::work::Active::new(rung_fixture::Receipt { processed: 99 });
}
