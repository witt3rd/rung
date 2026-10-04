//! `Receipt` (the payload of `Stored`) cannot be assembled by hand: a commit
//! the provider did not make has no receipt.

use rung_memory::{Receipt, RecordId, Trace};

fn fabricate(id: RecordId, trace: Trace) -> Receipt {
    Receipt { id, trace }
}

fn main() {}
