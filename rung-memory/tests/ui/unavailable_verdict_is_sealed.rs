//! `Unavailable` is minted only by the recall step: a caller cannot declare
//! memory unreachable (or reachable) without asking it.

use rung_memory::Unreached;
use rung_memory::recall::Unavailable;

fn fabricate(u: Unreached) -> Unavailable {
    Unavailable::new(u)
}

fn main() {}
