//! `Stored` is minted only by the retain step, after the provider reported
//! the commit. A caller cannot claim "remembered" without the receipt.

use rung_memory::Receipt;
use rung_memory::retain::Stored;

fn fabricate(r: Receipt) -> Stored {
    Stored::new(r)
}

fn main() {}
