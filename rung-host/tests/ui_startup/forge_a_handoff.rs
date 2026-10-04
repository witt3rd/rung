// The handoff to the Presence loop is sealed: it is built only by the
// ladder's last step, from a recovered record.
fn main() {
    let _ = rung_host::startup::Handoff {
        host: todo!(),
        recovered: todo!(),
        acp: todo!(),
    };
}
