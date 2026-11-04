//! Discarding work takes a one-time authority; a standing grant for it does not
//! exist.
use rung_fleet::authority::{act, Standing};
use rung_fleet::landing::Landed;

fn main() {}

fn discard(standing: Standing<act::Discard>) {
    let _l = Landed::scratch(standing);
}
