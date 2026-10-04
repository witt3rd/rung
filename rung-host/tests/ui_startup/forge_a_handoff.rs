// The handoff to the Presence loop is sealed: it is built only by the
// ladder's last step, from a recovered record.
fn mint(
    host: std::sync::Arc<rung_host::presence::Host>,
    recovered: rung_host::presence::Recovered,
    acp: rung_host::startup::AcpPlan,
) {
    let _ = rung_host::startup::Handoff {
        host,
        recovered,
        acp,
    };
}

fn main() {}
