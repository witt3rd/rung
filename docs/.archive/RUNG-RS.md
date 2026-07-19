# RUNG-RS — Design record for the `ladder!` proc macro

**2026-07-16 · Donald Thompson & Forge ⚒️**

> **Historical / non-normative.** This began as the design document for the
> `ladder!` macro (when it was not yet built) and grew into the running record of
> how it was built and how each gap was closed — the rationale, the coverage
> archaeology, and the roadmap. The **normative** description of the language now
> lives in [`SPEC.md`](../SPEC.md); read that for the rules. This document is kept
> for the *why* behind them.

A Rust proc macro that compiles the rung `ladder` primitive to the borrow-checker-enforced typestate pattern. The ladder syntax is the DSL. The macro is the compiler. Rust is the substrate.

---

## 1. The DSL — `ladder!` syntax

```
ladder Work {
    carry {
        task_id: String,
        correlation_key: Uuid,
    }

    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Schedulable(SchedulableWork)
      => Claimed(ClaimedWork)
      => Active(ActiveWork)
      => {
          Complete                    // terminal
          | Stalled => Active        // recoverable verdict with re-entry
          | BudgetExhausted          // terminal
      }

    recover {
        claim_failed: Failed(DesignedWork) => Designed(WorkSpec)
    }
}
```

### What each part means

| Construct | Semantics |
|---|---|
| `ladder Name { ... }` | Declares a sealed type ladder. Emits a module `name`. |
| `carry { field: Type, ... }` | Witness data inherited by every rung. Immutability is enforced: a private field exposed only through a `&Carry` accessor (§4.3). |
| `RungName(PayloadType)` | A rung. The payload type is the data the rung carries. |
| `=> NextRung(Payload)` | A forward transition. Consumes the left rung, produces the right. |
| `=> { V1 \| V2 => Target \| V3 }` | Verdict branching. Bare name = terminal. `Name(Payload)` = terminal carrying a result. `Name => Rung` = recoverable (carries its source rung; guarded). `Name -> Rung` = continue arm (step builds the next rung inline; variant carries it; no recover fn/guard). |
| `recover { name: Verdict => Rung }` | Verdict recovery: re-enter from a recoverable verdict (progress guard auto-injected). |
| `recover { name: Failed(Rung) => Rung }` | Error-path recovery: take the token back from `Failed` and retry (no progress guard). |

### What the macro generates

For `ladder Work { carry { .. } Designed(WorkSpec) => Active(ActiveWork) => { Complete | Stalled => Active | BudgetExhausted } recover { retry: Stalled => Active } }` the macro emits **a sealed module** `pub mod work` containing (this is what the code actually generates — verified by `rung/tests/end_to_end.rs`):

```rust
// witness data, cloned onto each rung
#[derive(Clone, Debug)]
pub struct Carry { pub task_id: String, /* ... */ }

// sealed, !Send + !Sync rung structs, each with a sealed constructor `::new`
pub struct Designed {
    _seal: (),
    _not_send: PhantomData<*const ()>,   // §4.6 — one token, one thread
    carry: Carry,                        // §4.3 — private, read via .carry()
    pub payload: WorkSpec,
}
impl Designed {
    pub fn new(payload: WorkSpec, carry: Carry) -> Self { /* seals the fields */ }
    pub fn carry(&self) -> &Carry { &self.carry }
}
// ...Active likewise. `::new` is the ONLY way to mint a rung (the seal fields are
// private) and the bridge transition bodies use to build the rung they return.

// verdict structs — sealed & !Send like rungs. A *recoverable* verdict carries
// the rung it came from, so its recover edge has full context to re-enter.
pub struct Complete { _seal: (), _not_send: PhantomData<*const ()> }          // terminal
impl Complete { pub fn new() -> Self { /* ... */ } }
pub struct Stalled { _seal: (), _not_send: PhantomData<*const ()>, source: Active } // recoverable
impl Stalled {
    pub fn new(source: Active) -> Self { /* ... */ }
    pub fn source(&self) -> &Active { &self.source }
    pub fn into_source(self) -> Active { self.source }
}

// verdicts as an enum — exhaustive match enforced
pub enum StepOutcome { Complete(Complete), Stalled(Stalled), BudgetExhausted(BudgetExhausted) }

// error payload — carries the unconsumed token
pub struct Failed<Prev> { pub token: Prev, pub error: String }

// recovery-progress guard (§4.4) — auto-injected around recover bodies
pub fn must_progress<T: PartialEq>(before: &T, after: &T) { /* panics if equal */ }

// transition + recover LOGIC — emitted as module `pub fn`s from the inline
// `impl { .. }` block (see below). `Designed::new` (entry) is `pub`; every other
// `::new` is module-private, so only these in-module bodies can build rungs (§4.1).
pub fn active(t: Designed) -> Active { /* your body */ }
pub fn step(t: Active) -> Result<StepOutcome, Failed<Active>> { /* your body */ }
pub fn retry(t: Stalled) -> Active { /* your body, wrapped with must_progress */ }
```

> All `#[must_use]` / `PhantomData` markers are elided above for readability — see §4.6, §4.7. To *stay on* a rung, use a continue arm (`Iterate -> Active`): `step` builds the next rung inline and the `StepOutcome` variant carries it directly (no recover fn, no guard) — distinct from a recoverable verdict `Stalled => Active`, which hands off to a guarded recover fn.

### Transition bodies — the inline `impl { .. }` form

Bodies are written inline and expand **inside** the module, so they use the sealed (private) constructors directly and the macro auto-injects the recovery guard. This is the **only** body mechanism (`rung/tests/end_to_end.rs` and `rust-example/` both drive full runs this way):

```rust
ladder!(Work {
    carry { task_id: String }
    Designed(WorkSpec) => Active(ActiveWork) => { Complete | Stalled => Active | BudgetExhausted }
    recover { retry: Stalled => Active }
} impl {
    active = |designed| { Active::new(/* next payload */, designed.carry().clone()) },
    step   = |active|   {
        // ...decide the verdict; refer to types unqualified (Active, Stalled, ...)...
        Ok(StepOutcome::Stalled(Stalled::new(active)))   // carry source rung
    },
    retry  = |stalled|  {
        let prev = stalled.into_source();
        Active::new(/* advanced */, prev.carry().clone())
        // no must_progress call — the macro wraps this body with it (§4.4)
    },
});
// start:  let d = work::Designed::new(spec, carry);   // entry ctor is public
// drive:  match work::step(work::active(d)) { StepOutcome::Stalled(s) => work::retry(s), .. }
```

Inside the block, refer to rungs/verdicts unqualified (`Active`, `Stalled`, `StepOutcome`); payload types resolve from the surrounding scope (`use super::*`). Omit the `impl { .. }` block entirely for a **type-only** declaration (structs, enum, and guards — no logic; all constructors are `pub` so a hand-written driver can build them).

There is no `Transitions` trait: one API surface. A separate-compilation trait form could be re-added as an opt-in if a real workload needs it.

---

## 2. The macro — expansion logic

The proc macro performs the same 8 structural checks as the Python rung checker at compile time (plus 2 more when an inline `impl { .. }` block is present: every body names a real transition/recover fn, and every transition/recover fn has a body), then expands to Rust code:

1. **Parse** the `ladder!` token stream into an AST (identical to `rung/ast.py`), plus the optional trailing `impl { .. }` bodies
2. **Check** the AST (8 rules mirroring `rung/checker.py`, + 2 impl-block rules, single pass)
3. **Emit** the Rust module

### Checks performed by the macro (compile-time refusals)

| # | Check | Error if |
|---|---|---|
| 1 | Carry fields are distinct | Duplicate field name |
| 2 | Transitions reference declared rungs | `from_rung` or `to_rung` not in `rungs` list |
| 3 | Verdicts are valid | Non-terminal verdict with no target; `recover_target`/`continue_target` not a declared rung; a recoverable verdict or a continue arm declaring a payload |
| 4 | Every recoverable verdict has a RecoverEdge | `\| Stalled => Active` but no `recover { ... }` entry for `Stalled` (continue arms excluded) |
| 5 | Every RecoverEdge has a matching function | `recover { x: ... }` declared but no `recover x` impl |
| 6 | Terminal verdicts have no recover edges | `Complete` is terminal but `recover { complete: ... }` exists |
| 7 | RecoverEdge references a known verdict | `recover { phantom: ... }` but `phantom` not in any verdict list |
| 8 | Recover targets are declared | `recover ... -> Missing`, or `Failed(Missing)`, where `Missing` is not a rung |
| 9 | Inline bodies name real fns | An `impl { .. }` body that matches no transition/recover fn (or is defined twice) |
| 10 | Inline bodies are complete | A transition/recover fn with no body in the `impl { .. }` block |

Checks 1–8 mirror the Python PoC (extended in-place for payloads, continue arms, and error-path recovery); 9–10 apply when an inline `impl { .. }` block is present. The macro fails with a compiler error pointing at the specific violation — same "compiler as cryptographic signature" property. The payload, error-path, and continue-arm extensions each have a `compile_fail` doctest in `rung/src/lib.rs`.

---

## 3. What is covered

### By the borrow checker (Rust's native enforcement)

| Constraint | Mechanism |
|---|---|
| Linear consumption | `fn activate(spec: Spec) -> Active` — takes `Spec` by value. Use-after-move is a compile error. |
| Error path returns token | `fn step(active: Active) -> Result<..., Failed<Active>>` — `Err(Failed { token: active, ... })` returns the unconsumed token. |
| Exhaustive match | `StepOutcome` enum. Every match site must handle all variants. Adding a variant breaks all callers. |
| No shared mutable state | `Active` is not `Clone`. Cannot duplicate the token. |
| Carry as ordinary data | `carry: Carry` is a private struct field (read via `.carry()`). Rust's ownership rules handle whether it's shared or copied. |

### By the proc macro (compile-time structural checks)

| Constraint | Mechanism |
|---|---|
| Sealed constructors | `_seal: ()` + `_not_send` private fields. External struct literals impossible. Rungs are minted only through `Rung::new` — public for the entry rung, module-private for every other (so only in-module transition bodies build them; §4.1). |
| Rung existence | All transitions reference declared rungs — checked at expansion time. |
| Recover pairing | Every `\| Stalled => Active` has a matching `recover { stalled: Stalled => Active }` — checked at expansion time. |
| Terminal vs recoverable | `Complete` (terminal) cannot have a recover edge — checked at expansion time. |
| Duplicate carry fields | Rejected at expansion time. |
| Recover function signatures match | `recover fn stalled(stalled: Stalled) -> Active` signature must match the declared edge — checked at expansion time. |

### By convention (not enforced)

| Constraint | Status |
|---|---|
| Transition body correctness | The type proves the function was *called*, not that it *did the right thing*. |

---

## 4. What is NOT covered

### 4.1 In-module fabrication — CLOSED (inline-closure form)

The seal fields (`_seal`, `_not_send`) are private to the generated module, so no outside code can write a rung struct literal. The open question was construction: transition bodies must build the rungs they return, so *somewhere* a constructor has to exist.

**Why the gap existed:** the trait form put transition bodies in a user `impl Transitions` **outside** the module, forcing `Rung::new` to be `pub` so the bodies could reach it — which also let any code in the crate fabricate a mid-ladder `Active::new(..)`.

**Status:** Closed 2026-07-17 by the **inline-closure form** (§1). Bodies now expand *inside* the module, so the macro makes every constructor **module-private except the entry rung's** (the entry `::new` stays `pub` so a run can start). External code cannot construct a mid-ladder rung — it can only obtain one by calling the public transition functions. Proven by the `compile_fail` doctest in `rung/src/lib.rs` (`demo::Active::new(..)` from outside fails with **E0624: associated function `new` is private**; verified via probe to fail for that reason and no other). The trait form was removed — one API surface, no `pub` constructor to widen the hole.

**Residual:** the entry constructor is public by necessity (you must be able to start a run), and code *inside* the module can still fabricate — the module-boundary limit Rust always has. Fully sealing even the entry (cross-crate provenance) is §4.5's sub-crate fix.

### 4.2 Transition body correctness

The type proves `claim()` was called. It does not prove the claim was *valid* — that the resource was available, that the policy allowed it, that the return value reflects reality. A `claim()` that immediately returns `Ok(Claimed { ... })` without checking compiles cleanly.

**Why this gap exists:** This is the boundary between typestate and formal verification. Typestate enforces the graph. Verification enforces the behavior inside the nodes. Proving that `claim()` actually checked availability requires either dependent types (the return type depends on the availability value) or external verification (property-based testing, model checking).

**Path to close:** Property-based testing with `proptest`. Generate sequences of ladder operations, verify invariants hold. "After `claim()`, the resource is marked as claimed." This doesn't close the gap at compile time, but it catches regressions. For the truly paranoid: run a model checker against the transition implementations.

### 4.3 Carry immutability — CLOSED

The carry is supposed to be read-only witness data. The `carry { ... }` declaration now generates a **private** field with a `pub fn carry(&self) -> &Carry` accessor. Transitions call `token.carry()` to read witness data; the immutable reference prevents accidental mutation. The `+transfer` annotation for fields that need to move out of the carry is a future extension.

**Status:** Closed 2026-07-17. Private field + `&Carry` accessor, generated by the macro, zero build cost.

### 4.4 Recovery progress — CLOSED (guard provided)

`retry(stalled: Stalled) -> Active` produces a new `Active`. Nothing in the *type* verifies that the new `Active` is *different* from the one that stalled. An infinite stall loop — where `step` returns `Stalled`, `retry` returns an identical `Active`, and `step` stalls again — compiles and (without a guard) runs forever.

**Status:** Closed 2026-07-17, and **enforced** (not by convention). Three macro-emitted pieces:
1. A **recoverable verdict carries its source rung** (`Stalled { source: Active }`, with `.source()` / `.into_source()`), so there is a `before` state to compare against — the earlier unit verdict discarded it.
2. The macro emits **`must_progress<T: PartialEq>(before, after)`**, which panics if the recovered value equals its source.
3. With the inline-closure form (§1), the macro **wraps every recover body** with the guard automatically: it snapshots `source().payload` before the body runs and asserts against the produced rung's payload afterward. The recover body cannot skip it.

Proven by `rung/tests/end_to_end.rs::recover_guard_is_auto_injected` — a recover body with **no** `must_progress` call still panics on a no-progress recovery, because the macro injected the check.

**Requirement / limit:** the guard compares the source rung's payload to the produced rung's payload, so recover-target payloads must be `Clone + PartialEq`, and the source and target rungs must share a payload type (true for retry loops `A => .. => A`). It is a *runtime* guard (a liveness property; typestate proves safety, not liveness) — it catches the dominant infinite-stall bug, not all of them.

### 4.5 Cross-crate provenance

If crate A defines `ladder Work`, crate B can receive `Work::Active` and call `Deployment::stage(active)`. The type is correct. But did that `Active` come from a valid ladder run in crate A, or was it fabricated inside crate A's module? Crate B has no way to know.

**Why this gap exists:** Rust's visibility boundary is crate-level. Once a type crosses a crate boundary, the receiving crate trusts the sending crate's API. This is the standard Rust trust model — crates trust each other's public APIs.

**Path to close:** If the gap matters for a specific architecture, emit the sealed types into a dedicated sub-crate that *only* the macro controls. No other code in the defining crate can access the seal field. This is the same fix as §4.1, and carries the same build-complexity cost.

### 4.6 Concurrent access — CLOSED (default `!Send + !Sync`)

Move semantics prevent use-after-move for owned values. But `Arc<Active>` or a `&Active` reference circumvents ownership. Two threads could call `step(active)` on the same logical token if they share a reference.

**Why the gap existed:** Rust's ownership system prevents data races, not logical races. `Arc<Active>` is memory-safe — two threads can read the same `Active` — but the ladder contract (one token, one consumer) is violated.

**Status:** Closed 2026-07-17. Every generated rung struct now carries a private `_not_send: PhantomData<*const ()>` field, making it `!Send + !Sync`. An `Arc<Active>` or `&Active` can no longer cross a thread boundary, so the one-token-one-consumer contract holds even under shared references. Zero build cost — one field per rung struct. Proven by `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` (autoref specialization; the assert flips if the marker is ever removed).

**Verdict remnant — now closed:** verdict structs were previously emitted as bare `pub struct Stalled;` (no `_seal`, no `PhantomData`, publicly constructible, `Send`). As of 2026-07-17 they carry the same seal + `_not_send` marker as rungs and are built through a sealed `::new`, so verdicts are `!Send + !Sync` too. Proven by the two verdict assertions in `test_rungs_are_not_send_or_sync`.

**Future extension:** a ladder-level `parallel` annotation would drop the marker for genuinely multi-threaded pipelines and instead emit an `AtomicBool consumed` runtime double-consume guard. Not yet built — the safe default is free; the dangerous case should be explicit and pay for its own check.

### 4.7 Silent drop — CLOSED (default `#[must_use]`)

Rust types are **affine**, not linear: any value may be silently dropped. Move semantics enforce the "at most once" half of the linear-token contract (§4.6 closes the shared-reference escape). But "at least once" — no silent abandonment — was completely unguarded. A live `Active` that falls out of scope without being advanced or returned in a `Failed` is a ladder run that vanishes: no verdict, no completion, no error. Arguably worse than double-consume, because it is invisible.

**Why the gap existed:** affine types permit silent `Drop`. True no-silent-drop needs language-level linear types, which Rust does not have.

**Status:** Closed 2026-07-17. Every generated token — rung structs, verdict structs, `StepOutcome`, and `Failed<Prev>` — now carries `#[must_use]` with a contract-specific message. Dropping a token is a warning, and a hard error under `#![deny(unused_must_use)]`. Zero build cost — one attribute per emitted type. Proven by the `compile_fail` doctest in `rung/src/lib.rs` (drops a verdict under `deny(unused_must_use)`; verified via probe to fail *specifically* on `unused_must_use`, not an incidental error — if the attribute is ever removed from emit, the example compiles and the `compile_fail` test fails).

This is the pragmatic ~80% of no-silent-drop available today. It does not stop `std::mem::forget` or an explicit `let _ = active;`, and `#[must_use]` on a type only fires when the value is dropped in statement position — but it catches the dominant accidental-abandonment case at compile time without waiting on linear types (§6 long-term).

---

## 5. Coverage matrix

| Constraint | Rust (borrow checker) | rung checker (Python) | `ladder!` macro |
|---|---|---|---|
| Sealed constructors | Module-level | — | ✓ Module-level |
| Linear consumption | ✓ Move semantics | — | ✓ Inherited from Rust |
| Error path returns token | ✓ `Failed<Prev>` carries token | — | ✓ Inherited from Rust |
| Exhaustive match | ✓ Enum exhaustiveness | — | ✓ Inherited from Rust |
| Rung existence | — | ✓ | ✓ At expansion time |
| Recover pairing | — | ✓ | ✓ At expansion time |
| Terminal vs recoverable | — | ✓ | ✓ At expansion time |
| Terminal verdict result payload | — | — | ✓ `Converged(Report)` → `.payload()` |
| Carry syntax | — | — | ✓ Emitted as struct field |
| Carry read-only | — | — | ✓ Private field + `&Carry` getter |
| Drivable end-to-end | — | — | ✓ Inline `impl { .. }` bodies + entry `::new` + module `pub fn`s |
| In-module fabrication | — | — | ✓ Non-entry `::new` module-private (bodies inside module) |
| Recovery progress | — | — | ✓ `must_progress` **auto-injected** into verdict-recover bodies |
| Error-path recovery | — | — | ✓ `recover { .. : Failed(rung) => rung }` (no guard) |
| Continue / self-loop arm | — | — | ✓ `Name -> Rung` (step builds next rung inline) |
| Cross-crate provenance | — | — | ✗ Crate boundary trust |
| Concurrent access | — | — | ✓ `!Send + !Sync` by default (`PhantomData<*const ()>`) |
| No silent drop | — affine, drop allowed | — | ✓ `#[must_use]` on every token (warn; error under `deny`) |
| Transition body correctness | — | — | ✗ Formal verification gap |

---

## 6. Getting closer to full coverage

### Short-term (proc macro v1)

- **Carry: split `carry` and `carry_mut`.** Fields in `carry` generate immutable accessors. Fields in an optional `carry_mut` block allow mutation. The common case (witness data) is read-only by default.
- **Recovery progress: `must_progress`.** ✅ Done and **auto-injected**. The macro wraps every recover body with the guard (inline-closure form), so it cannot be skipped. Recoverable verdicts carry their source rung to supply the `before`. Panics on no-progress; catches the common infinite-stall bug.
- **Inline-closure form + `Transitions` trait removal.** ✅ Done. Bodies live inside the module (closing §4.1), one API surface. A separate-compilation trait form can be re-added as opt-in if a real workload needs it.
- **Concurrent access: `!Send + !Sync`.** ✅ Done. All rung types carry `PhantomData<*const ()>` by default. Opt-in to `Send` with a ladder-level annotation `parallel` (future) for use cases that genuinely need multi-threaded access.
- **No silent drop: `#[must_use]`.** ✅ Done. Every emitted token (rungs, verdicts, `StepOutcome`, `Failed`) is `#[must_use]`. Dropping a token warns, and errors under `#![deny(unused_must_use)]`. The pragmatic partial close of the no-silent-drop gap — no linear types required.

### Medium-term (proc macro v2)

- **Transition body property tests.** The macro emits a `proptest` harness for the ladder. Generates random sequences of transitions and verifies invariants: "after claim, resource is marked claimed," "recovery always produces a valid next rung," "no panic in any reachable state." User provides the invariants as assertions in the transition bodies.
- **Cross-crate provenance via sub-crate emission.** Optional `--seal-crate` flag that emits the sealed types into a dedicated sub-crate. The defining crate cannot access the seal field. Cross-crate trust becomes compiler-enforced.

### Long-term (language-level)

- **Linear types in Rust.** `#[must_use]` (§4.7) closes the *common* case of the no-silent-drop gap — it warns (or errors under `deny`) on an abandoned token. But it is escapable: `std::mem::forget`, an explicit `let _ = active;`, or storing the token in a dropped container all bypass it. If Rust ever gains true linear types (not just affine), the `ladder!` macro could leverage them directly: a linear `Active` *cannot* be dropped at all — it must be consumed by a transition or returned in a `Failed`. That would close the gap at the language level with no escape hatch.
- **Dependent types for transition contracts.** A dependently typed systems language (Idris, Lean, a future Rust extension) could encode: "`claim()` returns `Ok(Claimed)` only if the resource was available." The return type depends on the runtime value. This closes the transition-body-correctness gap. But the ceremony cost must fall below the value for this to be practical outside proof-carrying code.

---

## 7. Status

| Artifact | Status |
|---|---|
| `rung/ast.py` | Done — AST nodes for ladder, carry, transition, recover, verdict |
| `rung/checker.py` | Done — 8 static checks, single-pass, 11 tests. **Verified in sync** with the Rust `check()` (audited 2026-07-17; all 8 rules map 1:1). Docstring mentions a "reachability" check that was never implemented in either — stale comment only. |
| `rung/interpreter.py` | Done — linear token tracking, provenance trace |
| `rust-example/` | Done — **ported to the macro** (2026-07-17). The hand-written `mod metric_opt` is deleted; the crate now drives a `ladder!`-generated MetricOptimization via the inline `impl { .. }` form. Workspace member. |
| `ladder!` proc macro | Done — `rung-macro/src/lib.rs`: parse (+ optional inline `impl` block) + 10 checks (8 structural + 2 impl-block) + emit (sealed `!Send` rungs & verdicts, sealed constructors, carry accessor, `Failed<Prev>`, verdict enum, auto-injected `must_progress` guard, inline transition/recover `pub fn`s) |
| End-to-end drivability | Done — `rung/tests/end_to_end.rs`: starts a run via the public entry `::new`, steps it, takes the recover edge, reaches a terminal verdict; plus load-bearing proofs that §4.1 fabrication fails to compile (E0624) and §4.4's guard is auto-injected (panics with no explicit call) |

**Findings surfaced by the `rust-example` port:**
- **Terminal verdict payloads — CLOSED (2026-07-17).** A terminal verdict may carry a result: `Converged(Report)` generates `Converged { payload: Report }` with `.payload()` / `.into_payload()`, so a run returns a value *through* the verdict. A recoverable verdict may not (it carries its source rung) — enforced by a check, proven by a `compile_fail` doctest. `drives_to_convergence` asserts the returned `Report`.
- **Error-path recovery — CLOSED (2026-07-17).** `recover { name: Failed(Active) => Active }` recovers from the `Err(Failed { .. })` path: it takes the unconsumed token back and produces the next rung, with no progress guard (a transient-error retry may reuse the token). Proven by `end_to_end.rs::recovers_from_the_failed_error_path`; the `rust-example` failure-injection demo (dropped in the port) is **restored** on top of it. `Failed(rung)` must name a declared rung — checked, `compile_fail` doctest.
- **Continue arm — CLOSED (2026-07-18).** A branching arm `Name -> Rung` (`->` = produces, vs `=>` = recover) is a first-class continue: `step` builds the next rung inline and the `StepOutcome` variant carries it directly — no recover fn, no guard, no source. `end_to_end.rs::continue_arm_loops_without_a_recover_fn` proves the driver just reassigns; `rust-example` was simplified onto it, **deleting the `advance` recover fn entirely**. Undeclared target rung is a compile error (`compile_fail` doctest).

**Bonus fix (2026-07-17):** generated transition/recover fns no longer emit `unused_braces` warnings — the closure body is used as the fn body directly (or as an initializer), and the closure's argument pattern becomes the fn parameter. Zero warnings workspace-wide.

**All three port findings are now closed.** Open (graded): §4.2 transition-body correctness (proptest/verification); §4.5 cross-crate provenance (sub-crate) — the two hard research gaps. §4.1, §4.3, §4.4, §4.6, §4.7 + terminal-payloads + error-path-recovery + continue-arm closed.

---

*Festina lente. Cut the root. Chesterton's Fence. Descend.*