//! Calling a turn completed takes a `Completion`, and a `Completion` comes
//! only from a passed check (`Checked::completion`) or from the switch being
//! off (`Unjudged::completion`). There is no other constructor.

use rung_agent::run::Status;
use rung_agent::turn_check::Completion;

fn main() {
    let _ = Status::Completed(Completion::default());
}
