//! rung-props.md §2 rule 3 — a recoverable verdict cannot declare a payload.
//!
//! A terminal verdict may carry a result (`Converged(Report)`), so a run
//! returns a value *through* the verdict. A recoverable verdict may not: it
//! already carries its source rung, which is what the recover edge consumes.
//!
//! The intended diagnostic is the macro's own error, quoted in full below.

use rung::ladder;

struct S;
struct L;
struct Info;

ladder!(Bad {
    Spec(S) => Active(L) => { Done | Stalled(Info) => Active }
    recover { unstall: Stalled => Active }
});

fn main() {}
