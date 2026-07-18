//! rung — a type ladder where the state machine IS the type system.
//!
//! Declare the ladder and its transition logic together. Bodies in the trailing
//! `impl { .. }` block expand *inside* the generated module, so they use the
//! sealed constructors and the macro auto-injects the recovery guard:
//!
//! ```rust,ignore
//! use rung::ladder;
//!
//! ladder!(Work {
//!     carry { task_id: String }
//!     Designed(WorkSpec) => Active(ActiveLoop) => {
//!         Complete | Stalled => Active | BudgetExhausted
//!     }
//!     recover { retry: Stalled => Active }
//! } impl {
//!     active = |designed| { Active::new(/* .. */, designed.carry().clone()) },
//!     step   = |active|   { Ok(StepOutcome::Complete(Complete::new())) },
//!     retry  = |stalled|  { let a = stalled.into_source(); Active::new(/* .. */, a.carry().clone()) },
//! });
//! // start:  let d = work::Designed::new(spec, carry);   // entry ctor is public
//! // drive:  match work::step(work::active(d)) { .. }     // module `pub fn`s
//! ```
//!
//! Omit the `impl { .. }` block for a type-only declaration (structs, verdict
//! enum, and guards, but no transition logic).
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
//!     demo::Converged::new(); // dropping a #[must_use] verdict — denied
//! }
//! ```
//!
//! ## No external fabrication (§4.1)
//!
//! With an inline `impl { .. }` block, only the *entry* rung has a public
//! constructor — every downstream rung's `new` is module-private, so no outside
//! code can mint a mid-ladder token. The following must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct SpecData;
//! #[derive(Clone, PartialEq)]
//! struct LoopData;
//! ladder!(Demo {
//!     Spec(SpecData) => Active(LoopData) => { Done | Retry => Active }
//!     recover { retry: Retry => Active }
//! } impl {
//!     active = |s| { Active::new(LoopData) },
//!     step   = |a| { Ok(StepOutcome::Done(Done::new())) },
//!     retry  = |r| { r.into_source() },
//! });
//! fn fabricate() {
//!     // `Active::new` is private to `demo` — cannot fabricate a mid-ladder rung.
//!     let _ = demo::Active::new(LoopData);
//! }
//! ```
//!
//! ## Terminal verdict payloads
//!
//! A terminal verdict may carry a result: `Converged(Report)` generates
//! `Converged { payload: Report }` with `.payload()` / `.into_payload()`, so a run
//! returns a value through the verdict. A *recoverable* verdict may not — it
//! carries its source rung instead — so the following must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S; struct L; struct Info;
//! ladder!(Bad {
//!     Spec(S) => Active(L) => { Done | Stalled(Info) => Active }
//!     recover { unstall: Stalled => Active }
//! });
//! ```
//!
//! ## Error-path recovery (`Failed(rung) => rung`)
//!
//! `recover { name: Failed(Active) => Active }` recovers from the error path: when
//! a branching transition returns `Err(Failed { token, error })`, this edge takes
//! the unconsumed `token` back and produces the next rung. No progress guard is
//! injected (a retry after a transient error may legitimately reuse the token).
//! The named rung must exist — this must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S; struct L;
//! ladder!(Bad {
//!     Start(S) => Working(L) => { Done }
//!     recover { clear: Failed(Nonexistent) => Working }
//! });
//! ```
//!
//! ## Continue arms (`Name -> Rung`)
//!
//! A branching arm written with `->` (produces) instead of `=>` (recover) is a
//! *continue* arm: `step` builds the next rung itself and the `StepOutcome` variant
//! carries it directly — no recover fn, no progress guard, no source-carrying.
//! `Tick -> Counting` gives `StepOutcome::Tick(Counting)`; the driver just
//! reassigns. The target rung must exist — this must fail to compile:
//!
//! ```compile_fail
//! use rung::ladder;
//! struct S;
//! ladder!(Bad {
//!     Begin(S) => Counting(i32) => { Tick -> Nonexistent | Done }
//! });
//! ```

pub use rung_macro::ladder;

// Compile-check and run the README's code blocks as doctests, so the README
// cannot silently drift from the macro. `#[cfg(doctest)]` means this item exists
// only during doctest builds — it never appears in the public API or on docs.rs.
// Illustrative README blocks are fenced ```rust,ignore; the Getting Started
// example is a complete ```rust program that is compiled and run.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
