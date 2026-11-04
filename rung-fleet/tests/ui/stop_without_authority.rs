//! `irreversible-action-needs-authority`: stop takes an authority token.
use rung_fleet::delivery::CorrelationId;

fn main() {
    let id = CorrelationId::new("c").unwrap();
    let _ = rung_fleet::steer::stop(std::path::Path::new("/x"), "m", "chief", id, "why");
}
