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
declaration := [ carry ] rung ( "=>" [gate] rung )* "=>" [gate] "{" verdict ( "|" verdict )* "}" [ recover ]

carry       := "carry" "{" ( ident ":" type )  ( "," ident ":" type )* "}"
rung        := Ident "(" type ")"                     -- name + payload type

gate        := "#" "[" "judgmental" "(" type ")" "]"  -- competence role

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

**Gate markers.** A marker annotates the transition's *target*, because that is
what the transition is named after. `#[judgmental(R)]` on a rung marks the
forward transition producing it; `#[judgmental(R)]` on the verdict block marks
`step`. An unmarked transition reads as *decidable* and is emitted exactly as it
was before markers existed (G12). The marker is out of Het's four gates
([`four-gates`](rung-het-propositions.md#four-gates)); of those, only
`judgmental` is implemented. `#[authorial]` and `#[conditional(..)]` are
`compile_error!` — see [Q11](questions/open/q11-gate-faithfulness.md) — and
`#[judgmental]` with no role is likewise refused
([`judgmental-declares-role`](rung-het-propositions.md#judgmental-declares-role):
a role that is not named cannot resolve a judge, so there is no signature to
emit).

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
(`.archive/python-poc/rung/checker.py`, verified in sync). Rules 9–10 apply only when an
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
- **`must_be_bound_to<A: Provenanced, R: Role>(argument: &A, licence:
  &::rung::Qualified<R>)`** — the token-binding guard (G13).
- **Transition and recover functions** (when an `impl` block is present) — one
  `pub fn` per transition/recover, expanded from the corresponding body *inside*
  the module. A forward transition returns its target rung; a branching transition
  returns `Result<StepOutcome, Failed<from>>`; a recover function returns its
  target rung. Omitting the `impl` block yields a type-only declaration (no
  functions).
  - **Unmarked:** `pub fn active(spec: Spec) -> Active`.
  - **`#[judgmental(R)]`:** `pub fn active(spec: Spec, q: ::rung::Qualified<R>)
    -> Active` — a second parameter, taken by value. Its name comes from the
    body's *second* closure input (`active = |spec, q| { .. }`) when there is
    one; otherwise it is bound to `_q` and consumed unread. The body is preceded
    by the injected binding prologue `must_be_bound_to(&spec.payload, &q);`
    (G13), so the source rung's payload MUST implement `::rung::Provenanced`.

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
  never fabricated to hold an arrow's result (see
  [the law](rung-ct-propositions.md#the-law)). That enforcement reaches exactly
  as far as this guarantee does: a type-only declaration publishes every
  constructor, and is freely generated by convention only (see
  [freeness-enforced-only-with-bodies](rung-ct-propositions.md#freeness-enforced-only-with-bodies)).
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
- **G12 — Gate-marked signature.** A `#[judgmental(R)]` transition MUST take a
  second parameter of type `::rung::Qualified<R>`, by value; an unmarked
  transition MUST emit byte-for-byte what it emitted before markers existed. Two
  gates are therefore two *signatures*, separated by the host's type system
  rather than by a convention someone keeps
  ([`two-signatures-not-two-fragments`](rung-het-propositions.md#two-signatures-not-two-fragments)).
  `Qualified` has no public constructor — `Pool::qualify` is the only mint — so a
  judgmental transition cannot be called without an outside, and a decidable one
  has no parameter an outside could enter through
  ([`decidable-cannot-consult-pool`](rung-het-propositions.md#decidable-cannot-consult-pool)).
  **This makes the signature honest; G13 is what binds the token to an
  argument** — see §5 for what neither secures.
  *Conformance: `gate_markers.rs::judgmental_transition_takes_a_qualified_token`
  (the emitted `fn` is coerced to a `fn` pointer of the exact expected type), and
  the `tests/ui/` `trybuild` cases: `gate_missing_token` → E0061,
  `gate_forged_token` → E0451.*
- **G13 — Token binding.** The macro MUST prefix every `#[judgmental(R)]`
  transition body with `must_be_bound_to(&<source>.payload, &<token>)`, which
  panics unless the token's recorded `π(a)` equals the source rung payload's.
  The body cannot skip it, exactly as it cannot skip G8's `must_progress`, and
  for the same reason: the body is the domain's, so a guarantee the body could
  omit is not a guarantee. This requires the source rung's payload to implement
  `::rung::Provenanced` — without `π(a)` there is nothing to measure.

  A `Qualified<R>` records the argument it was measured against alongside the
  principal, and `Qualified::admit` is the one gate that spends it. The seal
  (G12) closes *fabrication* — nobody can write a token. G13 closes *transfer* —
  nobody can spend an honestly-earned token on an argument it was never measured
  against, which is the act
  [`disjointness-against-argument`](rung-het-propositions.md#disjointness-against-argument)
  forbids and the pair
  [`non-identity-by-construction`](rung-het-propositions.md#non-identity-by-construction)
  requires the token to witness.

  It panics rather than returning an error because a marked transition's return
  type is the *domain's* declaration; there is no `Err` variant to route a
  refusal through, and a P0 violation is not a recoverable step outcome. The
  library-level consumers that do own their return type — `rung_het::dispose`,
  `theory!`'s `settle` — return `TokenNotBound` instead.
  *Conformance:
  `gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`
  (the ladder's judgmental body never mentions its token, and a transferred
  token is refused anyway), and
  `rung-het/tests/token_binding.rs::{dispose_refuses_a_token_minted_against_the_model,
  settle_refuses_a_token_minted_against_a_different_model}`.*

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
- **Gate-faithfulness.** G12 secures the signature and G13 secures the argument.
  Neither secures the value an arrow *returns*, and two of Het's four gates have
  no signature at all. Four named limits:
  - *Two gates are unimplemented.* `#[authorial]` and `#[conditional(..)]` are
    parse-time refusals, not encodings. Gate-faithfulness is a condition on
    **every** operation of an algebra, so an algebra with an authorial arrow
    cannot state it here at all.
  - *The returned value is unconstrained.* G13 checks `π(p) ∩ π(a) = ∅` on the
    way in. Admissibility as Het states it also constrains what comes out —
    `π(f(a)) ∩ π(a) = ∅` — which is a property of the body, and so inherits
    transition-body correctness whole.
  - *Decidable is not pure.* The unmarked signature excludes Het's outside — the
    principal pool — and is silent about clocks, files, and networks
    ([`purity-not-secured`](rung-het-propositions.md#purity-not-secured)).
  - *A type-only declaration emits no transitions,* so a marker on one has
    nothing to constrain and is inert, exactly as G2's seal is
    ([`freeness-enforced-only-with-bodies`](rung-ct-propositions.md#freeness-enforced-only-with-bodies)).

  Whether G12 + G13 amount to gate-faithfulness is argued — and answered *no* —
  in [Q11](questions/open/q11-gate-faithfulness.md), which stays open on the
  first two limits above.

---

## 6. Conformance

The conformance suite is `rung/tests/` and the doctests in `rung/src/lib.rs`. A
change that violates any guarantee above MUST break at least the cited test. The
README's Getting Started example is itself a run doctest (via `include_str!`), so
the documented public API cannot silently drift from the macro.
