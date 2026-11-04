//! A token is for one act: a steer token does not stop a member.
use rung_fleet::authority::{act, Once};
use rung_fleet::delivery::CorrelationId;

fn main() {
    let token: Once<act::SteerMember> = unimplemented!();
    let id = CorrelationId::new("c").unwrap();
    let _ = rung_fleet::steer::stop(std::path::Path::new("/x"), "m", "chief", id, "why", token);
}
