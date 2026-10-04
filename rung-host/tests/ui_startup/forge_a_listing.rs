// A mid-ladder rung has no public constructor: a Listed token can only come
// from listing a Configured one.
fn mint(configured: rung_host::startup::Configured, carry: rung_host::startup::Carry) {
    let _ = rung_host::startup::Listed::new(configured.payload, carry);
}

fn main() {}
