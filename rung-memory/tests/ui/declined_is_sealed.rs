//! `Declined` is minted only by the retain step: a caller cannot claim the
//! provider refused a note it never saw.

use rung_memory::Reason;
use rung_memory::retain::Declined;

fn fabricate(r: Reason) -> Declined {
    Declined::new(r)
}

fn main() {}
