// Nothing outside the host appends to its record.
fn emit_outside(core: &rung_host::core::Core) {
    core.emit("kernel.progress", serde_json::json!({}));
}

fn main() {}
