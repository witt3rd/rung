//! A `Checked` is the payload of the `Completed` verdict, which only the
//! TurnCheck step mints. A consumer holding the parts cannot build the
//! verdict itself.

use rung_agent::turn_check::Checked;
use rung_agent::turn_check::turncheck::Completed;

fn fabricate(c: Checked) -> Completed {
    Completed::new(c)
}

fn main() {}
