// A plan is built only by configuring from the operator's file: it has no
// public fields or constructor.
fn main() {
    let _ = rung_host::startup::Plan {
        state: std::path::PathBuf::from("/tmp/x"),
    };
}
