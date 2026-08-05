//! `rung-props.md`, encoded.
//!
//! **Generated once** from `docs/rung-props.md` by `docs/_migrate.py`, and the
//! source of truth from then on. The markdown is rendered from this; where the
//! two disagree, this is right and the markdown is stale.
//!
//! Every proposition arrives as [`Kind::Rationale`], which is not a claim that
//! they are all arguments — it is the absence of a claim. Markdown does not
//! record what kind a proposition is, so the migration does not invent one. The
//! triage into signature, decidable and judgmental is a reading, done
//! deliberately, and it is the work this encoding exists to make possible.

use crate::{Doctrine, Element, Kind, Prop};

/// The doctrine `docs/rung-props.md` is rendered from.
pub fn doctrine() -> Doctrine {
    Doctrine {
        file: "rung-props.md".into(),
        elements: vec![
        Element::Verbatim(r#"# rung — The ladder the macro declares

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

"#.into()),
        Element::Prop(Prop {
            slug: "declaration-is-a-block".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"A `ladder!` invocation is a **declaration block**, optionally followed by
an inline `impl` block supplying the transition logic.

```
ladder!( Name { <declaration> } [ impl { <bodies> } ] )
```

"#.into(),
            mechanism: r#"The macro accepts exactly this shape. The cited ladder is a declaration block followed by an inline `impl` block and is driven to a terminal verdict, so both halves of the form are exercised by a run rather than by an expansion that merely typechecks."#.into(),
        }),
        Element::Prop(Prop {
            slug: "declaration-grammar".into(),
            parent: Some("declaration-is-a-block".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_module_exists".into() },
            numbering: None,
            prose: r##"The declaration MUST match:

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
marker is **not optional** — see {#g16-the-residual-channel}.

"##.into(),
            mechanism: r#"The cited declaration uses every production of the grammar at once — a `carry` block, a multi-hop spine, a verdict block carrying a terminal marker, a recoverable verdict, and a `recover` edge. A production the parser dropped would fail to expand. The refusals that keep the grammar from accepting *more* than this are {#macro-must-reject}."#.into(),
        }),
        Element::Prop(Prop {
            slug: "bodies-grammar".into(),
            parent: Some("declaration-is-a-block".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"The inline `impl` block, when present, MUST match:

```
bodies := ( ident "=" closure [ "," | ";" ] )*        -- one per transition/recover fn
closure := "|" pat "|" ( block | expr )
```

"#.into(),
            mechanism: r#"The cited ladder supplies three inline bodies in the `ident = closure` form, comma-separated, mixing block and expression closures. They expand into the module and are called by the driver."#.into(),
        }),
        Element::Prop(Prop {
            slug: "transition-naming".into(),
            parent: Some("declaration-is-a-block".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"A forward transition (`A => B`) is **named after its target** rung,
lowercased (`B` ⇒ `b`). The branching transition (`A => { .. }`) is named
`step`. A recover function is named by its `recover` edge. Inline body names
MUST match these ({#macro-must-reject}, rules 9–10).

"#.into(),
            mechanism: r#"The driver calls `opt::active`, `opt::step` and `opt::iterate` by those names — the target lowercased, `step` for the branching transition, the recover edge's own name. Renaming any of the three in the macro turns the call site into an unresolved path."#.into(),
        }),
        Element::Prop(Prop {
            slug: "marker-annotates-the-target".into(),
            parent: Some("declaration-is-a-block".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token".into() },
            numbering: None,
            prose: r#"A gate marker **annotates the transition's target**, because that is
what the transition is named after. A marker on a rung marks the forward
transition producing it; a marker on the verdict block marks `step`.

"#.into(),
            mechanism: r#"The cited ladder marks both markable positions — a rung, and the verdict block — and the test coerces `review::active` and `review::step` to `fn` pointers of the exact expected types. A marker that annotated the source rather than the target would put the parameter on the wrong function and both coercions would fail."#.into(),
        }),
        Element::Prop(Prop {
            slug: "at-most-one-marker".into(),
            parent: Some("marker-annotates-the-target".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::two_markers_on_one_transition_are_refused".into() },
            numbering: None,
            prose: r#"A transition carries **at most one** marker. Het's gates are
alternatives, not a set
({#four-gates}). An unmarked transition
reads as *decidable* and is emitted exactly as it was before markers existed
({#g12-gate-marked-signature}).

"#.into(),
            mechanism: r#"A `trybuild` case with `#[judgmental(R)] #[authorial(R)]` on one transition, whose committed `.stderr` holds the macro's message. The macro has refused this since markers landed; until the case existed nothing would have noticed if it stopped."#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-markers-implemented".into(),
            parent: Some("marker-annotates-the-target".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen".into() },
            numbering: None,
            prose: r#"Two of Het's four gates are implemented, and they are the two that
dispatch to an outside — in **opposite directions**
({#judgment-refuses-authorship-requires}):

| marker | emitted second parameter | filter |
|---|---|---|
| `#[judgmental(R)]` | `::rung::Qualified<R>` | `capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅` |
| `#[authorial(R)]` | `::rung::Authorized<'_, R>` | `capable(p, role(o)) ∧ standing(p, M)` |

One pool, two filters
({#one-pool-two-filters}); the
marker selects the predicate, not the pool. The two share their left conjunct
and take opposite second conjuncts, so the two tokens are different types and
neither can be passed where the other is asked for
({#g12-gate-marked-signature}, {#g14-the-authorial-gate}).

"#.into(),
            mechanism: r#"Both markers emit, and emit *different* second parameters — the cited test coerces the authorial transition to `fn(Filed, Authorized<'_, R>) -> Revised`, and `judgmental_transition_takes_a_qualified_token` does the same for `Qualified<R>`. A pen cannot be passed where a licence is asked for, which is the whole content of "two gates, two signatures"."#.into(),
        }),
        Element::Prop(Prop {
            slug: "conditional-marker-refused".into(),
            parent: Some("marker-annotates-the-target".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::conditional_is_refused_and_names_the_open_question".into() },
            numbering: None,
            prose: r#"`#[conditional(..)]` MUST be a `compile_error!`. It classifies per
model, one level up
({#classifier-one-level-up}),
and `ladder!`'s checks run at expansion time against a single declaration; see
[Q11](questions/open/q11-gate-faithfulness.md).

"#.into(),
            mechanism: r#"A `trybuild` case whose committed `.stderr` holds the refusal, including the pointer to the open question. A `compile_fail` doctest would not have distinguished this refusal from a typo ({#compile-fail-asserts-only-non-compilation})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "marker-without-role-refused".into(),
            parent: Some("marker-annotates-the-target".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::judgmental_without_a_role_is_refused".into() },
            numbering: None,
            prose: r#"Either implemented marker written **without a role** MUST be a
`compile_error!`. A judgmental role that is not named cannot resolve a judge
({#judgmental-declares-role}),
and an authorial marker naming none can witness only the right conjunct of
{#authorial-qualifying-set},
which would make the competence filter decorative. In both cases there is no
signature to emit.

"#.into(),
            mechanism: r#"Two `trybuild` cases, one per marker — the cited one for `#[judgmental]`, `authorial_without_a_role_is_refused` for its mirror. Both `.stderr` snapshots carry the reason, which is that there is no signature to emit rather than that the syntax is unfamiliar."#.into(),
        }),
        Element::Verbatim(r#"---

## 2 · Static semantics

"#.into()),
        Element::Prop(Prop {
            slug: "macro-must-reject".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::a_duplicate_carry_field_is_refused".into() },
            numbering: None,
            prose: r#"The macro MUST reject, with a `compile_error!` pointing at the violation,
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

"#.into(),
            mechanism: r#"All ten rules, each a `trybuild` case with a committed `.stderr`. Two of the ten are unreachable through the grammar rather than untested, and the suite says so where the reachable neighbour lands: rule 2 cannot be written because every rung of the spine is declared by the hop that introduces it, and rule 5's *missing recover function* clause cannot be written because one `recover` entry pushes the edge and the function together. Before these cases, seven of the ten were prose the macro happened to implement."#.into(),
        }),
        Element::Prop(Prop {
            slug: "structural-rules-mirror-the-reference-checker".into(),
            parent: Some("macro-must-reject".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Rules 1–8 are structural and mirror the Python reference checker
(`.archive/python-poc/rung/checker.py`, verified in sync).

"#.into(),
            mechanism: r#"A provenance note about a retired artifact. The Python checker is under `.archive/`, nothing in the workspace depends on it, and "verified in sync" records a comparison made once by hand rather than a property anything re-checks. What the note is *about* — that rules 1–8 are structural — is now pinned rule by rule under {#macro-must-reject}."#.into(),
        }),
        Element::Prop(Prop {
            slug: "body-rules-need-an-impl-block".into(),
            parent: Some("macro-must-reject".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_module_exists".into() },
            numbering: None,
            prose: r#"Rules 9–10 apply **only** when an `impl` block is present.

"#.into(),
            mechanism: r#"The cited declaration omits the `impl` block entirely and expands cleanly, so rules 9–10 did not fire on a ladder with no bodies to check. That they *do* fire when the block is present is `spec_refusals.rs::an_impl_body_that_names_no_transition_is_refused` and `::an_impl_block_missing_a_body_is_refused`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "resume-rules-are-g2".into(),
            parent: Some("macro-must-reject".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_resume_edge_without_an_authorial_marker_is_refused".into() },
            numbering: None,
            prose: r#"Rule 11's three clauses are one clause: a resume edge is emitted
*inside* the module, so every way of declaring one that nothing gates, or that
nothing can reach, is {#g2-sealed-construction} with a door in it. The
marker clause is the sharpest — an unmarked spine transition reads as
*decidable* ({#at-most-one-marker}), and there is no decidable reading of
an arrow that writes a rung
({#resumption-is-authorial}).

"#.into(),
            mechanism: r#"The declaration-time refusals. The cited `trybuild` case declares a resume edge with no `#[authorial(R)]` marker and holds the macro's exact message: an edge emitted inside the seal that anyone may call is [G2](rung-props.md#g2-sealed-construction) with a door in it, so a penless resume is not a signature a caller forgot to satisfy — it is not declarable. Making the marker optional is type-valid and turns this red at the snapshot."#.into(),
        }),
        Element::Prop(Prop {
            slug: "extension-refusals-are-pinned".into(),
            parent: Some("macro-must-reject".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload".into() },
            numbering: None,
            prose: r#"The payload (rule 3), continue-target (rule 3), and `Failed(Rung)`
(rule 8) extensions are each pinned by a `trybuild` case —
`spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload`,
`::a_continue_arm_target_must_be_a_declared_rung`, and
`::a_failed_source_rung_must_be_declared`, whose committed `.stderr` snapshots
hold the macro's exact message. Each also appears as a `compile_fail` doctest in
`rung/src/lib.rs`, which documents the refusal but does not assert its
diagnostic ({#compile-fail-asserts-only-non-compilation}).

"#.into(),
            mechanism: r#"The proposition names its own three cases; this is the first of them. Each holds a committed `.stderr`, which is what makes the refusal's *message* part of the assertion rather than only its existence."#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · Emitted artifacts

"#.into()),
        Element::Prop(Prop {
            slug: "emitted-module".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_module_exists".into() },
            numbering: None,
            prose: r#"For a ladder `Name`, the macro emits a module `name` (the ladder name,
lowercased) containing the artifacts below.

"#.into(),
            mechanism: r#"Every path in the cited test goes through `metricoptimization::`, the ladder name lowercased. A module emitted under another name, or not emitted, is an unresolved path."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-carry".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_carry_accessor_exists".into() },
            numbering: None,
            prose: r#"**`Carry`** — if a `carry` block is present:
`#[derive(Clone, Debug)] pub struct Carry { pub <field>: <type>, .. }`.

"#.into(),
            mechanism: r#"`test_module_exists` constructs `Carry` with both declared fields by name, which needs the struct, the field names, and their public visibility. The cited test adds the accessor: a type-level coercion that only holds if `Spec::carry(&self) -> &Carry` exists with that exact signature."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-rung-structs".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync".into() },
            numbering: None,
            prose: r#"**One struct per rung** — sealed and thread-bound:
`pub struct R { _seal: (), _not_send: PhantomData<*const ()>, carry: Carry, pub
payload: P }`, `#[must_use]`. With a constructor `R::new(payload, carry)` and an
accessor `pub fn carry(&self) -> &Carry`. Constructor visibility follows
{#g2-sealed-construction}.

"#.into(),
            mechanism: r#"The seal and the thread-binding, which are the two clauses a host can lose silently. The cited test uses autoref specialization to assert `!Send` for rungs *and* verdicts; the `_seal` field is what `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` pins. Constructor visibility follows [G2](rung-props.md#g2-sealed-construction)."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-verdict-structs".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"**One struct per verdict** — sealed and thread-bound, `#[must_use]`:

- terminal marker: fields `{ _seal, _not_send }`, `new()`.
- terminal with payload `V(P)`: adds `payload: P`; `new(payload)`,
  `.payload()`, `.into_payload()`.
- recoverable `V => R`: adds `source: <from-rung>`; `new(source)`, `.source()`,
  `.into_source()`.
- a continue arm emits **no** verdict struct (its outcome carries a live rung).

"#.into(),
            mechanism: r#"All three shapes in one run: `Exhausted::new()` is the bare terminal marker, `Converged(Report)` is a terminal carrying a payload read back out through `.payload()`, and `Iterating => Active` is a recoverable verdict built from its source rung and unwrapped with `.into_source()`. The fourth clause — that a continue arm emits **no** verdict struct — is `end_to_end.rs::continue_arm_loops_without_a_recover_fn`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-suspended".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed".into() },
            numbering: None,
            prose: r#"**`Suspended<Prev>`** — `#[must_use] pub struct Suspended<Prev> { pub
token: Prev, pub raised: ::rung::Raised }`, with a hand-written `Debug` that
prints the reference and not the token. Emitted **only** when the ladder has a
`#[judgmental(R)]` forward transition or a `resume` edge, so that an unmarked
ladder's emission is unchanged ({#g12-gate-marked-signature}'s
compatibility clause).

It is **not** {#emitted-failed} widened. `Failed` carries an error
string, and routing a raised reference through one would make the theory's
identity for a raised matter indistinguishable from an error message; `Failed`
is also emitted for every ladder, so widening it would break the compatibility
clause outright. What the two share is the *shape*, `Result<_, Carrier<from>>`.

"#.into(),
            mechanism: r#"The residual channel as a type. The cited test coerces the emitted `fn` to a `fn` pointer of the exact expected type — `fn(Posed, Qualified<Adjudicator>) -> Result<Answered, Suspended<Posed>>` — so dropping the summand from the return type is a compile error at that line rather than a silently weaker signature, and it then reads the unconsumed token back out and finds the very argument. Emission is CONDITIONAL, which is what keeps {#g12-gate-marked-signature}'s compatibility clause true: an unmarked ladder emits no `Suspended` and its module is byte-identical."#.into(),
        }),
        Element::Prop(Prop {
            slug: "suspended-reports-what-it-awaits".into(),
            parent: Some("emitted-suspended".into()),
            kind: Kind::Decidable { proof: "rung-std/tests/driver.rs::a_parked_run_is_released_by_its_evidence_and_resumes_to_a_terminal".into() },
            numbering: None,
            prose: r#"The macro MUST also emit `impl<Prev> ::rung::Awaiting for
Suspended<Prev>`, returning the carried {#emitted-suspended}
unchanged. A run reports what it awaits **itself**, so that a holder of many
suspended runs can ask each the same question without naming its module.

The fields are already `pub`, so this adds no access. What it adds is that the
reference cannot be supplied *about* a run by whoever holds it. A holder keyed
by a caller-passed reference can be told a run awaits a matter it never raised —
the same fabrication {#g16-the-residual-channel} forecloses one
level down by deriving evidence from the `Raised` rather than accepting it
alongside. Reading it off the run leaves no parameter to lie through.

Het requires nothing further of the reference here: the trait hands back the
theory's value and declares no ordering, comparison or well-formedness over it
({#pool-is-opaque}).

"#.into(),
            mechanism: r#"The emitted `impl ::rung::Awaiting for Suspended<Prev>` is what lets a holder read what a run awaits off the run instead of being told. The cited suite parks suspensions from a real ladder and matches them by that trait alone; deleting the impl from the macro's emission makes the whole file fail to compile with `Suspended<Filed>: Awaiting is not satisfied`, because `Park<S>` is bounded on it. That the bound carries the claim — rather than a `raised` field read a holder could have done anyway — is the content of the proposition."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-step-outcome".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn".into() },
            numbering: None,
            prose: r#"**`StepOutcome`** — `#[must_use] pub enum` with one variant per verdict
of the branching transition. A continue arm's variant carries its **target
rung**; every other variant carries its **verdict struct**.

"#.into(),
            mechanism: r#"The clause that distinguishes `StepOutcome` from an ordinary verdict enum: a continue arm's variant carries a **live target rung**, not a verdict marker. The cited test reassigns that rung straight back into the driver, with no recover function and no guard in between."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-failed".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path".into() },
            numbering: None,
            prose: r#"**`Failed<Prev>`** — `#[must_use] pub struct Failed<Prev> { pub token:
Prev, pub error: String }`.

"#.into(),
            mechanism: r#"The cited test takes the error path and reads both fields back — the unconsumed `token` and the `error` string — which is what makes `Failed<Prev>` a recovery vehicle rather than a discarded value."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-guards".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recover_guard_is_auto_injected".into() },
            numbering: None,
            prose: r#"The guards the injected prologues call:

- **`must_progress<T: PartialEq>(before: &T, after: &T)`** — the recovery guard
  ({#g8-recovery-progress}).
- **`must_be_bound_to<A: Provenanced, R: Role>(argument: &A, licence:
  &::rung::Qualified<R>)`** — the token-binding guard
  ({#g13-token-binding}).
- **`must_hold_standing_over<A: Situated, R: Role>(subject: &A, pen:
  &::rung::Authorized<'_, R>)`** — the standing guard
  ({#g14-the-authorial-gate}). Emitted **only** when the ladder carries an
  `#[authorial(R)]` marker **or a `resume` edge**, so that an unmarked or
  judgmental ladder's emission is unchanged
  ({#g12-gate-marked-signature}'s compatibility clause).
- **`must_answer_the_raised(raised: &::rung::Raised, evidence:
  &::rung::Terminated)`** — the terminal guard
  ({#g16-the-residual-channel}). Emitted under the same condition as
  {#emitted-suspended}.

"#.into(),
            mechanism: r#"`must_progress` is the one an author cannot see: the cited ladder's recover body contains no call to it and panics anyway, because the macro wrapped the body ([G8](rung-props.md#g8-recovery-progress)). The other two guards are pinned the same way, at `gate_markers.rs::a_body_that_ignores_the_token_still_gets_the_binding_check` and `::a_body_that_ignores_the_pen_still_gets_the_standing_check`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "emitted-functions".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"**Transition and recover functions** (when an `impl` block is present) —
one `pub fn` per transition/recover, expanded from the corresponding body
*inside* the module. A forward transition returns its target rung; a branching
transition returns `Result<StepOutcome, Failed<from>>`; a recover function
returns its target rung. Omitting the `impl` block yields a type-only
declaration (no functions).

"#.into(),
            mechanism: r#"One `pub fn` per transition and per recover edge, expanded *inside* the module: the cited bodies call `Active::new`, which is private to the module and unreachable from the test file. A body expanded outside would not compile. The type-only case — no `impl` block, no functions — is `compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "unmarked-signature".into(),
            parent: Some("emitted-functions".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"**Unmarked:** `pub fn active(spec: Spec) -> Active`.

"#.into(),
            mechanism: r#"The driver calls `opt::active(spec)` with one argument. An unmarked transition that grew a second parameter is E0061 at that call site — which is the same diagnostic `gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061` pins from the other side."#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-signature".into(),
            parent: Some("emitted-functions".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token".into() },
            numbering: None,
            prose: r#"**`#[judgmental(R)]`:** `pub fn active(spec: Spec, q:
::rung::Qualified<R>) -> Active` — a second parameter, taken by value. Its name
comes from the body's *second* closure input (`active = |spec, q| { .. }`) when
there is one; otherwise it is bound to `_q` and consumed unread. The body is
preceded by the injected binding prologue `must_be_bound_to(&spec.payload, &q);`
({#g13-token-binding}), so the source rung's payload MUST implement
`::rung::Provenanced`.

"#.into(),
            mechanism: r#"The cited test coerces the emitted `fn` to `fn(review::Spec, Qualified<Reviewer>) -> review::Active`, so an absent, extra, or differently-typed second parameter fails to compile. The injected prologue is separately pinned by `gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`, whose ladder never reads the token."#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-outcome-bound".into(),
            parent: Some("emitted-functions".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render".into() },
            numbering: None,
            prose: r#"A **forward** `#[judgmental(R)]` transition is also *followed* by the
injected outcome epilogue `must_derive_from_judge(&out.payload, &judge_prov);`
({#g15-outcome-provenance}), so the *target* rung's payload MUST implement
`::rung::Provenanced` as well as the source's. A branching judgmental
transition gets the prologue and no epilogue.

"#.into(),
            mechanism: r#"The emitted forward judgmental transition is followed by the injected outcome epilogue ([G15](rung-props.md#g15-outcome-provenance)), so its *target* payload must implement `::rung::Provenanced` as well as its source. The cited test is the epilogue firing: a body that returns its own argument does not complete. Removing the injected call from the macro reddens it; a branching judgmental transition gets no epilogue, which is why `review::step` still compiles with a `Report` payload that carries no provenance."#.into(),
        }),
        Element::Prop(Prop {
            slug: "resume-signature".into(),
            parent: Some("emitted-functions".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge".into() },
            numbering: None,
            prose: r#"**`resume`:** `pub fn revive(s: Suspended<Posed>, evidence:
::rung::Terminated, pen: ::rung::Authorized<'_, R>) -> Posed`. The second and
third parameter names come from the body's second and third closure inputs when
there are any; otherwise they are `_evidence` and `_pen` and are consumed
unread. Two prologues are injected — `must_hold_standing_over(&s.token.payload,
&pen)` and `must_answer_the_raised(&s.raised, &evidence)` — so the source rung's
payload MUST implement `::rung::Situated`. **No `must_progress`**
({#g16-the-residual-channel}).

"#.into(),
            mechanism: r#"Three parameters and two injected prologues. The cited test coerces the emitted resume `fn` to its exact pointer type, so the pen cannot quietly leave the signature; its siblings pin the prologues — a pen minted over another container is refused although the body never mentions it, and evidence about another raised matter resumes nothing. Deleting the injected `must_hold_standing_over` reddens `::resume_refuses_a_pen_over_another_container`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "authorial-signature".into(),
            parent: Some("emitted-functions".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen".into() },
            numbering: None,
            prose: r#"**`#[authorial(R)]`:** `pub fn revised(filed: Filed, pen:
::rung::Authorized<'_, R>) -> Revised` — a second parameter, taken by value. Its
name comes from the body's *second* closure input when there is one; otherwise
it is bound to `_pen` and consumed unread. The body is preceded by the injected
standing prologue `must_hold_standing_over(&filed.payload, &pen);`
({#g14-the-authorial-gate}), so the source rung's payload MUST implement
`::rung::Situated`.

"#.into(),
            mechanism: r#"The authorial mirror, coerced the same way to `fn(revision::Filed, Authorized<'_, Curator>) -> revision::Revised`, with the standing prologue pinned by `gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "body-name-resolution".into(),
            parent: Some("emitted-module".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"Inside body expressions, rung/verdict names resolve unqualified;
payload types resolve from the surrounding scope (`use super::*`).

"#.into(),
            mechanism: r#"The cited bodies name `Active`, `StepOutcome`, `Converged` and `Carry` unqualified, and `LoopState`/`Report` from the surrounding scope through the emitted `use super::*`. Dropping either half leaves an unresolved name at expansion."#.into(),
        }),
        Element::Verbatim(r#"---

## 4 · Guarantees

"#.into()),
        Element::Prop(Prop {
            slug: "guarantees".into(),
            parent: None,
            kind: Kind::Decidable { proof: "(rustc)".into() },
            numbering: Some('G'),
            prose: r#"Each guarantee is normative and **names the conformance test that fails
if the implementation stops honoring it**. Guarantees delegated to the Rust
compiler are marked *(rustc)*.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g1-linear-consumption".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "(rustc)".into() },
            numbering: None,
            prose: r#"**Linear consumption.** A transition consumes its input rung by value;
using a rung after it is moved MUST be a compile error. *(rustc — move
semantics.)*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g2-sealed-construction".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624".into() },
            numbering: None,
            prose: r#"**Sealed construction.** A rung MUST NOT be constructible by struct
literal outside its module (`_seal` is private). When an `impl` block is
present, only the **entry** rung's `new` is public; every other rung's `new` is
module-private, so no code outside the module can mint a mid-ladder rung.
Categorically this is not merely a fabrication guard: it enforces that a *verb
cannot occupy object-position* — a state is reached only by traversing an arrow,
never fabricated to hold an arrow's result (see
{#the-law}). That enforcement reaches exactly
as far as this guarantee does: a type-only declaration publishes every
constructor, and is freely generated by convention only (see
{#freeness-enforced-only-with-bodies}).
*Conformance: `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624`
— a `trybuild` case whose committed `.stderr` pins **E0624, associated function
`new` is private**, as the sole error. The `rung/src/lib.rs` compile_fail
doctest shows the same refusal in rustdoc but does not assert the code
({#compile-fail-asserts-only-non-compilation}).*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g3-one-token-one-thread".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync".into() },
            numbering: None,
            prose: r#"**One token, one thread.** Every rung and verdict MUST be `!Send +
!Sync` (via `PhantomData<*const ()>`), so a shared `Arc`/`&` cannot carry it
across a thread boundary. *Conformance:
`compile_pass.rs::test_rungs_are_not_send_or_sync` (rungs and verdicts).*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g4-no-silent-drop".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error".into() },
            numbering: None,
            prose: r#"**No silent drop.** Every rung, verdict, `StepOutcome`, and `Failed` MUST
be `#[must_use]`. Dropping a token in statement position is a warning — an error
under `#![deny(unused_must_use)]`. *Conformance:
`spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error` — a
`trybuild` case whose committed `.stderr` pins the denied `unused_must_use` lint
and the macro's contract-specific note. The `rung/src/lib.rs` must_use
compile_fail doctest documents it but does not assert the diagnostic
({#compile-fail-asserts-only-non-compilation}). (Escapable by `mem::forget`
/ `let _ =` — see {#drop-proofing-beyond-the-lint}.)*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g5-carry-immutability".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_carry_accessor_exists".into() },
            numbering: None,
            prose: r#"**Carry immutability.** `Carry` MUST be a private field exposed only
through `&Carry`; a transition body cannot mutate it. *Conformance:
`compile_pass.rs::test_carry_accessor_exists`.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g6-exhaustive-outcomes".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "(rustc)".into() },
            numbering: None,
            prose: r#"**Exhaustive outcomes.** `StepOutcome` is an enum; every match site MUST
handle all variants. *(rustc — enum exhaustiveness.)*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g7-recover-pairing".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::a_recoverable_verdict_without_a_recover_edge_is_refused".into() },
            numbering: None,
            prose: r#"**Recover pairing.** Every recoverable verdict has a matching recover
function and vice versa; terminal verdicts have none
({#macro-must-reject}, rules 4–7). *(macro — static checks.)*

"#.into(),
            mechanism: r#"Rules 4–7, one `trybuild` case each. The cited one is the first direction (a recoverable verdict with no edge); `::a_recover_edges_target_must_be_a_declared_rung`, `::a_terminal_verdict_may_not_carry_a_recover_edge` and `::a_recover_edge_must_name_a_declared_verdict` are the rest. This guarantee said *(macro — static checks.)* and named no test, so it was the one guarantee of the fourteen with nothing behind it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "g8-recovery-progress".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recover_guard_is_auto_injected".into() },
            numbering: None,
            prose: r#"**Recovery progress.** The macro MUST wrap every *verdict* recover body
with `must_progress`, comparing the source rung's payload to the produced rung's
payload and panicking if equal. The body cannot skip it. Requires the shared
payload type be `Clone + PartialEq`. *Conformance:
`end_to_end.rs::recover_guard_is_auto_injected` (panics with no explicit call).*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g9-error-path-recovery".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path".into() },
            numbering: None,
            prose: r#"**Error-path recovery.** A `recover { .. : Failed(R) => R }` function
receives the `Failed` and returns the next rung. No progress guard is injected
(a retry may reuse the token). *Conformance:
`end_to_end.rs::recovers_from_the_failed_error_path`.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g10-continue-arms".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn".into() },
            numbering: None,
            prose: r#"**Continue arms.** A `V -> R` arm makes `step` build the next rung
inline; `StepOutcome::V` carries that rung directly — no recover function, no
guard, no source. *Conformance:
`end_to_end.rs::continue_arm_loops_without_a_recover_fn`.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g11-terminal-payloads".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"**Terminal payloads.** A `V(P)` terminal verdict returns a value through
the verdict, read via `.payload()` / `.into_payload()`. *Conformance:
`end_to_end.rs::drives_to_convergence` asserts the returned payload.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g12-gate-marked-signature".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token".into() },
            numbering: None,
            prose: r#"**Gate-marked signature.** A `#[judgmental(R)]` transition MUST take a
second parameter of type `::rung::Qualified<R>`, by value; an unmarked
transition MUST emit byte-for-byte what it emitted before markers existed. Two
gates are therefore two *signatures*, separated by the host's type system rather
than by a convention someone keeps
({#two-signatures-not-two-fragments}).
`Qualified` has no public constructor — `Pool::qualify` is the only mint — so a
judgmental transition cannot be called without an outside, and a decidable one
has no parameter an outside could enter through
({#decidable-cannot-consult-pool}).
**This makes the signature honest; {#g13-token-binding} is what binds the
token to an argument** — see {#gate-faithfulness-not-secured} for what
neither secures. *Conformance:
`gate_markers.rs::judgmental_transition_takes_a_qualified_token` (the emitted
`fn` is coerced to a `fn` pointer of the exact expected type), and the
`tests/ui/` `trybuild` cases: `gate_missing_token` → E0061, `gate_forged_token`
→ E0451.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g13-token-binding".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads".into() },
            numbering: None,
            prose: r#"**Token binding.** The macro MUST prefix every `#[judgmental(R)]`
transition body with `must_be_bound_to(&<source>.payload, &<token>)`, which
panics unless the token's recorded `π(a)` equals the source rung payload's. The
body cannot skip it, exactly as it cannot skip {#g8-recovery-progress}'s
`must_progress`, and for the same reason: the body is the domain's, so a
guarantee the body could omit is not a guarantee. This requires the source
rung's payload to implement `::rung::Provenanced` — without `π(a)` there is
nothing to measure.

A `Qualified<R>` records the argument it was measured against alongside the
principal, and `Qualified::admit` is the one gate that spends it. The seal
({#g12-gate-marked-signature}) closes *fabrication* — nobody can write a
token. G13 closes *transfer* — nobody can spend an honestly-earned token on an
argument it was never measured against, which is the act
{#disjointness-against-argument}
forbids and the pair
{#non-identity-by-construction}
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

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g14-the-authorial-gate".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen".into() },
            numbering: None,
            prose: r#"**The authorial gate.** An `#[authorial(R)]` transition MUST take a
second parameter of type `::rung::Authorized<'_, R>`, by value, and the macro
MUST prefix its body with `must_hold_standing_over(&<source>.payload, &<pen>)`,
which panics unless the pen's container equals the source rung payload's. This
requires the source rung's payload to implement `::rung::Situated` — without a
container there is nothing standing could be held over. Unmarked and
`#[judgmental(R)]` emission is unchanged.

**G14 is not {#g12-gate-marked-signature}+{#g13-token-binding} with a
different token name, and an implementation that made it one would satisfy every
clause above while enforcing nothing.** The two gates run over one pool and
select opposite predicates
({#one-pool-two-filters}):

| | judgmental | authorial |
|---|---|---|
| qualifying set | {#judgmental-qualifying-set} | {#authorial-qualifying-set} |
| second conjunct | `π(p) ∩ π(a) = ∅` — **disjointness** | `standing(p, M)` — **standing** |
| reading | you did **not** author this | this is **yours to revise** |
| admissibility | `π(f(a)) ∩ π(a) = ∅` | `π(f(a)) ⊆ π(p) ∧ standing(p, a)` |

Provenance overlap is what disqualifies a judge and what an author needs
({#provenance-overlap-is-the-point}),
so a principal that passes one filter has, on that evidence, said nothing about
the other and typically fails it
({#judgment-refuses-authorship-requires}).
`Pool::authorize` MUST therefore check **both** conjuncts of
{#authorial-qualifying-set}:
standing alone mints no pen. It refuses on the judgmental branch of
{#standing-conditional-gated}
rather than guessing — closing that branch needs a judge, terminating at depth
one
({#standing-terminates-at-depth-one}),
whose own qualification is non-identity relative to the **author**
({#standing-judge-disjoint-from-author}).

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

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g15-outcome-provenance".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render".into() },
            numbering: None,
            prose: r#"**Outcome provenance.** The macro MUST follow every `#[judgmental(R)]`
**forward** transition body with `must_derive_from_judge(&<out>.payload,
&<π(p) snapshot>)`, which panics unless the returned payload's provenance is
contained in the qualifying principal's. This requires the *target* rung's
payload to implement `::rung::Provenanced` — without $\pi(f(a))$ there is
nothing to measure. The snapshot is taken in the prologue, because the body
consumes the licence; and the body runs inside an immediately-invoked closure,
so a `return` in it cannot step over the check. Unmarked, `#[authorial(R)]` and
*branching* judgmental emission is unchanged.

{#g13-token-binding} constrains the arrow's **argument**; this constrains
its **outcome**, and they are the two halves of
{#admissibility-subcategories}. Without G15 a body may
hold an honest licence, bound to the very argument it is applied to, and hand
that argument straight back out — the constant arrow
{#constant-arrow-hazard} names, expressed as a ladder.

**It asserts containment, not disjointness, and that is the point.** With G13
having just re-established $\pi(p) \cap \pi(a) = \emptyset$ for this argument,
$\pi(f(a)) \subseteq \pi(p)$ entails
$\pi(f(a)) \cap \pi(a) = \emptyset$
({#judgment-provenance-is-the-judges}). A disjointness
epilogue on top would assert the conclusion of a derivation whose premises are
both enforced. Containment is also the half a lying body cannot satisfy by
stamping: `::rung::Judgment` has no constructor outside `rung`, so a payload
whose $\pi$ derives from one carries a provenance its producer did not choose.

**Forward transitions only.** A branching judgmental transition returns a sum
whose recoverable and continue arms carry the argument onward by design —
re-entry rather than laundering
({#reproposal-carries-the-chain}) — so which arms are
*outcomes* in the sense of {#admissibility-subcategories}
is unsettled, and the epilogue does not guess. Recorded as an open limit in
`docs/questions/open/q11-gate-faithfulness.md`.

*Conformance:
`gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render`
(a judgmental body that returns its own argument; deleting the injected call
reddens it) and
`::a_judgmental_arrow_may_not_return_the_provenance_it_judged` (the arrow whose
outcome is built on the judge's `Judgment`; minting that `Judgment` with the
argument's provenance instead of the judge's reddens it).*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "g16-the-residual-channel".into(),
            parent: Some("guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed".into() },
            numbering: None,
            prose: r#"**The residual channel, and the arrow back.** A `#[judgmental(R)]`
**forward** transition MUST return `Result<Next, Suspended<Prev>>`, so that a
dispatch which cannot be settled now can hand the argument back **unconsumed**
together with the opaque reference to what was raised. A `resume` edge MUST take
an `::rung::Authorized<'_, R>` pen and `::rung::Terminated` evidence, MUST be
prefixed with `must_hold_standing_over` and `must_answer_the_raised`, and MUST
NOT be wrapped in `must_progress`. Unmarked, `#[authorial(R)]` and *branching*
judgmental emission is unchanged.

**This adds no summand.** The residual is the final `+ A` Het's judgmental
arrow already carries
({#judgmental-arrow-shape}), and a
judge that exists and has not answered is adequacy **undischarged**, which
{#adequacy-failure-returns-residual} already returns as
that residual. What G16 supplies is the *channel*: before it, a forward
judgmental transition returned its target rung and had nowhere to put the
argument, so a theory whose principal could not answer yet had no term for
saying so — the suspension existed in the formalism and not in the language.

**The pen is forced, not chosen.** Resuming produces a rung of this ladder, and
{#g2-sealed-construction} seals that construction against everything outside
the module. The edge must therefore be emitted *inside* the module — and an
edge inside the seal that any caller may invoke is the seal with a door in it.
So resumption dispatches through the authorial filter
({#resumption-is-authorial}): capability and standing
over the container the subject sits in, the same shape as `enact`. The judge
that ruled on the raised matter cannot be the principal that resumes — it
qualified by provenance-disjointness, which is what denies it standing
({#provenance-overlap-is-the-point}).

**The absent guard is the point.** {#g8-recovery-progress} exists because a
recover edge that returns its own source is an infinite stall. A resume edge
that returns its own source is the *normal case*: the argument was never
consumed, the raised matter took another round, and nothing about the subject
should have changed. A progress guard here would refuse the shape rather than a
bug, and would be the bound Het declines to declare
({#guarded-reentry-is-eviction}).

**What it does not promise.** Termination. A raised matter that never terminates
yields no `Terminated`, and the arrow stays suspended
({#resumption-needs-a-terminal}). Nor does it survive
process death — see {#suspension-is-in-process-only}.

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

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 5 · Non-guarantees

"#.into()),
        Element::Prop(Prop {
            slug: "non-guarantees".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Explicitly out of scope. The macro does **not** enforce the following, and
a claim that it does has no standing.

"#.into(),
            mechanism: r#"The heading of the withdrawals. A non-guarantee states that the macro does **not** enforce something and that a claim it does has no standing; there is no obligation left for a host to discharge. Its children point at the boundary tests where a boundary exists."#.into(),
        }),
        Element::Prop(Prop {
            slug: "transition-body-correctness".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::a_transition_body_may_be_wrong_and_the_macro_does_not_care".into() },
            numbering: None,
            prose: r#"**Transition-body correctness.** The type proves a transition *ran*, not
that its logic was valid — the boundary between typestate and formal
verification.

"#.into(),
            mechanism: r#"The typestate/verification boundary. The type proves a transition ran; nothing here claims its logic was valid, so there is nothing to check. Every `expressible` row in this ledger inherits this limit."#.into(),
        }),
        Element::Prop(Prop {
            slug: "cross-crate-provenance".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung-fixture/tests/consumer.rs::a_consumer_cannot_tell_a_real_order_from_an_invented_one".into() },
            numbering: None,
            prose: r#"**Cross-crate provenance.** A rung crossing a crate boundary is trusted,
like any Rust public API. Sealing this needs a sub-crate per ladder.

"#.into(),
            mechanism: r#"A rung crossing a crate boundary is trusted, like any Rust public API. Closing it needs a sub-crate per ladder, which is a packaging decision rather than a macro guarantee."#.into(),
        }),
        Element::Prop(Prop {
            slug: "same-module-fabrication".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::the_entry_constructor_and_the_module_itself_may_fabricate".into() },
            numbering: None,
            prose: r#"**Same-module / entry fabrication.** {#g2-sealed-construction} stops
*external* fabrication; code inside the generated module, and the public entry
constructor, can still build rungs — the module-boundary limit Rust always has.

"#.into(),
            mechanism: r#"The module-boundary limit Rust always has. The cited test pins where the seal *does* bite — external construction is E0624 — so the withdrawal is readable as a boundary rather than as an absence."#.into(),
        }),
        Element::Prop(Prop {
            slug: "drop-proofing-beyond-the-lint".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::a_must_use_token_can_still_be_discarded_three_ways".into() },
            numbering: None,
            prose: r#"**Drop-proofing beyond the lint.** {#g4-no-silent-drop} is
`#[must_use]`, which `mem::forget`, `let _ = token;`, or burying the token in a
dropped container all bypass. True no-drop needs language-level linear types.

"#.into(),
            mechanism: r#"`mem::forget`, `let _ = token`, and a dropped container all bypass `#[must_use]`; true no-drop needs language-level linear types. The cited test pins the lint's actual reach, which is what is being bounded."#.into(),
        }),
        Element::Prop(Prop {
            slug: "liveness-beyond-the-guard".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::the_progress_guard_is_satisfied_by_motion_that_goes_nowhere".into() },
            numbering: None,
            prose: r#"**Liveness beyond the guard.** {#g8-recovery-progress} catches an
identical-token stall loop; it does not prove general forward progress.

"#.into(),
            mechanism: r#"[G8](rung-props.md#g8-recovery-progress) catches an identical-token stall; general forward progress is a halting question. The cited test exercises the guard on exactly the case it does catch, so what is being withdrawn is legible as the complement of something real."#.into(),
        }),
        Element::Prop(Prop {
            slug: "suspension-is-in-process-only".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::a_suspension_may_be_held_across_arbitrary_intervening_work".into() },
            numbering: None,
            prose: r#"**Suspension does not survive process death.**
{#g16-the-residual-channel} suspends and resumes **in one process**: a
driver may hold a `Suspended<Prev>` in memory for as long as it likes, and that
is the whole of the claim. Writing one to disk and reconstituting it later is
not supported and is not merely unimplemented — a rung read back from bytes is a
mid-ladder rung nobody traversed to, which is exactly what
{#g2-sealed-construction} exists to refuse. Resumption being authorial
answers *who may* revive a run; it says nothing about *how a token survives
serialization*. Filed as
[Q13](questions/open/q13-suspension-across-process-death.md), and related to
{#cross-crate-provenance}.

"#.into(),
            mechanism: r#"A driver may hold a `Suspended<Prev>` in memory for as long as it likes and that is the whole of the claim. Persisting one across process death is not merely unimplemented — a rung read back from bytes is a mid-ladder rung nobody traversed to, which is what [G2](rung-props.md#g2-sealed-construction) exists to refuse, and {#resumption-is-authorial} answers WHO MAY revive a run without saying WHAT a reconstituted token is. Filed rather than guessed at."#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-faithfulness-not-secured".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**Gate-faithfulness.** {#g12-gate-marked-signature} secures the
judgmental signature, {#g13-token-binding} its argument,
{#g14-the-authorial-gate} both halves of the authorial gate's *input*, and
{#g15-outcome-provenance} the judgmental *outcome* of a forward
transition. What is still not secured is the outcome everywhere else, and one
of Het's four gates still has no signature.

"#.into(),
            mechanism: r#"Narrowed, not closed. rung now checks the way **out** as well as the way in: G15 is the judgmental outcome epilogue and `settle` is its sentence-surface twin, so {#returned-value-unconstrained} no longer describes the whole outward side. What keeps this non-guarantee standing is {#one-gate-unimplemented} — one of Het's four gates has a refusal rather than an encoding — and the residue at [5.621](rung-props.md#outward-conditions-remaining). The cited test is the first of those, made runnable."#.into(),
        }),
        Element::Prop(Prop {
            slug: "one-gate-unimplemented".into(),
            parent: Some("gate-faithfulness-not-secured".into()),
            kind: Kind::Owed { why: "unimplemented — #[conditional(..)] is a parse-time refusal, not an encoding".into() },
            numbering: None,
            prose: r#"*One gate is unimplemented.* `#[conditional(..)]` is a parse-time
refusal, not an encoding. Gate-faithfulness is a condition on **every** operation
of an algebra, so an algebra with a conditional arrow cannot state it here at
all.

"#.into(),
            mechanism: r#"The refusal itself is enforced, which is the honest reading: `#[conditional(..)]` is a parse-time `compile_error!` naming the open question, pinned by a `trybuild` snapshot. What is *not* secured is gate-faithfulness for an algebra that has a conditional operation — that algebra cannot be written here at all, and the refusal is what says so."#.into(),
        }),
        Element::Prop(Prop {
            slug: "returned-value-unconstrained".into(),
            parent: Some("gate-faithfulness-not-secured".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render".into() },
            numbering: None,
            prose: r#"*The returned value is constrained judgmentally, and only there.* This
non-guarantee used to read "the returned value is unconstrained," and it was
exact: `Prov::contained_in` existed and no guarantee called it. Two now do.
`theory!`'s `settle` takes a sealed `Judgment` rather than a `Verdict` and
refuses `π(f(a)) ⊄ π(p)`; {#g15-outcome-provenance} injects the same check
as an epilogue on a forward judgmental transition. Disjointness —
{#admissibility-subcategories}'s judgmental clause — is
not checked because it is entailed
({#judgment-provenance-is-the-judges}).

The residue is stated at {#outward-conditions-remaining} rather than
absorbed into a claim that the outward side is closed. It is not.

"#.into(),
            mechanism: r#"The proposition used to read "the returned value is unconstrained", and the measure of it was that `Prov::contained_in` existed and no guarantee called it. Two guarantees call it now. G15 injects `must_derive_from_judge` after every forward `#[judgmental(R)]` body, and `theory!`'s `settle` refuses a `Judgment` whose provenance the licence does not contain. The cited test is the constant arrow as a ladder — a judgmental body that returns the argument it was handed — and deleting the injected call from the macro reddens it. The companion `::a_judgmental_arrow_may_not_return_the_provenance_it_judged` is the positive case, and minting the token's `Judgment` with the argument's provenance instead of the judge's reddens that one. What is NOT covered is stated at [5.621](rung-props.md#outward-conditions-remaining) and parked on its own test rather than folded in here."#.into(),
        }),
        Element::Prop(Prop {
            slug: "outward-conditions-remaining".into(),
            parent: Some("returned-value-unconstrained".into()),
            kind: Kind::Owed { why: "unimplemented — the authorial containment conjunct is left to the body".into() },
            numbering: None,
            prose: r#"*Two outward conditions remain.* First, the **authorial** one:
{#admissibility-subcategories} states the authorial
clause as `π(f(a)) ⊆ π(p) ∧ standing(p, a)`, and
{#g14-the-authorial-gate} secures the standing conjunct on the way in
while leaving the containment conjunct on the way out entirely to the body —
the same shape as {#g13-token-binding}'s gap, on the second gate. Second,
**branching** judgmental transitions take the prologue and no epilogue, because
a branching outcome is a sum whose recoverable and continue arms carry the
argument onward by design
({#reproposal-carries-the-chain}), and which of those
arms is an *outcome* in {#admissibility-subcategories}'s
sense is not settled. Both inherit {#transition-body-correctness} whole,
as the whole outward side used to.

"#.into(),
            mechanism: r#"The two halves of the outward side that G15 does not reach. The authorial conjunct π(f(a)) ⊆ π(p) from {#admissibility-subcategories} is left to the body exactly as the judgmental one was before R2 — G14 secures `standing` on the way in and nothing looks on the way out — and the cited test is that arrow: an honest pen over the right container, and a revision carrying someone else's provenance. A branching judgmental transition takes the prologue and no epilogue, because its recoverable and continue arms carry the argument onward by design ({#reproposal-carries-the-chain}) and which arms are *outcomes* is unsettled; that is a question rather than a hole, and it is recorded in the same `#[ignore]` reason."#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidable-is-not-pure".into(),
            parent: Some("gate-faithfulness-not-secured".into()),
            kind: Kind::Decidable { proof: "rung/tests/non_guarantees.rs::an_unmarked_transition_may_touch_the_world".into() },
            numbering: None,
            prose: r#"*Decidable is not pure.* The unmarked signature excludes Het's outside
— the principal pool — and is silent about clocks, files, and networks
({#purity-not-secured}).

"#.into(),
            mechanism: r#"rung has no effect system. The unmarked signature excludes Het's outside — there is no parameter a principal could enter through — and says nothing about clocks, files, or sockets. Het states the same limit independently ({#purity-not-secured})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "type-only-marker-is-inert".into(),
            parent: Some("gate-faithfulness-not-secured".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert".into() },
            numbering: None,
            prose: r#"*A type-only declaration emits no transitions,* so a marker on one has
nothing to constrain and is inert, exactly as {#g2-sealed-construction}'s
seal is
({#freeness-enforced-only-with-bodies}).

"#.into(),
            mechanism: r#"A declaration with no `impl` block emits no transition functions, so a marker on one has no signature to change. The cited test states that as something the compiler checks: the marked role type does **not** implement `Role`, and the declaration compiles anyway — which it could not if the marker were emitting a `Qualified<R>` parameter or a prologue."#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-faithfulness-answered-no".into(),
            parent: Some("gate-faithfulness-not-secured".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Whether {#g12-gate-marked-signature} +
{#g13-token-binding} + {#g14-the-authorial-gate} +
{#g15-outcome-provenance} amount to gate-faithfulness is argued — and
answered *no* — in [Q11](questions/open/q11-gate-faithfulness.md), which stays
open on {#one-gate-unimplemented} and, in its narrowed form,
{#outward-conditions-remaining}.

"#.into(),
            mechanism: r#"A claim about an argument, not about the host: it records that Q11 is open and answered *no*. The two things it stays open on are {#one-gate-unimplemented}, which is `enforced` as a refusal, and {#returned-value-unconstrained}, which is `parked`. Both carry their own row; this one carries the reasoning."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-cycle-through-an-authorial-act-cannot-close".into(),
            parent: Some("non-guarantees".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**A cycle that must pass through an authorial act cannot close inside
one `ladder!`.** {#declaration-grammar} declares a **linear spine** with
backward continue arms. A continue arm's target rung is built *inline by
`step`* ({#g10-continue-arms}), so every arm of the branching transition is
authored by whoever holds that transition's token. Where `step` is
`#[judgmental(R)]`, an arm returning to the ladder's **entry** rung would
therefore have the judge produce the revised subject — the amendment
{#no-amending-disposition} forbids.

The audit–rectify pass is the case. Het states that `enact` is what makes the
pass an **endofunctor** rather than a one-way funnel into a verdict
({#enact-makes-an-endofunctor}); `rung-het`'s
`het_pass!` therefore stops one arrow short, with `Accept` terminal and
carrying a licence, and `enact` a **separate** authorial arrow consuming that
licence and a pen. The loop closes by **composition** — feeding `enact`'s
result into a fresh run — and not within the declaration.

So `ladder!` does not express the endofunctor, and a claim that a single
declaration is one has no standing here. Whether a rung's payload may be a
completed sub-ladder run — which is the shape that would let the composite be
declared rather than driven — is
[Q4](questions/open/q4-composition-nested-ladders.md), open.

"#.into(),
            mechanism: r#"A limit on the DECLARATION, recorded rather than worked around. `ladder!` declares a linear spine with backward continue arms, and a continue arm's target rung is built inline by `step` ([G10](rung-props.md#g10-continue-arms)) — by whoever holds that transition's token. An `Accept -> Governed` arm on the pass would therefore have the JUDGE produce the revised subject, which {#no-amending-disposition} forbids. So `enact` sits outside the branching transition and the loop of {#enact-makes-an-endofunctor} closes by composition, not inside one declaration. The cited test is the shape as built: `Accept` is terminal and carries a `Licence`, and the run leaves the ladder to enact. Expressing the composite as a declaration is Q4 (`docs/questions/open/q4-composition-nested-ladders.md`), open — nothing here is claimed to close it."#.into(),
        }),
        Element::Verbatim(r#"---

## 6 · Conformance

"#.into()),
        Element::Prop(Prop {
            slug: "conformance-suite".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung-doctrine/tests/roundtrip.rs::the_conformance_record_is_rendered_from_the_doctrine".into() },
            numbering: None,
            prose: r#"The conformance suite is `rung/tests/` and the doctests in
`rung/src/lib.rs`. A change that violates any guarantee above MUST break at
least the cited test. The README's Getting Started example is itself a run
doctest (via `include_str!`), so the documented public API cannot silently drift
from the macro.

"#.into(),
            mechanism: r#""A change that violates any guarantee MUST break at least the cited test" is only a claim if the citation is live. `./_ledger.py check` regenerates every row from the propositions documents and fails when a cited file is missing or a cited `fn` has been renamed away, so a guarantee cannot quietly lose its test."#.into(),
        }),
        Element::Prop(Prop {
            slug: "compile-fail-asserts-only-non-compilation".into(),
            parent: Some("conformance-suite".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**A `compile_fail` doctest does not verify the error code.** rustdoc
ignores the `E0NNN` in a fence such as `compile_fail,E0999`, and E0999 does not
exist — the block passes. So a `compile_fail` doctest asserts exactly one thing:
*this did not compile*. It cannot tell the refusal it was written for from a
typo, an unresolved import, a missing `main` (E0601), or a name that fell out of
scope when rustdoc wrapped the snippet in a `fn main` of its own. Adding the code
annotation does not fix this; nothing reads it.

"#.into(),
            mechanism: r#"A fact about rustdoc — it ignores the `E0NNN` in a `compile_fail` fence — rather than an obligation on the host. What follows *from* it is {#no-guarantee-cites-a-compile-fail-doctest}, and that is enforced."#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-guarantee-cites-a-compile-fail-doctest".into(),
            parent: Some("conformance-suite".into()),
            kind: Kind::Decidable { proof: "rung-doctrine/tests/roundtrip.rs::every_decidable_proposition_names_a_proof_that_resolves".into() },
            numbering: None,
            prose: r#"Consequently **no guarantee may cite a `compile_fail` doctest as its
conformance test.** Refusals are pinned by `trybuild` cases in `rung/tests/ui/`,
which diff the full rendered stderr against a committed `.stderr` snapshot, so
the code and the message are both part of the assertion. The doctests are kept
alongside — they are the documentation, and a reader meets the refusal in
rustdoc — but they are the illustration, not the evidence.

"#.into(),
            mechanism: r#"`./_ledger.py check` refuses any conformance citation that points into a crate's `src/`, which is the only place a doctest can live. A row that tried to rest on a `compile_fail` fence is a ledger failure rather than a reviewer's catch. Refusals are pinned by `trybuild` cases in `rung/tests/ui/`, whose committed `.stderr` makes the message part of the assertion."#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-silent-doctest-traps".into(),
            parent: Some("conformance-suite".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Two further traps, both found in this repo and both silent:

- A doctest with no `fn main` is wrapped in one by rustdoc, so `struct` and
  `macro_rules!` items land in a function body. A `ladder!`-generated `mod` then
  cannot see them and the example fails on E0425 — a green test asserting
  nothing about the guarantee. Write an explicit `fn main` when the snippet
  declares items.
- A struct literal that omits a field fails with E0063 whether or not the fields
  are private, so a "cannot be forged" example with a stale field list keeps
  passing after the seal is removed. Name every field.

"#.into(),
            mechanism: r#"Two ways to write a doctest that passes while asserting nothing. Guidance for authors of examples; the guarantees do not rest on doctests at all ({#no-guarantee-cites-a-compile-fail-doctest})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-refusal-test-that-cannot-fail".into(),
            parent: Some("conformance-suite".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**A refusal test that cannot fail is not a guarantee.** The way to
establish that a case can fail is to make the guarded thing legal and watch the
case go red.

"#.into(),
            mechanism: r#"The mutation discipline itself — make the guarded thing legal and watch the case go red. It is a rule about how evidence is produced, and no machine performs it; a checker that could would be the guarantee."#.into(),
        }),
        Element::Verbatim(r#"---

## 7 · Design judgments

"#.into()),
        Element::Prop(Prop {
            slug: "design-judgments".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: Some('J'),
            prose: r#"The propositions above are settled by the macro, by rustc, or by a named
test. The judgments below are not: **no machine decides them.** They govern how
rung is *used* — where a ladder should stop, and what earns a place in
`rung-std`. They are earned through use rather than derived from first
principles; amend them when a new case does not fit, but amend them
deliberately, as a ruling on the record.

"#.into(),
            mechanism: r#"The document says of this subtree that **no machine decides them** and that they carry no conformance test. They bind design decisions — where a ladder stops, what earns a place in `rung-std` — and are amended as rulings on the record rather than checked."#.into(),
        }),
        Element::Prop(Prop {
            slug: "j1-where-the-tower-bottoms-out".into(),
            parent: Some("design-judgments".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**Where does the tower bottom out?** A rung ladder should terminate where
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

"#.into(),
            mechanism: r#"A judgment about leverage: extend the tower while structural enforcement still buys correctness gains. Nothing in a run can answer it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "j2-what-belongs-in-rung-std".into(),
            parent: Some("design-judgments".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**What belongs in rung-std?** A ladder belongs in `rung-std` when it
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
"#.into(),
            mechanism: r#"A judgment about recurrence and canonicity. A test could count dependents; it could not decide whether the canonical statement is better than a project's own derivation."#.into(),
        }),
        ],
    }
}
