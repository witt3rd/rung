//! `Unretained` is minted only by the retain step: a caller cannot report a
//! write failure the provider never had.

use rung_memory::Unreached;
use rung_memory::retain::Unretained;

fn fabricate(u: Unreached) -> Unretained {
    Unretained::new(u)
}

fn main() {}
