//! A `Compacted` has no public constructor and private fields, so the
//! record of a compaction cannot be written by hand.

use rung_agent::compact::{Compacted, Record};

fn main() {
    let _ = Compacted {
        lines: Vec::new(),
        record: Record { tokens_before: 0, tokens_after: 0, lines_folded: 0, lines_kept: 0 },
    };
}
