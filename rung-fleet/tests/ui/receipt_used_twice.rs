//! A receipt is consumed exactly once: recording it moves it.
use rung_fleet::delivery::Receipt;

fn twice(r: Receipt) {
    let _a = r.record();
    let _b = r.record();
}

fn main() {
    let _ = twice;
}
