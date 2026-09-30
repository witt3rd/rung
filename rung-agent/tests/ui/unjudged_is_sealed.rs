//! The switched-off witness cannot be minted outside the switch: a consumer
//! cannot claim the check is off in order to report "completed".

use rung_agent::turn_check::Unjudged;

fn main() {
    let _ = Unjudged { _seal: () }.completion();
}
