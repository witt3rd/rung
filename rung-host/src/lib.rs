//! rung-host — the continuous rung host: one agent that never rests.
//!
//! The host owns everything around a turn: stimuli and the inbox, time and
//! the calendar, the registers, the free-time kernel, the decision desk,
//! the layered context pack, memory, the governor, and the stop. It runs
//! one agent through `rung-agent-core`'s turn engine (imported as a
//! library, never the binary) or, in this slice, a scripted mock.
//!
//! See `docs/rung-host.md` for the design and the record vocabulary, and
//! [`gates`] for the frozen acceptance gates.

pub mod canon;
pub mod clock;
pub mod gates;
pub mod notify;
pub mod record;
pub mod stop;
