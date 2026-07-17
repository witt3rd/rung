# RUNG-RUST — The `ladder!` proc macro

**2026-07-16 · Donald Thompson & Forge ⚒️**

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
| `carry { field: Type, ... }` | Witness data inherited by every rung. Immutable by convention, read-only in transitions. |
| `RungName(PayloadType)` | A rung. The payload type is the data the rung carries. |
| `=> NextRung(Payload)` | A forward transition. Consumes the left rung, produces the right. |
| `=> { V1 \| V2 => Target \| V3 }` | Verdict branching. Bare name = terminal. `\| Name => Rung` = recoverable. |
| `recover { name: FromType => ToRung(ToPayload) }` | Recovery edges. Declared separately because they have different semantics (re-entry, not advance). |

### What the macro generates

For `ladder Work { Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveWork) }`:

**A sealed module** — `pub mod work` containing:

```rust
// sealed structs — _seal field prevents external construction
pub struct Designed { _seal: (), pub carry: Carry, pub payload: WorkSpec }
pub struct Claimed  { _seal: (), pub carry: Carry, pub payload: DesignedWork }
pub struct Active   { _seal: (), pub carry: Carry, pub payload: ActiveWork }

// verdicts as an enum — exhaustive match enforced
pub enum StepOutcome {
    Continue(Active),
    Complete(Complete),
    Stalled(Stalled),
    BudgetExhausted(BudgetExhausted),
}

// error payload — carries unconsumed token
pub struct Failed<Prev> { pub token: Prev, pub error: String }

// sealed constructors — the only way to create these types
pub fn design(spec: WorkSpec, carry: Carry) -> Designed { ... }
pub fn claim(designed: Designed) -> Result<Claimed, Failed<Designed>> { ... }
pub fn activate(claimed: Claimed) -> Result<StepOutcome, Failed<Claimed>> { ... }

// recovery — declared edges, paired with recover functions
pub fn recover_claim_failed(failed: Failed<Designed>) -> Designed { ... }
pub fn recover_stalled(stalled: Stalled) -> Active { ... }
```

The transition bodies are **user-provided**. The macro generates the types, the sealed constructors, the exhaustive enum, the error payload, and the recover function signatures. The user writes `impl_block!` or equivalent to provide the transition logic.

### User-written transition bodies

The macro emits the signatures; the user writes the bodies. Two plausible approaches:

**A. Trait implementation:**

```rust
impl work::Transitions for WorkCtx {
    fn claim(designed: Designed) -> Result<Claimed, Failed<Designed>> {
        // user writes this
    }
    fn activate(claimed: Claimed) -> Result<StepOutcome, Failed<Claimed>> {
        // user writes this
    }
    fn recover_claim_failed(failed: Failed<Designed>) -> Designed {
        // user writes this
    }
}
```

**B. Inline closures at the ladder declaration site:**

```rust
ladder!(Work {
    carry { task_id: String }
    Designed(WorkSpec) => Claimed(Designed) => Active => { Complete | Stalled => Active }
}) {
    transition claim = |designed| -> Result<Claimed, Failed<Designed>> { ... };
    transition activate = |claimed| -> Result<StepOutcome, Failed<Claimed>> { ... };
    recover stalled = |stalled| -> Active { ... };
}
```

Approach A is cleaner for separate compilation; approach B is cleaner for single-file use. The macro supports both.

---

## 2. The macro — expansion logic

The proc macro performs the same checks as the Python rung checker (8 static rules) at compile time, then expands to Rust code:

1. **Parse** the `ladder!` token stream into an AST (identical to `rung/ast.py`)
2. **Check** the AST (identical to `rung/checker.py` — 8 rules, single pass)
3. **Emit** the Rust module

### Checks performed by the macro (compile-time refusals)

| # | Check | Error if |
|---|---|---|
| 1 | Carry fields are distinct | Duplicate field name |
| 2 | Transitions reference declared rungs | `from_rung` or `to_rung` not in `rungs` list |
| 3 | Verdicts are valid | Non-terminal verdict with no `recover_target`, or `recover_target` not a declared rung |
| 4 | Every recoverable verdict has a RecoverEdge | `\| Stalled => Active` but no `recover { ... }` entry for `Stalled` |
| 5 | Every RecoverEdge has a matching function | `recover { x: ... }` declared but no `recover x` impl |
| 6 | Terminal verdicts have no recover edges | `Complete` is terminal but `recover { complete: ... }` exists |
| 7 | RecoverEdge references a known verdict | `recover { phantom: ... }` but `phantom` not in any verdict list |
| 8 | Recover function return_rung is declared | `recover fn ... -> Missing` but `Missing` not a rung |

These are the same 8 checks the Python PoC verifies. The macro fails with a compiler error pointing at the specific violation — same "compiler as cryptographic signature" property.

---

## 3. What is covered

### By the borrow checker (Rust's native enforcement)

| Constraint | Mechanism |
|---|---|
| Linear consumption | `fn activate(spec: Spec) -> Active` — takes `Spec` by value. Use-after-move is a compile error. |
| Error path returns token | `fn step(active: Active) -> Result<..., Failed<Active>>` — `Err(Failed { token: active, ... })` returns the unconsumed token. |
| Exhaustive match | `StepOutcome` enum. Every match site must handle all variants. Adding a variant breaks all callers. |
| No shared mutable state | `Active` is not `Clone`. Cannot duplicate the token. |
| Carry as ordinary data | `carry: Carry` is just a struct field. Rust's ownership rules handle whether it's shared or copied. |

### By the proc macro (compile-time structural checks)

| Constraint | Mechanism |
|---|---|
| Sealed constructors | `_seal: ()` private field. External construction impossible. Only the generated `fn design()` etc. can create rung types. |
| Rung existence | All transitions reference declared rungs — checked at expansion time. |
| Recover pairing | Every `\| Stalled => Active` has a matching `recover { stalled: Stalled => Active }` — checked at expansion time. |
| Terminal vs recoverable | `Complete` (terminal) cannot have a recover edge — checked at expansion time. |
| Duplicate carry fields | Rejected at expansion time. |
| Recover function signatures match | `recover fn stalled(stalled: Stalled) -> Active` signature must match the declared edge — checked at expansion time. |

### By convention (not enforced)

| Constraint | Status |
|---|---|
| Carry is read-only | Convention. The `carry` field is `pub` on each rung — transitions could mutate it. |
| Transition body correctness | The type proves the function was *called*, not that it *did the right thing*. |

---

## 4. What is NOT covered

### 4.1 In-module fabrication

The `_seal` pattern works across module boundaries. Inside the generated module, the seal field is accessible. A transition body could fabricate a rung:

```rust
fn claim(designed: Designed) -> Result<Claimed, Failed<Designed>> {
    // Could write: Claimed { _seal: (), carry: designed.carry, payload: designed }
    // instead of actually performing a claim.
}
```

**Why this gap exists:** Rust's visibility is per-module, not per-function. The sealed constructor pattern relies on module boundaries for enforcement. Inside the module, all bets are off.

**Path to close:** Nothing short of a separate crate per ladder. The macro could emit a sub-crate with the sealed types and a public trait for transition bodies. The sub-crate boundary would seal the constructors. But this adds build complexity (one crate per ladder) and may not be worth the cost for most use cases.

### 4.2 Transition body correctness

The type proves `claim()` was called. It does not prove the claim was *valid* — that the resource was available, that the policy allowed it, that the return value reflects reality. A `claim()` that immediately returns `Ok(Claimed { ... })` without checking compiles cleanly.

**Why this gap exists:** This is the boundary between typestate and formal verification. Typestate enforces the graph. Verification enforces the behavior inside the nodes. Proving that `claim()` actually checked availability requires either dependent types (the return type depends on the availability value) or external verification (property-based testing, model checking).

**Path to close:** Property-based testing with `proptest`. Generate sequences of ladder operations, verify invariants hold. "After `claim()`, the resource is marked as claimed." This doesn't close the gap at compile time, but it catches regressions. For the truly paranoid: run a model checker against the transition implementations.

### 4.3 Carry immutability — CLOSED

The carry is supposed to be read-only witness data. The `carry { ... }` declaration now generates a **private** field with a `pub fn carry(&self) -> &Carry` accessor. Transitions call `token.carry()` to read witness data; the immutable reference prevents accidental mutation. The `+transfer` annotation for fields that need to move out of the carry is a future extension.

**Status:** Closed 2026-07-17. Private field + `&Carry` accessor, generated by the macro, zero build cost.

### 4.4 Recovery progress

`recover_stalled(stalled: Stalled) -> Active` produces a new `Active`. Nothing verifies that the new `Active` is *different* from the stalled one. An infinite stall loop — where `step` returns `Stalled`, `recover_stalled` returns an identical `Active`, and `step` returns `Stalled` again — compiles and runs forever.

**Why this gap exists:** Proving forward progress requires reasoning about the *values* inside the types, not just the types themselves. This is a liveness property, not a safety property. Typestate handles safety (you can't skip a rung). Liveness (you can't stall forever) requires temporal logic.

**Path to close:** A `#[must_progress]` attribute on recover functions. The macro instruments the recover function to require that at least one field of the produced `Active` differs from the stalled `Active` — a runtime check that panics on no-progress. For compile-time enforcement: a `StallCount(u32)` in the carry that increments on each stall, with a compile-time or runtime ceiling. Neither is a full solution; both catch the common failure mode.

### 4.5 Cross-crate provenance

If crate A defines `ladder Work`, crate B can receive `Work::Active` and call `Deployment::stage(active)`. The type is correct. But did that `Active` come from a valid ladder run in crate A, or was it fabricated inside crate A's module? Crate B has no way to know.

**Why this gap exists:** Rust's visibility boundary is crate-level. Once a type crosses a crate boundary, the receiving crate trusts the sending crate's API. This is the standard Rust trust model — crates trust each other's public APIs.

**Path to close:** If the gap matters for a specific architecture, emit the sealed types into a dedicated sub-crate that *only* the macro controls. No other code in the defining crate can access the seal field. This is the same fix as §4.1, and carries the same build-complexity cost.

### 4.6 Concurrent access — CLOSED (default `!Send + !Sync`)

Move semantics prevent use-after-move for owned values. But `Arc<Active>` or a `&Active` reference circumvents ownership. Two threads could call `step(active)` on the same logical token if they share a reference.

**Why the gap existed:** Rust's ownership system prevents data races, not logical races. `Arc<Active>` is memory-safe — two threads can read the same `Active` — but the ladder contract (one token, one consumer) is violated.

**Status:** Closed 2026-07-17. Every generated rung struct now carries a private `_not_send: PhantomData<*const ()>` field, making it `!Send + !Sync`. An `Arc<Active>` or `&Active` can no longer cross a thread boundary, so the one-token-one-consumer contract holds even under shared references. Zero build cost — one field per rung struct. Proven by `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` (autoref specialization; the assert flips if the marker is ever removed).

**⚠️ Known remnant:** Verdict structs (`Stalled`, `Converged`, etc.) are emitted as bare `pub struct Stalled;` — no `_seal`, no `PhantomData`. A `Stalled` token (which gets fed into `recover_stalled`) can cross a thread boundary, and unit verdicts are publicly constructible. Lower stakes than live rung tokens, but an inconsistency worth flagging. Graded alongside the other open gaps — close when the value justifies the cost.

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
| Carry syntax | — | — | ✓ Emitted as struct field |
| Carry read-only | — | — | ✓ Private field + `&Carry` getter |
| Recovery progress | — | — | ✗ Liveness property |
| Cross-crate provenance | — | — | ✗ Crate boundary trust |
| Concurrent access | — | — | ✓ `!Send + !Sync` by default (`PhantomData<*const ()>`) |
| No silent drop | — affine, drop allowed | — | ✓ `#[must_use]` on every token (warn; error under `deny`) |
| Transition body correctness | — | — | ✗ Formal verification gap |

---

## 6. Getting closer to full coverage

### Short-term (proc macro v1)

- **Carry: split `carry` and `carry_mut`.** Fields in `carry` generate immutable accessors. Fields in an optional `carry_mut` block allow mutation. The common case (witness data) is read-only by default.
- **Recovery progress: `#[must_progress]`.** A runtime assertion that the recovered token differs from the stalled token in at least one field. Panics on no-progress. Catches 90% of infinite-stall bugs.
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
| `rung/checker.py` | Done — 8 static checks, single-pass, 11 tests |
| `rung/interpreter.py` | Done — linear token tracking, provenance trace |
| `rust-example/` | Done — hand-written MetricOptimization, sealed constructors, move semantics |
| `ladder!` proc macro | Done — `rung-macro/src/lib.rs`: parse + 8 static checks + emit (sealed structs, `!Send` rungs, carry accessor, `Failed<Prev>`, verdict enum, transition/recover trait signatures) |

---

*Festina lente. Cut the root. Chesterton's Fence. Descend.*