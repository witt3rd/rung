// The startup ladder cannot skip a stage: recovering the record takes a
// Listed token, and a Configured one is not it.
fn skip(configured: rung_host::startup::Configured) {
    let _ = rung_host::startup::recovered(configured);
}

fn main() {}
