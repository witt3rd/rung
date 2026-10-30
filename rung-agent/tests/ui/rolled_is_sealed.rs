//! A `Compacted` is the payload of the `Rolled` verdict, which only the
//! Compaction step mints; a caller cannot claim a compaction it did not run.

use rung_agent::compact::Compacted;
use rung_agent::compact::compaction::Rolled;

fn fabricate(c: Compacted) -> Rolled {
    Rolled::new(c)
}

fn main() {}
