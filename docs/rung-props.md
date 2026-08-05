# rung — The ladder the macro declares

**Status: normative.** This document states what `ladder!` accepts, what it
emits, and what it guarantees. It records no history and names no reviewer; how
each rule was arrived at is in [`rung-notes.md`](rung-notes.md). Every claim is
stated once, in one place, and referred to elsewhere by number.

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm` is a
remark on `n.m`. Interior propositions are the conjunction of their children.
Leaves are single checkable claims. **MUST** / **MUST NOT** are normative.

**This document is generated.** Its source is `rung-doctrine/src/rung.rs`, and it
is written by `cargo run -p rung-doctrine --bin render`. Editing it here does
not change what it says; the next render restores this text. Where the two
differ, the encoding is right and this file is stale — CI checks exactly that.

**Numbers are derived, not authored.** A proposition's identity is its slug;
its place in the tree is its declared parent; its order is declaration order.
The decimal number and every reference to it are computed at render time and
appear nowhere in the source, so inserting, removing or reparenting a
proposition cannot break a reference and cannot leave a number stale — there is
no number to leave.

**Two labelled subtrees.** [4](#guarantees) numbers its children `G1`–`G15` and
[7](#design-judgments) numbers its children `J1`–`J2`, declared by
`data-numbering` on the root. These are still derived numbers — the letter is
the root's, the index is document order — but they are flat rather than
concatenated, because both are cited by label from Rust source and from test
filenames.

**Three documents, one slug space.** A reference whose target names
`rung-ct-props.md` points into the categorical account, and one naming
`rung-het-props.md` points into Het's formalism. Where a claim here
touches one there, it links rather than restates.

**What settles what.** [1](#declaration-is-a-block)–[6](#conformance-suite) are
settled by the macro, by rustc, or by a named test. [7](#design-judgments) is
not: no machine decides where a ladder should bottom out or what earns a place
in `rung-std`. Those bind design decisions and carry no conformance test.

---

## 1 · Grammar

<a id="declaration-is-a-block"></a>
**1** A `ladder!` invocation is a **declaration block**, optionally followed by
an inline `impl` block supplying the transition logic.

```
ladder!( Name { <declaration> } [ impl { <bodies> } ] )
```

<a id="declaration-grammar" data-parent="declaration-is-a-block"></a>
**1.1** The declaration MUST match:

```
declaration := [ carry ] rung ( "=>" [gate] rung )* "=>" [gate] "{" verdict ( "|" verdict )* "}" [ recover ] [ resume ]

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

resume      := "resume" "{" redge* "}"
redge       := ident ":" authorial "Suspended" "(" Ident ")" "=>" Ident
                                                      -- name: #[authorial(R)] Suspended(Rung) => Rung
```

`=>` reads *recover*; `->` reads *produces*. A continue arm carries its target
rung as a live token; a recoverable verdict carries its source rung and hands
off to a guarded recover function.

A `resume` edge is the adjoint of the **residual**, and its `#[authorial(R)]`
marker is **not optional** — see [G16](#g16-the-residual-channel).

<a id="bodies-grammar" data-parent="declaration-is-a-block"></a>
**1.2** The inline `impl` block, when present, MUST match:

```
bodies := ( ident "=" closure [ "," | ";" ] )*        -- one per transition/recover fn
closure := "|" pat "|" ( block | expr )
```

<a id="transition-naming" data-parent="declaration-is-a-block"></a>
**1.3** A forward transition (`A => B`) is **named after its target** rung,
lowercased (`B` ⇒ `b`). The branching transition (`A => { .. }`) is named
`step`. A recover function is named by its `recover` edge. Inline body names
MUST match these ([2](#macro-must-reject), rules 9–10).

<a id="marker-annotates-the-target" data-parent="declaration-is-a-block"></a>
**1.4** A gate marker **annotates the transition's target**, because that is
what the transition is named after. A marker on a rung marks the forward
transition producing it; a marker on the verdict block marks `step`.

<a id="at-most-one-marker" data-parent="marker-annotates-the-target"></a>
**1.41** A transition carries **at most one** marker. Het's gates are
alternatives, not a set
([2.1](rung-het-props.md#four-gates)). An unmarked transition
reads as *decidable* and is emitted exactly as it was before markers existed
([G12](#g12-gate-marked-signature)).

<a id="two-markers-implemented" data-parent="marker-annotates-the-target"></a>
**1.42** Two of Het's four gates are implemented, and they are the two that
dispatch to an outside — in **opposite directions**
([3.61](rung-het-props.md#judgment-refuses-authorship-requires)):

| marker | emitted second parameter | filter |
|---|---|---|
| `#[judgmental(R)]` | `::rung::Qualified<R>` | `capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅` |
| `#[authorial(R)]` | `::rung::Authorized<'_, R>` | `capable(p, role(o)) ∧ standing(p, M)` |

One pool, two filters
([3.4](rung-het-props.md#one-pool-two-filters)); the
marker selects the predicate, not the pool. The two share their left conjunct
and take opposite second conjuncts, so the two tokens are different types and
neither can be passed where the other is asked for
([G12](#g12-gate-marked-signature), [G14](#g14-the-authorial-gate)).

<a id="conditional-marker-refused" data-parent="marker-annotates-the-target"></a>
**1.43** `#[conditional(..)]` MUST be a `compile_error!`. It classifies per
model, one level up
([2.53](rung-het-props.md#classifier-one-level-up)),
and `ladder!`'s checks run at expansion time against a single declaration; see
[Q11](../questions/open/q11-gate-faithfulness.md).

<a id="marker-without-role-refused" data-parent="marker-annotates-the-target"></a>
**1.44** Either implemented marker written **without a role** MUST be a
`compile_error!`. A judgmental role that is not named cannot resolve a judge
([2.3](rung-het-props.md#judgmental-declares-role)),
and an authorial marker naming none can witness only the right conjunct of
[3.6](rung-het-props.md#authorial-qualifying-set),
which would make the competence filter decorative. In both cases there is no
signature to emit.

---

## 2 · Static semantics

<a id="macro-must-reject"></a>
**2** The macro MUST reject, with a `compile_error!` pointing at the violation,
any ladder in which:

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
| 11 | a `resume` edge names an undeclared rung; carries no `#[authorial(R)]` marker, or a `#[judgmental(..)]` one; or resumes from a rung that no `#[judgmental(R)]` forward transition can suspend |

<a id="structural-rules-mirror-the-reference-checker" data-parent="macro-must-reject"></a>
**2.1** Rules 1–8 are structural and mirror the Python reference checker
(`.archive/python-poc/rung/checker.py`, verified in sync).

<a id="body-rules-need-an-impl-block" data-parent="macro-must-reject"></a>
**2.2** Rules 9–10 apply **only** when an `impl` block is present.

<a id="resume-rules-are-g2" data-parent="macro-must-reject"></a>
**2.3** Rule 11's three clauses are one clause: a resume edge is emitted
*inside* the module, so every way of declaring one that nothing gates, or that
nothing can reach, is [G2](#g2-sealed-construction) with a door in it. The
marker clause is the sharpest — an unmarked spine transition reads as
*decidable* ([1.41](#at-most-one-marker)), and there is no decidable reading of
an arrow that writes a rung
([6.552](rung-het-props.md#resumption-is-authorial)).

<a id="extension-refusals-are-pinned" data-parent="macro-must-reject"></a>
**2.4** The payload (rule 3), continue-target (rule 3), and `Failed(Rung)`
(rule 8) extensions are each pinned by a `trybuild` case —
`spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload`,
`::a_continue_arm_target_must_be_a_declared_rung`, and
`::a_failed_source_rung_must_be_declared`, whose committed `.stderr` snapshots
hold the macro's exact message. Each also appears as a `compile_fail` doctest in
`rung/src/lib.rs`, which documents the refusal but does not assert its
diagnostic ([6.1](#compile-fail-asserts-only-non-compilation)).

---

## 3 · Emitted artifacts

<a id="emitted-module"></a>
**3** For a ladder `Name`, the macro emits a module `name` (the ladder name,
lowercased) containing the artifacts below.

<a id="emitted-carry" data-parent="emitted-module"></a>
**3.1** **`Carry`** — if a `carry` block is present:
`#[derive(Clone, Debug)] pub struct Carry { pub <field>: <type>, .. }`.

<a id="emitted-rung-structs" data-parent="emitted-module"></a>
**3.2** **One struct per rung** — sealed and thread-bound:
`pub struct R { _seal: (), _not_send: PhantomData<*const ()>, carry: Carry, pub
payload: P }`, `#[must_use]`. With a constructor `R::new(payload, carry)` and an
accessor `pub fn carry(&self) -> &Carry`. Constructor visibility follows
[G2](#g2-sealed-construction).

<a id="emitted-verdict-structs" data-parent="emitted-module"></a>
**3.3** **One struct per verdict** — sealed and thread-bound, `#[must_use]`:

- terminal marker: fields `{ _seal, _not_send }`, `new()`.
- terminal with payload `V(P)`: adds `payload: P`; `new(payload)`,
  `.payload()`, `.into_payload()`.
- recoverable `V => R`: adds `source: <from-rung>`; `new(source)`, `.source()`,
  `.into_source()`.
- a continue arm emits **no** verdict struct (its outcome carries a live rung).

<a id="emitted-suspended" data-parent="emitted-module"></a>
**3.4** **`Suspended<Prev>`** — `#[must_use] pub struct Suspended<Prev> { pub
token: Prev, pub raised: ::rung::Raised }`, with a hand-written `Debug` that
prints the reference and not the token. Emitted **only** when the ladder has a
`#[judgmental(R)]` forward transition or a `resume` edge, so that an unmarked
ladder's emission is unchanged ([G12](#g12-gate-marked-signature)'s
compatibility clause).

It is **not** [3.6](#emitted-failed) widened. `Failed` carries an error
string, and routing a raised reference through one would make the theory's
identity for a raised matter indistinguishable from an error message; `Failed`
is also emitted for every ladder, so widening it would break the compatibility
clause outright. What the two share is the *shape*, `Result<_, Carrier<from>>`.

<a id="suspended-reports-what-it-awaits" data-parent="emitted-suspended"></a>
**3.41** The macro MUST also emit `impl<Prev> ::rung::Awaiting for
Suspended<Prev>`, returning the carried [3.4](#emitted-suspended)
unchanged. A run reports what it awaits **itself**, so that a holder of many
suspended runs can ask each the same question without naming its module.

The fields are already `pub`, so this adds no access. What it adds is that the
reference cannot be supplied *about* a run by whoever holds it. A holder keyed
by a caller-passed reference can be told a run awaits a matter it never raised —
the same fabrication [G16](#g16-the-residual-channel) forecloses one
level down by deriving evidence from the `Raised` rather than accepting it
alongside. Reading it off the run leaves no parameter to lie through.

Het requires nothing further of the reference here: the trait hands back the
theory's value and declares no ordering, comparison or well-formedness over it
([3.2](rung-het-props.md#pool-is-opaque)).

<a id="emitted-step-outcome" data-parent="emitted-module"></a>
**3.5** **`StepOutcome`** — `#[must_use] pub enum` with one variant per verdict
of the branching transition. A continue arm's variant carries its **target
rung**; every other variant carries its **verdict struct**.

<a id="emitted-failed" data-parent="emitted-module"></a>
**3.6** **`Failed<Prev>`** — `#[must_use] pub struct Failed<Prev> { pub token:
Prev, pub error: String }`.

<a id="emitted-guards" data-parent="emitted-module"></a>
**3.7** The guards the injected prologues call:

- **`must_progress<T: PartialEq>(before: &T, after: &T)`** — the recovery guard
  ([G8](#g8-recovery-progress)).
- **`must_be_bound_to<A: Provenanced, R: Role>(argument: &A, licence:
  &::rung::Qualified<R>)`** — the token-binding guard
  ([G13](#g13-token-binding)).
- **`must_hold_standing_over<A: Situated, R: Role>(subject: &A, pen:
  &::rung::Authorized<'_, R>)`** — the standing guard
  ([G14](#g14-the-authorial-gate)). Emitted **only** when the ladder carries an
  `#[authorial(R)]` marker **or a `resume` edge**, so that an unmarked or
  judgmental ladder's emission is unchanged
  ([G12](#g12-gate-marked-signature)'s compatibility clause).
- **`must_answer_the_raised(raised: &::rung::Raised, evidence:
  &::rung::Terminated)`** — the terminal guard
  ([G16](#g16-the-residual-channel)). Emitted under the same condition as
  [3.4](#emitted-suspended).

<a id="emitted-functions" data-parent="emitted-module"></a>
**3.8** **Transition and recover functions** (when an `impl` block is present) —
one `pub fn` per transition/recover, expanded from the corresponding body
*inside* the module. A forward transition returns its target rung; a branching
transition returns `Result<StepOutcome, Failed<from>>`; a recover function
returns its target rung. Omitting the `impl` block yields a type-only
declaration (no functions).

<a id="unmarked-signature" data-parent="emitted-functions"></a>
**3.81** **Unmarked:** `pub fn active(spec: Spec) -> Active`.

<a id="judgmental-signature" data-parent="emitted-functions"></a>
**3.82** **`#[judgmental(R)]`:** `pub fn active(spec: Spec, q:
::rung::Qualified<R>) -> Active` — a second parameter, taken by value. Its name
comes from the body's *second* closure input (`active = |spec, q| { .. }`) when
there is one; otherwise it is bound to `_q` and consumed unread. The body is
preceded by the injected binding prologue `must_be_bound_to(&spec.payload, &q);`
([G13](#g13-token-binding)), so the source rung's payload MUST implement
`::rung::Provenanced`.

<a id="judgmental-outcome-bound" data-parent="emitted-functions"></a>
**3.83** A **forward** `#[judgmental(R)]` transition is also *followed* by the
injected outcome epilogue `must_derive_from_judge(&out.payload, &judge_prov);`
([G15](#g15-outcome-provenance)), so the *target* rung's payload MUST implement
`::rung::Provenanced` as well as the source's. A branching judgmental
transition gets the prologue and no epilogue.

<a id="resume-signature" data-parent="emitted-functions"></a>
**3.84** **`resume`:** `pub fn revive(s: Suspended<Posed>, evidence:
::rung::Terminated, pen: ::rung::Authorized<'_, R>) -> Posed`. The second and
third parameter names come from the body's second and third closure inputs when
there are any; otherwise they are `_evidence` and `_pen` and are consumed
unread. Two prologues are injected — `must_hold_standing_over(&s.token.payload,
&pen)` and `must_answer_the_raised(&s.raised, &evidence)` — so the source rung's
payload MUST implement `::rung::Situated`. **No `must_progress`**
([G16](#g16-the-residual-channel)).

<a id="authorial-signature" data-parent="emitted-functions"></a>
**3.85** **`#[authorial(R)]`:** `pub fn revised(filed: Filed, pen:
::rung::Authorized<'_, R>) -> Revised` — a second parameter, taken by value. Its
name comes from the body's *second* closure input when there is one; otherwise
it is bound to `_pen` and consumed unread. The body is preceded by the injected
standing prologue `must_hold_standing_over(&filed.payload, &pen);`
([G14](#g14-the-authorial-gate)), so the source rung's payload MUST implement
`::rung::Situated`.

<a id="body-name-resolution" data-parent="emitted-module"></a>
**3.9** Inside body expressions, rung/verdict names resolve unqualified;
payload types resolve from the surrounding scope (`use super::*`).

---

## 4 · Guarantees

<a id="guarantees" data-numbering="G"></a>
**4** Each guarantee is normative and **names the conformance test that fails
if the implementation stops honoring it**. Guarantees delegated to the Rust
compiler are marked *(rustc)*.

<a id="g1-linear-consumption" data-parent="guarantees"></a>
**G1** **Linear consumption.** A transition consumes its input rung by value;
using a rung after it is moved MUST be a compile error. *(rustc — move
semantics.)*

<a id="g2-sealed-construction" data-parent="guarantees"></a>
**G2** **Sealed construction.** A rung MUST NOT be constructible by struct
literal outside its module (`_seal` is private). When an `impl` block is
present, only the **entry** rung's `new` is public; every other rung's `new` is
module-private, so no code outside the module can mint a mid-ladder rung.
Categorically this is not merely a fabrication guard: it enforces that a *verb
cannot occupy object-position* — a state is reached only by traversing an arrow,
never fabricated to hold an arrow's result (see
[1.3](rung-ct-props.md#the-law)). That enforcement reaches exactly
as far as this guarantee does: a type-only declaration publishes every
constructor, and is freely generated by convention only (see
[1.41](rung-ct-props.md#freeness-enforced-only-with-bodies)).
*Conformance: `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`
— a `trybuild` case whose committed `.stderr` pins **E0624, associated function
`new` is private**, as the sole error. The `rung/src/lib.rs` compile_fail
doctest shows the same refusal in rustdoc but does not assert the code
([6.1](#compile-fail-asserts-only-non-compilation)).*

<a id="g3-one-token-one-thread" data-parent="guarantees"></a>
**G3** **One token, one thread.** Every rung and verdict MUST be `!Send +
!Sync` (via `PhantomData<*const ()>`), so a shared `Arc`/`&` cannot carry it
across a thread boundary. *Conformance:
`compile_pass.rs::test_rungs_are_not_send_or_sync` (rungs and verdicts).*

<a id="g4-no-silent-drop" data-parent="guarantees"></a>
**G4** **No silent drop.** Every rung, verdict, `StepOutcome`, and `Failed` MUST
be `#[must_use]`. Dropping a token in statement position is a warning — an error
under `#![deny(unused_must_use)]`. *Conformance:
`spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error` — a
`trybuild` case whose committed `.stderr` pins the denied `unused_must_use` lint
and the macro's contract-specific note. The `rung/src/lib.rs` must_use
compile_fail doctest documents it but does not assert the diagnostic
([6.1](#compile-fail-asserts-only-non-compilation)). (Escapable by `mem::forget`
/ `let _ =` — see [5.4](#drop-proofing-beyond-the-lint).)*

<a id="g5-carry-immutability" data-parent="guarantees"></a>
**G5** **Carry immutability.** `Carry` MUST be a private field exposed only
through `&Carry`; a transition body cannot mutate it. *Conformance:
`compile_pass.rs::test_carry_accessor_exists`.*

<a id="g6-exhaustive-outcomes" data-parent="guarantees"></a>
**G6** **Exhaustive outcomes.** `StepOutcome` is an enum; every match site MUST
handle all variants. *(rustc — enum exhaustiveness.)*

<a id="g7-recover-pairing" data-parent="guarantees"></a>
**G7** **Recover pairing.** Every recoverable verdict has a matching recover
function and vice versa; terminal verdicts have none
([2](#macro-must-reject), rules 4–7). *(macro — static checks.)*

<a id="g8-recovery-progress" data-parent="guarantees"></a>
**G8** **Recovery progress.** The macro MUST wrap every *verdict* recover body
with `must_progress`, comparing the source rung's payload to the produced rung's
payload and panicking if equal. The body cannot skip it. Requires the shared
payload type be `Clone + PartialEq`. *Conformance:
`end_to_end.rs::recover_guard_is_auto_injected` (panics with no explicit call).*

<a id="g9-error-path-recovery" data-parent="guarantees"></a>
**G9** **Error-path recovery.** A `recover { .. : Failed(R) => R }` function
receives the `Failed` and returns the next rung. No progress guard is injected
(a retry may reuse the token). *Conformance:
`end_to_end.rs::recovers_from_the_failed_error_path`.*

<a id="g10-continue-arms" data-parent="guarantees"></a>
**G10** **Continue arms.** A `V -> R` arm makes `step` build the next rung
inline; `StepOutcome::V` carries that rung directly — no recover function, no
guard, no source. *Conformance:
`end_to_end.rs::continue_arm_loops_without_a_recover_fn`.*

<a id="g11-terminal-payloads" data-parent="guarantees"></a>
**G11** **Terminal payloads.** A `V(P)` terminal verdict returns a value through
the verdict, read via `.payload()` / `.into_payload()`. *Conformance:
`end_to_end.rs::drives_to_convergence` asserts the returned payload.*

<a id="g12-gate-marked-signature" data-parent="guarantees"></a>
**G12** **Gate-marked signature.** A `#[judgmental(R)]` transition MUST take a
second parameter of type `::rung::Qualified<R>`, by value; an unmarked
transition MUST emit byte-for-byte what it emitted before markers existed. Two
gates are therefore two *signatures*, separated by the host's type system rather
than by a convention someone keeps
([11.31](rung-het-props.md#two-signatures-not-two-fragments)).
`Qualified` has no public constructor — `Pool::qualify` is the only mint — so a
judgmental transition cannot be called without an outside, and a decidable one
has no parameter an outside could enter through
([11.32](rung-het-props.md#decidable-cannot-consult-pool)).
**This makes the signature honest; [G13](#g13-token-binding) is what binds the
token to an argument** — see [5.7](#gate-faithfulness-not-secured) for what
neither secures. *Conformance:
`gate_markers.rs::judgmental_transition_takes_a_qualified_token` (the emitted
`fn` is coerced to a `fn` pointer of the exact expected type), and the
`tests/ui/` `trybuild` cases: `gate_missing_token` → E0061, `gate_forged_token`
→ E0451.*

<a id="g13-token-binding" data-parent="guarantees"></a>
**G13** **Token binding.** The macro MUST prefix every `#[judgmental(R)]`
transition body with `must_be_bound_to(&<source>.payload, &<token>)`, which
panics unless the token's recorded `π(a)` equals the source rung payload's. The
body cannot skip it, exactly as it cannot skip [G8](#g8-recovery-progress)'s
`must_progress`, and for the same reason: the body is the domain's, so a
guarantee the body could omit is not a guarantee. This requires the source
rung's payload to implement `::rung::Provenanced` — without `π(a)` there is
nothing to measure.

A `Qualified<R>` records the argument it was measured against alongside the
principal, and `Qualified::admit` is the one gate that spends it. The seal
([G12](#g12-gate-marked-signature)) closes *fabrication* — nobody can write a
token. G13 closes *transfer* — nobody can spend an honestly-earned token on an
argument it was never measured against, which is the act
[3.51](rung-het-props.md#disjointness-against-argument)
forbids and the pair
[3.54](rung-het-props.md#non-identity-by-construction)
requires the token to witness.

It panics rather than returning an error because a marked transition's return
type is the *domain's* declaration; there is no `Err` variant to route a refusal
through, and a P0 violation is not a recoverable step outcome. The library-level
consumers that do own their return type — `rung_het::dispose`, `theory!`'s
`settle` — return `TokenNotBound` instead. *Conformance:
`gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`
(the ladder's judgmental body never mentions its token, and a transferred token
is refused anyway), and
`rung-het/tests/token_binding.rs::{dispose_refuses_a_token_minted_against_the_model,
settle_refuses_a_token_minted_against_a_different_model}`.*

<a id="g14-the-authorial-gate" data-parent="guarantees"></a>
**G14** **The authorial gate.** An `#[authorial(R)]` transition MUST take a
second parameter of type `::rung::Authorized<'_, R>`, by value, and the macro
MUST prefix its body with `must_hold_standing_over(&<source>.payload, &<pen>)`,
which panics unless the pen's container equals the source rung payload's. This
requires the source rung's payload to implement `::rung::Situated` — without a
container there is nothing standing could be held over. Unmarked and
`#[judgmental(R)]` emission is unchanged.

**G14 is not [G12](#g12-gate-marked-signature)+[G13](#g13-token-binding) with a
different token name, and an implementation that made it one would satisfy every
clause above while enforcing nothing.** The two gates run over one pool and
select opposite predicates
([3.4](rung-het-props.md#one-pool-two-filters)):

| | judgmental | authorial |
|---|---|---|
| qualifying set | [3.5](rung-het-props.md#judgmental-qualifying-set) | [3.6](rung-het-props.md#authorial-qualifying-set) |
| second conjunct | `π(p) ∩ π(a) = ∅` — **disjointness** | `standing(p, M)` — **standing** |
| reading | you did **not** author this | this is **yours to revise** |
| admissibility | `π(f(a)) ∩ π(a) = ∅` | `π(f(a)) ⊆ π(p) ∧ standing(p, a)` |

Provenance overlap is what disqualifies a judge and what an author needs
([3.62](rung-het-props.md#provenance-overlap-is-the-point)),
so a principal that passes one filter has, on that evidence, said nothing about
the other and typically fails it
([3.61](rung-het-props.md#judgment-refuses-authorship-requires)).
`Pool::authorize` MUST therefore check **both** conjuncts of
[3.6](rung-het-props.md#authorial-qualifying-set):
standing alone mints no pen. It refuses on the judgmental branch of
[3.63](rung-het-props.md#standing-conditional-gated)
rather than guessing — closing that branch needs a judge, terminating at depth
one
([3.64](rung-het-props.md#standing-terminates-at-depth-one)),
whose own qualification is non-identity relative to the **author**
([3.65](rung-het-props.md#standing-judge-disjoint-from-author)).

The pen is borrowed-lifetime rather than consumed-and-gone in the library
(`Proposal::remedy`, `enact` take `&Authorized`): standing is not spent by a
single act, unlike a judgment licence, which is spent because each dispatch
re-runs the filter against a different argument. A `ladder!` transition takes the
pen by value only because a transition consumes its inputs; nothing was spent,
and the same principal may mint another on the next rung.

*Conformance: `gate_markers.rs::authorial_transition_takes_an_authorized_pen`
(the emitted `fn` is coerced to a `fn` pointer of the exact expected type, and
the ladder drives end to end);
`::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`
(the ladder's authorial body never mentions its pen, and a pen minted over
another container is refused anyway);
`::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` — **the
asymmetry test**: a steward that is not capable of the declared role is refused,
a principal that qualifies as a *judge* of the subject gets no pen, and the
principal that does hold the pen is refused as a judge of the same subject.
Dropping the capability conjunct from `Pool::authorize` reddens it; stubbing the
injected prologue reddens the prologue test. And the `tests/ui/` `trybuild`
cases: `gate_authorial_missing_pen` → E0061, `gate_forged_pen` → E0451,
`gate_authorial_no_role` → the macro's `compile_error!`.*

<a id="g15-outcome-provenance" data-parent="guarantees"></a>
**G15** **Outcome provenance.** The macro MUST follow every `#[judgmental(R)]`
**forward** transition body with `must_derive_from_judge(&<out>.payload,
&<π(p) snapshot>)`, which panics unless the returned payload's provenance is
contained in the qualifying principal's. This requires the *target* rung's
payload to implement `::rung::Provenanced` — without $\pi(f(a))$ there is
nothing to measure. The snapshot is taken in the prologue, because the body
consumes the licence; and the body runs inside an immediately-invoked closure,
so a `return` in it cannot step over the check. Unmarked, `#[authorial(R)]` and
*branching* judgmental emission is unchanged.

[G13](#g13-token-binding) constrains the arrow's **argument**; this constrains
its **outcome**, and they are the two halves of
[5.41](rung-het-props.md#admissibility-subcategories). Without G15 a body may
hold an honest licence, bound to the very argument it is applied to, and hand
that argument straight back out — the constant arrow
[5.4](rung-het-props.md#constant-arrow-hazard) names, expressed as a ladder.

**It asserts containment, not disjointness, and that is the point.** With G13
having just re-established $\pi(p) \cap \pi(a) = \emptyset$ for this argument,
$\pi(f(a)) \subseteq \pi(p)$ entails
$\pi(f(a)) \cap \pi(a) = \emptyset$
([5.42](rung-het-props.md#judgment-provenance-is-the-judges)). A disjointness
epilogue on top would assert the conclusion of a derivation whose premises are
both enforced. Containment is also the half a lying body cannot satisfy by
stamping: `::rung::Judgment` has no constructor outside `rung`, so a payload
whose $\pi$ derives from one carries a provenance its producer did not choose.

**Forward transitions only.** A branching judgmental transition returns a sum
whose recoverable and continue arms carry the argument onward by design —
re-entry rather than laundering
([7.44](rung-het-props.md#reproposal-carries-the-chain)) — so which arms are
*outcomes* in the sense of [5.41](rung-het-props.md#admissibility-subcategories)
is unsettled, and the epilogue does not guess. Recorded as an open limit in
`questions/open/q11-gate-faithfulness.md`.

*Conformance:
`gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render`
(a judgmental body that returns its own argument; deleting the injected call
reddens it) and
`::a_judgmental_arrow_may_not_return_the_provenance_it_judged` (the arrow whose
outcome is built on the judge's `Judgment`; minting that `Judgment` with the
argument's provenance instead of the judge's reddens it).*

<a id="g16-the-residual-channel" data-parent="guarantees"></a>
**G16** **The residual channel, and the arrow back.** A `#[judgmental(R)]`
**forward** transition MUST return `Result<Next, Suspended<Prev>>`, so that a
dispatch which cannot be settled now can hand the argument back **unconsumed**
together with the opaque reference to what was raised. A `resume` edge MUST take
an `::rung::Authorized<'_, R>` pen and `::rung::Terminated` evidence, MUST be
prefixed with `must_hold_standing_over` and `must_answer_the_raised`, and MUST
NOT be wrapped in `must_progress`. Unmarked, `#[authorial(R)]` and *branching*
judgmental emission is unchanged.

**This adds no summand.** The residual is the final `+ A` Het's judgmental
arrow already carries
([5.25](rung-het-props.md#judgmental-arrow-shape)), and a
judge that exists and has not answered is adequacy **undischarged**, which
[6.55](rung-het-props.md#adequacy-failure-returns-residual) already returns as
that residual. What G16 supplies is the *channel*: before it, a forward
judgmental transition returned its target rung and had nowhere to put the
argument, so a theory whose principal could not answer yet had no term for
saying so — the suspension existed in the formalism and not in the language.

**The pen is forced, not chosen.** Resuming produces a rung of this ladder, and
[G2](#g2-sealed-construction) seals that construction against everything outside
the module. The edge must therefore be emitted *inside* the module — and an
edge inside the seal that any caller may invoke is the seal with a door in it.
So resumption dispatches through the authorial filter
([6.552](rung-het-props.md#resumption-is-authorial)): capability and standing
over the container the subject sits in, the same shape as `enact`. The judge
that ruled on the raised matter cannot be the principal that resumes — it
qualified by provenance-disjointness, which is what denies it standing
([3.62](rung-het-props.md#provenance-overlap-is-the-point)).

**The absent guard is the point.** [G8](#g8-recovery-progress) exists because a
recover edge that returns its own source is an infinite stall. A resume edge
that returns its own source is the *normal case*: the argument was never
consumed, the raised matter took another round, and nothing about the subject
should have changed. A progress guard here would refuse the shape rather than a
bug, and would be the bound Het declines to declare
([12.5](rung-het-props.md#guarded-reentry-is-eviction)).

**What it does not promise.** Termination. A raised matter that never terminates
yields no `Terminated`, and the arrow stays suspended
([6.5521](rung-het-props.md#resumption-needs-a-terminal)). Nor does it survive
process death — see [5.6](#suspension-is-in-process-only).

*Conformance: `suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed`
(the emitted `fn` is coerced to a `fn` pointer of the exact expected type, and
the returned token is the very argument);
`::a_suspension_resumes_through_the_authorial_edge` (the round trip, with the
resume edge coerced to its exact pointer type);
`::the_same_suspension_resumes_twice_with_no_progress_guard` — **the unguarded
test**: injecting `must_progress` on the resume edge is type-valid and reddens
it on the first round; `::resume_refuses_a_pen_over_another_container` (the
body never mentions the pen, and a pen over another container is refused
anyway — deleting the injected `must_hold_standing_over` reddens it);
`::resume_refuses_evidence_from_another_raised_matter`. And the `tests/ui/`
`trybuild` cases: `resume_without_a_pen` → the macro's `compile_error!`,
`resume_missing_pen` → E0061.*

---

## 5 · Non-guarantees

<a id="non-guarantees"></a>
**5** Explicitly out of scope. The macro does **not** enforce the following, and
a claim that it does has no standing.

<a id="transition-body-correctness" data-parent="non-guarantees"></a>
**5.1** **Transition-body correctness.** The type proves a transition *ran*, not
that its logic was valid — the boundary between typestate and formal
verification.

<a id="cross-crate-provenance" data-parent="non-guarantees"></a>
**5.2** **Cross-crate provenance.** A rung crossing a crate boundary is trusted,
like any Rust public API. Sealing this needs a sub-crate per ladder.

<a id="same-module-fabrication" data-parent="non-guarantees"></a>
**5.3** **Same-module / entry fabrication.** [G2](#g2-sealed-construction) stops
*external* fabrication; code inside the generated module, and the public entry
constructor, can still build rungs — the module-boundary limit Rust always has.

<a id="drop-proofing-beyond-the-lint" data-parent="non-guarantees"></a>
**5.4** **Drop-proofing beyond the lint.** [G4](#g4-no-silent-drop) is
`#[must_use]`, which `mem::forget`, `let _ = token;`, or burying the token in a
dropped container all bypass. True no-drop needs language-level linear types.

<a id="liveness-beyond-the-guard" data-parent="non-guarantees"></a>
**5.5** **Liveness beyond the guard.** [G8](#g8-recovery-progress) catches an
identical-token stall loop; it does not prove general forward progress.

<a id="suspension-is-in-process-only" data-parent="non-guarantees"></a>
**5.6** **Suspension does not survive process death.**
[G16](#g16-the-residual-channel) suspends and resumes **in one process**: a
driver may hold a `Suspended<Prev>` in memory for as long as it likes, and that
is the whole of the claim. Writing one to disk and reconstituting it later is
not supported and is not merely unimplemented — a rung read back from bytes is a
mid-ladder rung nobody traversed to, which is exactly what
[G2](#g2-sealed-construction) exists to refuse. Resumption being authorial
answers *who may* revive a run; it says nothing about *how a token survives
serialization*. Filed as
[Q13](../questions/open/q13-suspension-across-process-death.md), and related to
[5.2](#cross-crate-provenance).

<a id="gate-faithfulness-not-secured" data-parent="non-guarantees"></a>
**5.7** **Gate-faithfulness.** [G12](#g12-gate-marked-signature) secures the
judgmental signature, [G13](#g13-token-binding) its argument,
[G14](#g14-the-authorial-gate) both halves of the authorial gate's *input*, and
[G15](#g15-outcome-provenance) the judgmental *outcome* of a forward
transition. What is still not secured is the outcome everywhere else, and one
of Het's four gates still has no signature.

<a id="one-gate-unimplemented" data-parent="gate-faithfulness-not-secured"></a>
**5.71** *One gate is unimplemented.* `#[conditional(..)]` is a parse-time
refusal, not an encoding. Gate-faithfulness is a condition on **every** operation
of an algebra, so an algebra with a conditional arrow cannot state it here at
all.

<a id="returned-value-unconstrained" data-parent="gate-faithfulness-not-secured"></a>
**5.72** *The returned value is constrained judgmentally, and only there.* This
non-guarantee used to read "the returned value is unconstrained," and it was
exact: `Prov::contained_in` existed and no guarantee called it. Two now do.
`theory!`'s `settle` takes a sealed `Judgment` rather than a `Verdict` and
refuses `π(f(a)) ⊄ π(p)`; [G15](#g15-outcome-provenance) injects the same check
as an epilogue on a forward judgmental transition. Disjointness —
[5.41](rung-het-props.md#admissibility-subcategories)'s judgmental clause — is
not checked because it is entailed
([5.42](rung-het-props.md#judgment-provenance-is-the-judges)).

The residue is stated at [5.721](#outward-conditions-remaining) rather than
absorbed into a claim that the outward side is closed. It is not.

<a id="outward-conditions-remaining" data-parent="returned-value-unconstrained"></a>
**5.721** *Two outward conditions remain.* First, the **authorial** one:
[5.41](rung-het-props.md#admissibility-subcategories) states the authorial
clause as `π(f(a)) ⊆ π(p) ∧ standing(p, a)`, and
[G14](#g14-the-authorial-gate) secures the standing conjunct on the way in
while leaving the containment conjunct on the way out entirely to the body —
the same shape as [G13](#g13-token-binding)'s gap, on the second gate. Second,
**branching** judgmental transitions take the prologue and no epilogue, because
a branching outcome is a sum whose recoverable and continue arms carry the
argument onward by design
([7.44](rung-het-props.md#reproposal-carries-the-chain)), and which of those
arms is an *outcome* in [5.41](rung-het-props.md#admissibility-subcategories)'s
sense is not settled. Both inherit [5.1](#transition-body-correctness) whole,
as the whole outward side used to.

<a id="decidable-is-not-pure" data-parent="gate-faithfulness-not-secured"></a>
**5.73** *Decidable is not pure.* The unmarked signature excludes Het's outside
— the principal pool — and is silent about clocks, files, and networks
([11.42](rung-het-props.md#purity-not-secured)).

<a id="type-only-marker-is-inert" data-parent="gate-faithfulness-not-secured"></a>
**5.74** *A type-only declaration emits no transitions,* so a marker on one has
nothing to constrain and is inert, exactly as [G2](#g2-sealed-construction)'s
seal is
([1.41](rung-ct-props.md#freeness-enforced-only-with-bodies)).

<a id="gate-faithfulness-answered-no" data-parent="gate-faithfulness-not-secured"></a>
**5.75** Whether [G12](#g12-gate-marked-signature) +
[G13](#g13-token-binding) + [G14](#g14-the-authorial-gate) +
[G15](#g15-outcome-provenance) amount to gate-faithfulness is argued — and
answered *no* — in [Q11](../questions/open/q11-gate-faithfulness.md), which stays
open on [5.71](#one-gate-unimplemented) and, in its narrowed form,
[5.721](#outward-conditions-remaining).

<a id="a-cycle-through-an-authorial-act-cannot-close" data-parent="non-guarantees"></a>
**5.8** **A cycle that must pass through an authorial act cannot close inside
one `ladder!`.** [1.1](#declaration-grammar) declares a **linear spine** with
backward continue arms. A continue arm's target rung is built *inline by
`step`* ([G10](#g10-continue-arms)), so every arm of the branching transition is
authored by whoever holds that transition's token. Where `step` is
`#[judgmental(R)]`, an arm returning to the ladder's **entry** rung would
therefore have the judge produce the revised subject — the amendment
[7.42](rung-het-props.md#no-amending-disposition) forbids.

The audit–rectify pass is the case. Het states that `enact` is what makes the
pass an **endofunctor** rather than a one-way funnel into a verdict
([7.5](rung-het-props.md#enact-makes-an-endofunctor)); `rung-het`'s
`het_pass!` therefore stops one arrow short, with `Accept` terminal and
carrying a licence, and `enact` a **separate** authorial arrow consuming that
licence and a pen. The loop closes by **composition** — feeding `enact`'s
result into a fresh run — and not within the declaration.

So `ladder!` does not express the endofunctor, and a claim that a single
declaration is one has no standing here. Whether a rung's payload may be a
completed sub-ladder run — which is the shape that would let the composite be
declared rather than driven — is
[Q4](../questions/open/q4-composition-nested-ladders.md), open.

---

## 6 · Conformance

<a id="conformance-suite"></a>
**6** The conformance suite is `rung/tests/` and the doctests in
`rung/src/lib.rs`. A change that violates any guarantee above MUST break at
least the cited test. The README's Getting Started example is itself a run
doctest (via `include_str!`), so the documented public API cannot silently drift
from the macro.

<a id="compile-fail-asserts-only-non-compilation" data-parent="conformance-suite"></a>
**6.1** **A `compile_fail` doctest does not verify the error code.** rustdoc
ignores the `E0NNN` in a fence such as `compile_fail,E0999`, and E0999 does not
exist — the block passes. So a `compile_fail` doctest asserts exactly one thing:
*this did not compile*. It cannot tell the refusal it was written for from a
typo, an unresolved import, a missing `main` (E0601), or a name that fell out of
scope when rustdoc wrapped the snippet in a `fn main` of its own. Adding the code
annotation does not fix this; nothing reads it.

<a id="no-guarantee-cites-a-compile-fail-doctest" data-parent="conformance-suite"></a>
**6.2** Consequently **no guarantee may cite a `compile_fail` doctest as its
conformance test.** Refusals are pinned by `trybuild` cases in `rung/tests/ui/`,
which diff the full rendered stderr against a committed `.stderr` snapshot, so
the code and the message are both part of the assertion. The doctests are kept
alongside — they are the documentation, and a reader meets the refusal in
rustdoc — but they are the illustration, not the evidence.

<a id="two-silent-doctest-traps" data-parent="conformance-suite"></a>
**6.3** Two further traps, both found in this repo and both silent:

- A doctest with no `fn main` is wrapped in one by rustdoc, so `struct` and
  `macro_rules!` items land in a function body. A `ladder!`-generated `mod` then
  cannot see them and the example fails on E0425 — a green test asserting
  nothing about the guarantee. Write an explicit `fn main` when the snippet
  declares items.
- A struct literal that omits a field fails with E0063 whether or not the fields
  are private, so a "cannot be forged" example with a stale field list keeps
  passing after the seal is removed. Name every field.

<a id="a-refusal-test-that-cannot-fail" data-parent="conformance-suite"></a>
**6.4** **A refusal test that cannot fail is not a guarantee.** The way to
establish that a case can fail is to make the guarded thing legal and watch the
case go red.

---

## 7 · Design judgments

<a id="design-judgments" data-numbering="J"></a>
**7** The propositions above are settled by the macro, by rustc, or by a named
test. The judgments below are not: **no machine decides them.** They govern how
rung is *used* — where a ladder should stop, and what earns a place in
`rung-std`. They are earned through use rather than derived from first
principles; amend them when a new case does not fit, but amend them
deliberately, as a ruling on the record.

<a id="j1-where-the-tower-bottoms-out" data-parent="design-judgments"></a>
**J1** **Where does the tower bottom out?** A rung ladder should terminate where
**structural enforcement stops buying correctness gains**.

The floor of a tower is not defined by the line between "our code" and "a
library." That line is arbitrary — hermes-agent is not our code, and yet
inner-loop models its inner loop as a rung ladder. The floor is not defined by
the boundary between user space and kernel space either; you could keep extending
the tower through syscalls to hardware interrupts. The question is not ownership,
it is leverage.

Ask: *would a rung ladder over the states below this point catch any wrong
transition that the existing infrastructure does not already catch?*

If the answer is no — if the external code (a library, the OS, a protocol
implementation) already handles its own state correctly, and a ladder over it
would add ceremony without catching anything the type system does not already
enforce — then the tower terminates here.

**The principled floor is where structural enforcement stops buying correctness
gains. Everything above that line is ours to model; everything below it is
someone else's type system doing its job.**

Worked example: `raw_call` in `rung-std::llm` is a plain function — one blocking
HTTP POST, `Result<String, RawCallError>`. There are no wrong state transitions
to prevent below it; `reqwest` already handles the I/O correctly. The ladder
above (`LlmCall`) is where states live: the counter check, the attempt in flight,
the terminal verdicts. That is the right floor.

<a id="j2-what-belongs-in-rung-std" data-parent="design-judgments"></a>
**J2** **What belongs in rung-std?** A ladder belongs in `rung-std` when it
satisfies two conditions:

1. **It recurs across unrelated domains.** Independent projects would otherwise
   rediscover the same shape — often collapsing rungs that should be distinct,
   or hiding retry logic inside a single morphism body that should be two rungs.
2. **The canonical statement is better than any project's derivation.** A project
   that needs this computation is better served by depending on the correct
   formulation than by writing their own. The value is in the shape — the right
   rung boundaries, the right terminal verdicts — not in the implementation
   details.

**The test:** would a project that needed this computation be better off
depending on the canonical statement than deriving their own? If yes, it belongs
in rung-std. If the ladder carries vocabulary that only makes sense in one
domain, it stays in that project.

**Corollary — rung-std is not a kitchen sink.** A ladder that is merely *useful*
does not earn a place in rung-std. The bar is *recurrent and domain-generic*. A
ladder that happens to appear in two projects but carries domain-specific names
should be abstracted at the domain level, not elevated to rung-std.

Worked examples:

- `LlmCall` (Pending → Calling → {Success | AuthError | Exhausted}) **belongs in
  rung-std** — the bounded-retry + terminal-classification shape recurs wherever
  an LLM is called, across every project in the keiretsu and beyond. GL's
  collapsed single-rung version is a regressed form of the canonical two-rung
  statement; the stdlib is the correction.
- `InnerLoop` (Idle → Calling → {EndTurn | MaxIterations | ...}) **may belong in
  rung-std** — the agentic turn loop shape recurs across any framework that
  drives tool-calling agents. Worth watching whether a second independent
  derivation confirms the shape.
- An audit ladder written against one project's own schema **does not belong** —
  it carries domain vocabulary (that project's verdict kinds, its boundary
  conditions) that is meaningful only to garden-ladders.
