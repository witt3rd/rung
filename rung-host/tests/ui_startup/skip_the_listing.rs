// The startup ladder cannot skip a stage: recovering the record takes a
// Listed token, and a Configured one is not it.
fn main() {
    let configured: rung_host::startup::Configured = todo!();
    let _ = rung_host::startup::recovered(configured);
}
