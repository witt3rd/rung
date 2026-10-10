//! rung-host — the continuous rung host: one agent that never rests.
//!
//! The host owns everything around a turn: stimuli and the inbox, time and
//! the calendar, the registers, the free-time kernel, the decision desk,
//! the layered context pack, memory, the governor, and the stop. It runs
//! one agent through `rung-agent-core`'s turn engine (imported as a
//! library, never the binary; [`adapter`]) or a scripted mock.
//!
//! See `docs/rung-host.md` for the design and the record vocabulary, and
//! [`gates`] for the frozen acceptance gates.

pub mod acp;
pub mod adapter;
pub mod calendar;
pub mod canon;
pub mod clock;
pub mod core;
pub mod desk;
pub mod engine;
pub mod gates;
pub mod governor;
pub mod inbox;
pub mod kernel;
pub mod ladder;
pub mod memory;
pub mod notify;
pub mod pack;
pub mod presence;
pub mod record;
pub mod registers;
pub mod render;
pub mod sim;
pub mod startup;
pub mod state;
pub mod statelock;
pub mod stop;
pub mod toolbox;
