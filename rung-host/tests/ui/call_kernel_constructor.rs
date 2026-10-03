// The sealed entry's constructors are private to the kernel module.
use rung_host::kernel::KernelEntry;

fn main() {
    let _forged = KernelEntry::release(serde_json::json!({"project": "p1"}));
}
