# Het gates in rung — what this is, and what it is not

**Branch:** `feat/het-gates` · **Crate:** `rung-het` · **Status:** experiment.
Nothing here is settled doctrine. It exists to find out what breaks.

---

## The claim being tested

Het (`witt3rd/heteronomy`, here as `docs/rung-het-propositions.md`) extends
institution theory at exactly one point: the satisfaction relation `M ⊨ φ`. Sentences carry a **gate
marker** — `decidable` sentences are machine-checked, `judgmental` sentences
dispatch to a principal drawn from a pool, and the principal's verdict *is* the
outcome.

The claim this crate tests:

> Gate-marked satisfaction is enforceable by the type system, and enforcing it
> there makes **P0** — nothing self-certifies — a compile error rather than a
> code path someone must remember to call.

## Why it belongs in rung

[The law](rung-ct-propositions.md#the-law) already states what rung enforces: *a
verb can only live on
a morphism, never inside an object* — enforced by sealed constructors (G2). That
law was **found from the inside**: an attempt to fold an LLM verdict into a
ladder tried to construct the next state to hold the verdict, and the sealed
constructor refused.

Het's gate law is the same move on a second axis:

> **An outside call can only live on a judgmental arrow.**

Same mechanism — seal the capability and hand it only to the arrow licensed to
hold it. `Qualified<R>` is to the outside what `_seal` is to the rung.

## What was built

| piece | what it does |
|---|---|
| `Prov` | Het's `π` — a finite tag set, with `overlaps` (judgmental) and `contained_in` (authorial) |
| `Role` | a competence, as a **type**. This is what supplies `role(φ)`. |
| `Principal` | the pool interface: `capable(p, Role)` and `π(p)`, and nothing more (nothing-further-required) |
| `Qualified<R>` | **the sealed capability.** No public constructor; `Pool::qualify_for` is the only mint. Records the principal **and** `π(a)`, the argument disjointness was measured against (non-identity-by-construction) |
| `Qualified::admit` | the one gate that spends a token: refuses it for any argument but the one it was measured against, with `TokenNotBound` |
| `Pool::qualify_for` | dispatch-is-two-operations's qualifying set for a given argument — competence filter, then non-identity — returning *any* survivor (no-preference-among-judges: Het must not rank). `Pool::qualify` is its `audit` reading, where `π(a) = π(M)` |
| `theory!` | the surface: declare a sort and its gate-marked sentences |
| `Judgmental` | the trait carrying `role(φ)`; unimplementable without naming a role |

### The enforcement, concretely

A **decidable** sentence emits `fn holds(model: &M) -> Settled`. Arity one.
There is no parameter through which a pool, a principal, or a token could
arrive — so the body cannot consult an outside. Not "should not." *Cannot.*

A **judgmental** sentence emits
`fn settle(model: &M, q: Qualified<R>, verdict: Verdict)
-> Result<Settled, TokenNotBound>`, consuming the token **by value**. Without a
`Qualified`, there is no term. The only way to get one runs `π(p) ∩ π(a) = ∅`.

And the token is **bound**: `settle` admits it only for the model it was
measured against. Sealing the constructor closes *fabrication* — nobody can
write a token. It does not close *transfer*: before the binding, a licence
earned honestly against one argument could be spent on another, which is the act
disjointness-against-argument forbids. `TokenNotBound` is that refusal, and it
is a value the caller cannot drop in silence. `rung_het::dispose` does the same
against the **proposal**, which is where the two readings come apart
(argument-governs).

That is Het gate-faithful (gate-faithfulness) by construction: an algebra cannot launder a
judgmental operation into a decidable one, because the two have different types.
It is not *all* of gate-faithfulness — see Q11 for what remains.

## Verification

`cargo test -p rung-het` — 16 integration tests, 1 doctest, **6 `compile_fail`
doctests**. The negative cases are the ones that matter: a gate that never fires
on a deliberate violation is not a gate (rung SPEC.md fractal-property).

Each `compile_fail` was additionally checked **by compiling it standalone** and
reading the actual error, because a `compile_fail` that passes for the wrong
reason is exactly the vacuity this work exists to catch:

| case | fails with |
|---|---|
| decidable body handed a pool | `E0061` — arity 1, cannot take an outside |
| judgmental settled with no token | `E0061` — `Qualified` is not optional |
| `Qualified` fabricated by struct literal | `E0451` — private fields |
| one licence reused for two sentences | `E0382` — moved value |
| judgmental with no role | custom `compile_error!` citing judgmental-declares-role |
| judgmental with a body | custom `compile_error!` citing the gate law |

Two runtime tests deserve naming:

- **`p0_refuses_a_judge_who_authored_the_material`** — the failure Het exists to
  forbid, refused with the shared tags named.
- **`p0_is_not_vacuous_when_the_model_claims_no_author`** — `qualify` *refuses*
  a model with empty provenance rather than admitting everyone. If `π` returns
  empty, disjointness holds trivially and P0 becomes decorative. That is the
  precise shape in which a real implementation can pass its own tests while
  enforcing nothing.

## What this closes

**`role(φ)`.** Het judgmental-declares-role requires every judgmental sentence to declare the
competence role needed to discharge it, and capable-single-arity pins `capable` to arity
`𝒫 × Role`. In a prose encoding that map can simply be *absent* — it was, and
nothing could notice, until the interface was written down and the gap became
visible. Here a judgmental sentence that names no role does not parse.

This is also one of two blockers an outside reviewer independently named as
standing between Het and publication (`docs/heteronomy/publish_gaps.md`
models-defined-by-dispatch).

## What is deliberately missing

| missing | why |
|---|---|
| **`authorial` gate** | standing, not disjointness — `π(outcome) ⊆ π(p)`. `Prov::contained_in` exists so the asymmetry is visible, but no gate consumes it yet. |
| **`conditional` gate** | classified *one level up*, per model (conditional-names-classifier). rung's checks are static; this one is not known at declaration time. **The first place the encoding will tell us something Het has not decided.** |
| **verdict metric `d` and `ε`** | verdict-space-with-metric–epsilon-declared-not-ranked. The verdict here is Boolean, so the satisfaction condition does not survive renaming. Named, not papered. |
| **the pass** | `audit → propose → dispose → enact` as a ladder. Needs the authorial gate first. |
| **populations** | data, not code. Reading `SOUL.md` into objects is a separate concern. |

## The open question this raises

`conditional` is the interesting one. Het says decidability of a conditional
sentence depends on the specific algebra, classified by a sentence in the theory
one level up. rung's checks are all static. So either:

- the ladder resolves the gate at **declaration** time — which flattens exactly
  what makes it conditional; or
- the ladder is **generic over a classifier** the caller supplies — which pushes
  the decision to instantiation and may be right.

I do not know which. That is the point of encoding it: the compile error will
say something Het's prose currently does not.

## Design note — data and code

The split this rests on:

> **Data = populations. Code = theories.**

`SOUL.md` is data — a carrier with objects in it. *What makes a valid SOUL item*
is a theory, and a theory is code. Under fractal closure (fractal-property) some carrier
objects are themselves theories, and those cross the boundary at exactly that
point; rung's models-defined-by-dispatch composite opfibration is where that composition already lives.

## Prior state

`het-rs` is the existing implementation. An audit on 2026-08-03 found its
qualification machinery real, well-tested, and **called by nothing on the
dispatch path**, with the authorial gate a hardcoded success. That is the
failure mode this design forecloses structurally: here, a qualification filter
that nothing calls would mean no judgmental sentence could be settled at all,
and the program would not build.
