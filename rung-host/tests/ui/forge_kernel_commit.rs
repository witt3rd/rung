// A host path that writes `kernel.commit` without the agent's tool: refused.
use rung_host::kernel::KernelEntry;

fn main() {
    let _forged = KernelEntry {
        kind: "kernel.commit",
        body: serde_json::json!({"project": "p1"}),
    };
}
