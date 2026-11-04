//! A standing grant cannot be passed where a one-time one is asked: it does
//! not even exist for an irreversible act, and a standing grant for another
//! act is not accepted either.
use rung_fleet::authority::{act, Granted, Standing};
use rung_fleet::delivery::CorrelationId;

fn main() {
    let g: Granted = unimplemented!();
    let standing: Standing<act::SteerMember> = g.standing("p", "r").unwrap();
    let id = CorrelationId::new("c").unwrap();
    let _ = rung_fleet::steer::stop(std::path::Path::new("/x"), "m", "chief", id, "why", standing);
}
