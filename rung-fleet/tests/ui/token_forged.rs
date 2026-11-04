//! A token cannot be written down: its fields are private, and the only way to
//! one is an owner item in the principal's record.
use rung_fleet::authority::{act, Once};

fn main() {
    let _t: Once<act::StopMember> = Once { id: "x".into(), _act: std::marker::PhantomData };
}
