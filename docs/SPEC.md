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

gate        := "#" "[" "judgmental" "(" type ")" "]"  -- competence role, judgmental
             | "#" "[" "authorial"  "(" type ")" "]"  -- competence role, authorial

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
what the transition is named after. A marker on a rung marks the forward
transition producing it; a marker on the verdict block marks `step`. A
transition carries **at most one** marker: Het's gates are alternatives, not a
set ([`four-gates`](rung-het-propositions.md#four-gates)). An unmarked
transition reads as *decidable* and is emitted exactly as it was before markers
existed (G12).

Two of Het's four gates are implemented, and they are the two that dispatch to
an outside — in **opposite directions**
([`judgment-refuses-authorship-requires`](rung-het-propositions.md#judgment-refuses-authorship-requires)):

| marker | emitted second parameter | filter |
|---|---|---|
| `#[judgmental(R)]` | `::rung::Qualified<R>` | `capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅` |
| `#[authorial(R)]` | `::rung::Authorized<'_, R>` | `capable(p, role(o)) ∧ standing(p, M)` |

One pool, two filters
([`one-pool-two-filters`](rung-het-propositions.md#one-pool-two-filters)); the
marker selects the predicate, not the pool. The two share their left conjunct
and take opposite second conjuncts, so the two tokens are different types and
neither can be passed where the other is asked for (G12, G14).

`#[conditional(..)]` remains a `compile_error!` — it classifies per model, one
level up ([`classifier-one-level-up`](rung-het-propositions.md#classifier-one-level-up)),
and `ladder!`'s checks run at expansion time against a single declaration; see
[Q11](questions/open/q11-gate-faithfulness.md). Either implemented marker
written **without a role** is likewise refused: a judgmental role that is not
named cannot resolve a judge
([`judgmental-declares-role`](rung-het-propositions.md#judgmental-declares-role)),
and an authorial marker that names none can witness only the right conjunct of
[`authorial-qualifying-set`](rung-het-propositions.md#authorial-qualifying-set),
which would make the competence filter decorative. In both cases there is no
signature to emit.

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
(rule 8) extensions are each pinned by a `trybuild` case —
`spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload`,
`::a_continue_arm_target_must_be_a_declared_rung`, and
`::a_failed_source_rung_must_be_declared`, whose committed `.stderr` snapshots
hold the macro's exact message. Each also appears as a `compile_fail` doctest in
`rung/src/lib.rs`, which documents the refusal but does not assert its
diagnostic (§6).

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
- **`must_hold_standing_over<A: Situated, R: Role>(subject: &A, pen:
  &::rung::Authorized<'_, R>)`** — the standing guard (G14). Emitted **only**
  when the ladder carries an `#[authorial(R)]` marker, so that an unmarked or
  judgmental ladder's emission is unchanged (G12's compatibility clause).
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
  - **`#[authorial(R)]`:** `pub fn revised(filed: Filed, pen:
    ::rung::Authorized<'_, R>) -> Revised` — a second parameter, taken by value.
    Its name comes from the body's *second* closure input when there is one;
    otherwise it is bound to `_pen` and consumed unread. The body is preceded by
    the injected standing prologue `must_hold_standing_over(&filed.payload,
    &pen);` (G14), so the source rung's payload MUST implement
    `::rung::Situated`.

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
  *Conformance: `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`
  — a `trybuild` case whose committed `.stderr` pins **E0624, associated
  function `new` is private**, as the sole error. The `rung/src/lib.rs`
  compile_fail doctest shows the same refusal in rustdoc but does not assert the
  code (§6).*
- **G3 — One token, one thread.** Every rung and verdict MUST be `!Send + !Sync`
  (via `PhantomData<*const ()>`), so a shared `Arc`/`&` cannot carry it across a
  thread boundary. *Conformance: `compile_pass.rs::test_rungs_are_not_send_or_sync`
  (rungs and verdicts).*
- **G4 — No silent drop.** Every rung, verdict, `StepOutcome`, and `Failed` MUST be
  `#[must_use]`. Dropping a token in statement position is a warning — an error
  under `#![deny(unused_must_use)]`. *Conformance:
  `spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error` — a
  `trybuild` case whose committed `.stderr` pins the denied `unused_must_use`
  lint and the macro's contract-specific note. The `rung/src/lib.rs` must_use
  compile_fail doctest documents it but does not assert the diagnostic (§6).
  (Escapable by `mem::forget` / `let _ =` — see §5.)*
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
- **G14 — The authorial gate.** An `#[authorial(R)]` transition MUST take a
  second parameter of type `::rung::Authorized<'_, R>`, by value, and the macro
  MUST prefix its body with `must_hold_standing_over(&<source>.payload, &<pen>)`,
  which panics unless the pen's container equals the source rung payload's. This
  requires the source rung's payload to implement `::rung::Situated` — without a
  container there is nothing standing could be held over. Unmarked and
  `#[judgmental(R)]` emission is unchanged.

  **G14 is not G12+G13 with a different token name, and an implementation that
  made it one would satisfy every clause above while enforcing nothing.** The
  two gates run over one pool and select opposite predicates
  ([`one-pool-two-filters`](rung-het-propositions.md#one-pool-two-filters)):

  | | judgmental | authorial |
  |---|---|---|
  | qualifying set | [`judgmental-qualifying-set`](rung-het-propositions.md#judgmental-qualifying-set) | [`authorial-qualifying-set`](rung-het-propositions.md#authorial-qualifying-set) |
  | second conjunct | `π(p) ∩ π(a) = ∅` — **disjointness** | `standing(p, M)` — **standing** |
  | reading | you did **not** author this | this is **yours to revise** |
  | admissibility | `π(f(a)) ∩ π(a) = ∅` | `π(f(a)) ⊆ π(p) ∧ standing(p, a)` |

  Provenance overlap is what disqualifies a judge and what an author needs
  ([`provenance-overlap-is-the-point`](rung-het-propositions.md#provenance-overlap-is-the-point)),
  so a principal that passes one filter has, on that evidence, said nothing
  about the other and typically fails it
  ([`judgment-refuses-authorship-requires`](rung-het-propositions.md#judgment-refuses-authorship-requires)).
  `Pool::authorize` MUST therefore check **both** conjuncts of
  [`authorial-qualifying-set`](rung-het-propositions.md#authorial-qualifying-set):
  standing alone mints no pen. It refuses on the judgmental branch of
  [`standing-conditional-gated`](rung-het-propositions.md#standing-conditional-gated)
  rather than guessing — closing that branch needs a judge, terminating at
  depth one
  ([`standing-terminates-at-depth-one`](rung-het-propositions.md#standing-terminates-at-depth-one)),
  whose own qualification is non-identity relative to the **author**
  ([`standing-judge-disjoint-from-author`](rung-het-propositions.md#standing-judge-disjoint-from-author)).

  The pen is borrowed-lifetime rather than consumed-and-gone in the library
  (`Proposal::remedy`, `enact` take `&Authorized`): standing is not spent by a
  single act, unlike a judgment licence, which is spent because each dispatch
  re-runs the filter against a different argument. A `ladder!` transition takes
  the pen by value only because a transition consumes its inputs; nothing was
  spent, and the same principal may mint another on the next rung.

  *Conformance: `gate_markers.rs::authorial_transition_takes_an_authorized_pen`
  (the emitted `fn` is coerced to a `fn` pointer of the exact expected type, and
  the ladder drives end to end);
  `::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`
  (the ladder's authorial body never mentions its pen, and a pen minted over
  another container is refused anyway);
  `::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` — **the
  asymmetry test**: a steward that is not capable of the declared role is
  refused, a principal that qualifies as a *judge* of the subject gets no pen,
  and the principal that does hold the pen is refused as a judge of the same
  subject. Dropping the capability conjunct from `Pool::authorize` reddens it;
  stubbing the injected prologue reddens the prologue test. And the
  `tests/ui/` `trybuild` cases: `gate_authorial_missing_pen` → E0061,
  `gate_forged_pen` → E0451, `gate_authorial_no_role` → the macro's
  `compile_error!`.*

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
- **Gate-faithfulness.** G12 secures the judgmental signature, G13 its argument,
  G14 both halves for the authorial gate. None secures the value an arrow
  *returns*, and one of Het's four gates still has no signature. Four named
  limits:
  - *One gate is unimplemented.* `#[conditional(..)]` is a parse-time refusal,
    not an encoding. Gate-faithfulness is a condition on **every** operation of
    an algebra, so an algebra with a conditional arrow cannot state it here at
    all.
  - *The returned value is unconstrained.* G13 checks `π(p) ∩ π(a) = ∅` and G14
    checks standing, both on the way **in**. Admissibility as Het states it also
    constrains what comes **out** —
    [`admissibility-subcategories`](rung-het-propositions.md#admissibility-subcategories)
    gives `π(f(a)) ∩ π(a) = ∅` judgmentally and `π(f(a)) ⊆ π(p)` authorially —
    which is a property of the body, and so inherits transition-body
    correctness whole. `Prov::contained_in` exists and no guarantee calls it;
    that is the honest measure of the gap.
  - *Decidable is not pure.* The unmarked signature excludes Het's outside — the
    principal pool — and is silent about clocks, files, and networks
    ([`purity-not-secured`](rung-het-propositions.md#purity-not-secured)).
  - *A type-only declaration emits no transitions,* so a marker on one has
    nothing to constrain and is inert, exactly as G2's seal is
    ([`freeness-enforced-only-with-bodies`](rung-ct-propositions.md#freeness-enforced-only-with-bodies)).

  Whether G12 + G13 + G14 amount to gate-faithfulness is argued — and answered
  *no* — in [Q11](questions/open/q11-gate-faithfulness.md), which stays open on
  the first two limits above.

---

## 6. Conformance

The conformance suite is `rung/tests/` and the doctests in `rung/src/lib.rs`. A
change that violates any guarantee above MUST break at least the cited test. The
README's Getting Started example is itself a run doctest (via `include_str!`), so
the documented public API cannot silently drift from the macro.

**A `compile_fail` doctest does not verify the error code.** rustdoc ignores the
`E0NNN` in a fence such as `compile_fail,E0999`, and E0999 does not exist — the
block passes. So a `compile_fail` doctest asserts exactly one thing:
*this did not compile*. It cannot tell the refusal it was written for from a
typo, an unresolved import, a missing `main` (E0601), or a name that fell out of
scope when rustdoc wrapped the snippet in a `fn main` of its own. Adding the
code annotation does not fix this; nothing reads it.

Consequently **no guarantee above may cite a `compile_fail` doctest as its
conformance test.** Refusals are pinned by `trybuild` cases in `rung/tests/ui/`,
which diff the full rendered stderr against a committed `.stderr` snapshot, so
the code and the message are both part of the assertion. The doctests are kept
alongside — they are the documentation, and a reader meets the refusal in
rustdoc — but they are the illustration, not the evidence.

Two further traps, both found in this repo and both silent:

- A doctest with no `fn main` is wrapped in one by rustdoc, so `struct` and
  `macro_rules!` items land in a function body. A `ladder!`-generated `mod`
  then cannot see them and the example fails on E0425 — a green test asserting
  nothing about the guarantee. Write an explicit `fn main` when the snippet
  declares items.
- A struct literal that omits a field fails with E0063 whether or not the
  fields are private, so a "cannot be forged" example with a stale field list
  keeps passing after the seal is removed. Name every field.

A refusal test that cannot fail is not a guarantee. The way to establish that a
case can fail is to make the guarded thing legal and watch the case go red.
