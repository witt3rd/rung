# The Ladder Specification

Normative specification of the `ladder!` macro. For an introduction and a
runnable example, see the [README](../README.md). For design history, rationale,
and the record of how each gap was closed, see [RUNG-RS.md](.archive/RUNG-RS.md).

This document states the rules. **MUST** / **MUST NOT** are normative. Each
guarantee names the conformance test that fails if the implementation stops
honoring it; guarantees delegated to the Rust compiler are marked *(rustc)*.

---

## 1. Grammar

A `ladder!` invocation is a declaration block, optionally followed by an inline
`impl` block supplying the transition logic:

```
ladder!( Name { <declaration> } [ impl { <bodies> } ] )
```

**Declaration:**

```
declaration := [ carry ] rung ( "=>" rung )* "=>" "{" verdict ( "|" verdict )* "}" [ recover ]

carry       := "carry" "{" ( ident ":" type )  ( "," ident ":" type )* "}"
rung        := Ident "(" type ")"                     -- name + payload type

verdict     := Ident                                  -- terminal marker
             | Ident "(" type ")"                     -- terminal carrying a result payload
             | Ident "=>" Ident                       -- recoverable: verdict => target rung
             | Ident "->" Ident                       -- continue arm: verdict -> target rung

recover     := "recover" "{" edge* "}"
edge        := ident ":" Ident        "=>" Ident      -- verdict recovery:   name: Verdict => Rung
             | ident ":" "Failed" "(" Ident ")" "=>" Ident   -- error-path recovery: name: Failed(Rung) => Rung
```

**Bodies** (inline `impl` block):

```
bodies := ( ident "=" closure [ "," | ";" ] )*        -- one per transition/recover fn
closure := "|" pat "|" ( block | expr )
```

**Transition naming.** A forward transition (`A => B`) is named after its target
rung, lowercased (`B` ⇒ `b`). The branching transition (`A => { .. }`) is named
`step`. A recover function is named by its `recover` edge. Inline body names MUST
match these (§2, checks 9–10).

`=>` reads *recover*; `->` reads *produces*. A continue arm carries its target
rung as a live token; a recoverable verdict carries its source rung and hands off
to a guarded recover function.

---

## 2. Static semantics

The macro MUST reject, with a `compile_error!` pointing at the violation, any
ladder in which:

| # | Rule |
|---|---|
| 1 | two `carry` fields share a name |
| 2 | a transition names a `from`/`to` rung that is not declared |
| 3 | a non-terminal verdict has no target; a `=>`/`->` target is not a declared rung; or a recoverable verdict or continue arm declares a payload |
| 4 | a recoverable verdict (`=>`) has no matching `recover` edge (continue arms `->` are exempt) |
| 5 | a `recover` edge has no matching recover function, or its target rung is not declared |
| 6 | a terminal verdict has a `recover` edge |
| 7 | a `recover` edge names a verdict that appears on no transition |
| 8 | a recover function's return rung — or a `Failed(Rung)` source rung — is not declared |
| 9 | *(inline bodies present)* a body names no transition/recover function, or names one twice |
| 10 | *(inline bodies present)* a transition/recover function has no body |

Rules 1–8 are structural and mirror the Python reference checker
(`python-poc/rung/checker.py`, verified in sync). Rules 9–10 apply only when an
`impl` block is present.

Conformance: the payload (rule 3), continue-target (rule 3), and `Failed(Rung)`
(rule 8) extensions each have a `compile_fail` doctest in `rung/src/lib.rs`.

---

## 3. Emitted artifacts

For a ladder `Name`, the macro emits a module `name` (the ladder name,
lowercased) containing:

- **`Carry`** — if a `carry` block is present: `#[derive(Clone, Debug)] pub struct
  Carry { pub <field>: <type>, .. }`.
- **One struct per rung** — sealed and thread-bound:
  `pub struct R { _seal: (), _not_send: PhantomData<*const ()>, carry: Carry, pub
  payload: P }`, `#[must_use]`. With a constructor `R::new(payload, carry)` and an
  accessor `pub fn carry(&self) -> &Carry`. Constructor visibility follows G2.
- **One struct per verdict** — sealed and thread-bound, `#[must_use]`:
  - terminal marker: fields `{ _seal, _not_send }`, `new()`.
  - terminal with payload `V(P)`: adds `payload: P`; `new(payload)`, `.payload()`,
    `.into_payload()`.
  - recoverable `V => R`: adds `source: <from-rung>`; `new(source)`, `.source()`,
    `.into_source()`.
  - a continue arm emits **no** verdict struct (its outcome carries a live rung).
- **`StepOutcome`** — `#[must_use] pub enum` with one variant per verdict of the
  branching transition. A continue arm's variant carries its **target rung**; every
  other variant carries its **verdict struct**.
- **`Failed<Prev>`** — `#[must_use] pub struct Failed<Prev> { pub token: Prev, pub
  error: String }`.
- **`must_progress<T: PartialEq>(before: &T, after: &T)`** — the recovery guard
  (G8).
- **Transition and recover functions** (when an `impl` block is present) — one
  `pub fn` per transition/recover, expanded from the corresponding body *inside*
  the module. A forward transition returns its target rung; a branching transition
  returns `Result<StepOutcome, Failed<from>>`; a recover function returns its
  target rung. Omitting the `impl` block yields a type-only declaration (no
  functions).

Inside body expressions, rung/verdict names resolve unqualified; payload types
resolve from the surrounding scope (`use super::*`).

---

## 4. Guarantees

Each guarantee is normative and names its conformance test.

- **G1 — Linear consumption.** A transition consumes its input rung by value; using
  a rung after it is moved MUST be a compile error. *(rustc — move semantics.)*
- **G2 — Sealed construction.** A rung MUST NOT be constructible by struct literal
  outside its module (`_seal` is private). When an `impl` block is present, only
  the **entry** rung's `new` is public; every other rung's `new` is
  module-private, so no code outside the module can mint a mid-ladder rung.
  Categorically this is not merely a fabrication guard: it enforces that a *verb
  cannot occupy object-position* — a state is reached only by traversing an arrow,
  never fabricated to hold an arrow's result (see `docs/RUNG-CT.md`, "The law").
  *Conformance: `rung/src/lib.rs` compile_fail doctest — external `Active::new`
  fails with E0624.*
- **G3 — One token, one thread.** Every rung and verdict MUST be `!Send + !Sync`
  (via `PhantomData<*const ()>`), so a shared `Arc`/`&` cannot carry it across a
  thread boundary. *Conformance: `compile_pass.rs::test_rungs_are_not_send_or_sync`
  (rungs and verdicts).*
- **G4 — No silent drop.** Every rung, verdict, `StepOutcome`, and `Failed` MUST be
  `#[must_use]`. Dropping a token in statement position is a warning — an error
  under `#![deny(unused_must_use)]`. *Conformance: `rung/src/lib.rs` must_use
  compile_fail doctest. (Escapable by `mem::forget` / `let _ =` — see §5.)*
- **G5 — Carry immutability.** `Carry` MUST be a private field exposed only through
  `&Carry`; a transition body cannot mutate it. *Conformance:
  `compile_pass.rs::test_carry_accessor_exists`.*
- **G6 — Exhaustive outcomes.** `StepOutcome` is an enum; every match site MUST
  handle all variants. *(rustc — enum exhaustiveness.)*
- **G7 — Recover pairing.** Every recoverable verdict has a matching recover
  function and vice versa; terminal verdicts have none (§2, rules 4–7). *(macro —
  static checks.)*
- **G8 — Recovery progress.** The macro MUST wrap every *verdict* recover body with
  `must_progress`, comparing the source rung's payload to the produced rung's
  payload and panicking if equal. The body cannot skip it. Requires the shared
  payload type be `Clone + PartialEq`. *Conformance:
  `end_to_end.rs::recover_guard_is_auto_injected` (panics with no explicit call).*
- **G9 — Error-path recovery.** A `recover { .. : Failed(R) => R }` function
  receives the `Failed` and returns the next rung. No progress guard is injected (a
  retry may reuse the token). *Conformance:
  `end_to_end.rs::recovers_from_the_failed_error_path`.*
- **G10 — Continue arms.** A `V -> R` arm makes `step` build the next rung inline;
  `StepOutcome::V` carries that rung directly — no recover function, no guard, no
  source. *Conformance: `end_to_end.rs::continue_arm_loops_without_a_recover_fn`.*
- **G11 — Terminal payloads.** A `V(P)` terminal verdict returns a value through
  the verdict, read via `.payload()` / `.into_payload()`. *Conformance:
  `end_to_end.rs::drives_to_convergence` asserts the returned payload.*

---

## 5. Non-guarantees

Explicitly out of scope. The macro does **not** enforce:

- **Transition-body correctness.** The type proves a transition *ran*, not that its
  logic was valid — the boundary between typestate and formal verification.
- **Cross-crate provenance.** A rung crossing a crate boundary is trusted, like any
  Rust public API. Sealing this needs a sub-crate per ladder.
- **Same-module / entry fabrication.** G2 stops *external* fabrication; code inside
  the generated module, and the public entry constructor, can still build rungs —
  the module-boundary limit Rust always has.
- **Drop-proofing beyond the lint.** G4 is `#[must_use]`, which `mem::forget`, `let
  _ = token;`, or burying the token in a dropped container all bypass. True
  no-drop needs language-level linear types.
- **Liveness beyond the guard.** G8 catches an identical-token stall loop; it does
  not prove general forward progress.

---

## 6. Conformance

The conformance suite is `rung/tests/` and the doctests in `rung/src/lib.rs`. A
change that violates any guarantee above MUST break at least the cited test. The
README's Getting Started example is itself a run doctest (via `include_str!`), so
the documented public API cannot silently drift from the macro.
