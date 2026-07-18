# Q2 — Cross-crate provenance *(parked)*

**Status:** PARKED

**Question.** Once a token (`Work::Active`) crosses a crate boundary, the
receiving crate trusts it. Can that be sealed — and is a lighter mechanism than a
whole sub-crate possible?

**Why it's parked.** The mechanism is already known: emit the sealed types into a
sub-crate that only the macro controls, so even the defining crate cannot
fabricate. That closes it — at the cost of one crate per ladder. Lighter ideas
(sealed traits, a zero-sized provenance capability threaded through a run) don't
actually seal, because the capability can itself be minted in-crate. So this is
engineering-with-a-cost, not deep research, and YAGNI until a real multi-crate
architecture demands it.
