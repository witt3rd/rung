// Only the host settles an expectation: a settlement cannot be built
// outside the registers.
use rung_host::registers::Settlement;

fn main() {
    let _forged = Settlement {
        id: "e1".to_string(),
        body: serde_json::json!({"state": "met"}),
    };
}
