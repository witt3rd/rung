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
//!
//! ## No-silent-drop (`#[must_use]`)
//!
//! Every generated token — rungs, verdicts, `StepOutcome`, and `Failed` — is
//! `#[must_use]`. Rust types are affine (droppable); the linear-token contract is
//! "consumed *exactly* once". Move semantics give "at most once"; `#[must_use]`
//! guards "at least once". Dropping a token is a warning, and an error under
//! `#![deny(unused_must_use)]`.
//!
//! This is load-bearing: the verdict struct below is publicly constructible, so
//! dropping it under `deny(unused_must_use)` must fail to compile. If the
//! `#[must_use]` attribute were ever dropped from the macro's emit, this example
//! would start compiling and the `compile_fail` test would fail.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! use rung::ladder;
//! struct SpecData;
//! struct LoopData;
//! ladder!(Demo {
//!     Spec(SpecData) => Active(LoopData) => { Converged | Stalled => Active }
//!     recover { stalled: Stalled => Active }
//! });
//! fn abandons_the_outcome() {
//!     demo::Converged; // dropping a #[must_use] verdict — denied
//! }
//! ```

pub use rung_macro::ladder;
