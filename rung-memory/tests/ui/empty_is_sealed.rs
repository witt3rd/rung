//! `Empty` is minted only by the recall step: a caller cannot claim memory
//! held nothing without asking it.

use rung_memory::Nothing;
use rung_memory::recall::Empty;

fn fabricate(n: Nothing) -> Empty {
    Empty::new(n)
}

fn main() {}
