// A mid-ladder rung has no public constructor: a Listed token can only come
// from listing a Configured one.
fn main() {
    let configured: rung_host::startup::Configured = todo!();
    let plan = configured.payload;
    let _ = rung_host::startup::Listed::new(plan, todo!());
}
