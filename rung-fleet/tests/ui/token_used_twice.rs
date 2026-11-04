//! A one-time token is spent by being passed: it is not `Clone`.
use rung_fleet::authority::{act, Once};
use rung_fleet::delivery::CorrelationId;

fn main() {}

fn twice(token: Once<act::StopMember>) {
    let index = std::path::Path::new("/x");
    let a = CorrelationId::new("a").unwrap();
    let b = CorrelationId::new("b").unwrap();
    let _ = rung_fleet::steer::stop(index, "m", "chief", a, "why", token);
    let _ = rung_fleet::steer::stop(index, "m", "chief", b, "again", token);
}
