//! An irreversible act has no standing grant: merge, discard, destructive and
//! security-sensitive work take a one-time token only.
use rung_fleet::authority::{act, Standing};

fn main() {
    let _t: Option<Standing<act::Merge>> = None;
}
