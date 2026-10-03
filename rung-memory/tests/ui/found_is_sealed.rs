//! `Found` is minted only by the recall step, after the provider answered.
//! A caller holding evidence cannot wrap it as a verdict itself.

use rung_memory::Evidence;
use rung_memory::recall::Found;

fn fabricate(e: Evidence) -> Found {
    Found::new(e)
}

fn main() {}
