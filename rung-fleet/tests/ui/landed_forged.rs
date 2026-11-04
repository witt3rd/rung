//! A `Landed` cannot be written down: it comes from reading a worktree or from
//! a one-time authority to discard.
use rung_fleet::landing::Landed;

fn main() {
    let _l = Landed { how: todo!() };
}
