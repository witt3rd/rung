//! rung — a type ladder where the state machine IS the type system.
//!
//! ```rust,ignore
//! use rung::ladder;
//!
//! ladder!(Work {
//!     carry { task_id: String, correlation_key: u64 }
//!     Designed(WorkSpec) => Claimed(Designed) => Active(ActiveLoop) => {
//!         Complete | Stalled => Active | BudgetExhausted
//!     }
//!     recover {
//!         claim_failed: Failed(Designed) => Designed(WorkSpec)
//!     }
//! });
//! ```

pub use rung_macro::ladder;
