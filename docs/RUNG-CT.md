# RUNG-CT — The category rung declares

**2026-07-16 · Donald Thompson & Forge ⚒️** · *stance revised 2026-07-18*

rung is a **category-declaration language.** A ladder declares the objects and the legal arrows of a category, and the type system enforces that you may travel only declared arrows. This is not an analogy the way "it's *like* a state machine" is an analogy — it is what the primitive **is**, and the compiler enforces the category's axioms directly.

## The law

- **States are objects.** A state is inert — data at rest, a point. It has no verbs.
- **Transitions are arrows (morphisms).** Every *doing* lives on an arrow.
- **Therefore a verb can only live on a morphism, never inside an object.** Compute, judge, call an LLM, hit the network — each is a verb, and each belongs in a transition *body*, never in the construction of a *state*.

This was not read from theory and implemented — it was found from the inside. An attempt to fold a live LLM verdict into a ladder tried to *construct the next state to hold the verdict*, and the sealed constructor (SPEC.md **G2**) refused: `Evaluated::new` cannot be called from outside the arrow. That refusal **is** the free-category axiom being enforced — a morphism was being asked for in object-position, and no such thing exists. The fix was forced and correct: the verb moved into the transition body, the only place a verb can be. The sealed constructor is not merely a fabrication guard; **it is the enforcement of what a category *is*.**

The rest of this document traces the correspondence in detail — coproducts (verdict branching), products (carry), the indexed monad, the dagger (recovery), linear logic, Curry-Howard. The syntax hides the mathematics behind the programmer's mental model; the compiler does not.

---

## 1. The free category — what the `ladder` declaration really declares

A `ladder` declaration:

```
ladder Work {
    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Active(ActiveWork)
}
```

is a **presentation of a free category** W:

- **Objects**: the rungs — `Designed`, `Claimed`, `Active`. Each object is a state in the transition graph.
- **Generating morphisms**: the `=>` arrows — `design: Designed → Claimed`, `claim: Claimed → Active`. Each arrow is a declared transition.
- **No other morphisms exist.** The sealed constructor (SPEC.md G2) enforces that the category is *freely generated* by the declared arrows — this is "the law" above, read locally. There is no morphism `Designed → Active` except the composition `claim ∘ design`. The compiler refuses any attempt to construct a morphism not in the free category (equivalently: to put a verb in object-position).

A well-typed program is a **functor**

```
F: W → RustTypes
```

mapping each rung to its Rust struct, and each transition to its Rust function. The compiler enforces that `F` respects composition: `F(claim ∘ design)` = `F(claim) ∘ F(design)` — meaning `claim(design(spec))` is the only way to get from `Spec` to `Claimed`.

### Composition law

```
design: Designed → Claimed
claim:  Claimed   → Active
───────────────────────────────  (composition)
claim ∘ design: Designed → Active
```

The composition is **linear**: `design` consumes the `Designed` token. After composition, the intermediate `Claimed` is consumed — it exists only as a proof artifact in the provenance trace. This is the categorical semantics of move semantics: composition is not just sequencing; it is **resource consumption**.

---

## 2. Coproducts — verdict branching

The verdict branching on the final rung:

```
Active => {
    Converged                   // terminal
    | Stalled => Active         // recoverable
    | BudgetExhausted           // terminal
}
```

is a **coproduct** (sum type, discriminated union):

```
StepOutcome ≅ Converged + Stalled + BudgetExhausted
```

The coproduct has **injections** (construct each variant from its payload) and a **universal property** (pattern matching). The Rust `match` statement is the universal morphism from the coproduct:

```
         inj_Converged                   inj_Stalled                   inj_BudgetExhausted
Converged ─────────────→ StepOutcome ←───────────── Stalled ←─────────────────────────── BudgetExhausted
    │                          │                       │                                        │
    │                          │         match         │                                        │
    │                          └───────────┬───────────┘                                        │
    │                                      │                                                    │
    └──────────────────────────────────────┼────────────────────────────────────────────────────┘
                                           │
                                           ▼
                                         Active
```

**Exhaustiveness** is the coproduct's universal property: every morphism out of the coproduct must be defined on every injection. A missing case is a violation of the universal property — the compiler refuses. This is why adding a verdict variant breaks every `match` site at compile time. The coproduct's universal property is the compile-time gate.

---

## 3. Products — the carry data

The `carry` block:

```
carry {
    task_id: String,
    correlation_key: Uuid,
}
```

declares a **product** `Carry ≅ String × Uuid` that is preserved across the entire category. Every rung is paired with the carry:

```
Designed  ≅  DesignedPayload × Carry
Claimed   ≅  ClaimedPayload  × Carry
Active    ≅  ActivePayload   × Carry
```

The projection `π₂: Rung → Carry` extracts the carry data from any rung. The linear consumption applies only to the first component; the carry is **structurally shared** (immutable, duplicated by reference or trivial copy). This is a **cartesian product in a linear category** — the second factor is unrestricted, the first is linear.

### The graded comonad reading

The carry can be read as a **graded comonad**. Consider:

- The grade is the rung name (position in the ladder).
- The comonadic context is the `Carry` — it provides ambient witness data.
- A transition `Designed → Claimed` is a Kleisli arrow for the comonad: `DesignedPayload × Carry → ClaimedPayload × Carry`.

The comonad laws encode that the carry is preserved:
- **Counit**: extracting the carry from a rung gives back the same carry (projection π₂).
- **Coassociativity**: the carry is the same at every rung (it doesn't change as you move through the ladder).

---

## 4. The indexed monad — the type of the ladder itself

The entire ladder is an **indexed monad** (also called a graded monad or parameterised monad). The index is the pair of rung names `(i, j)`. A value of type `M i j A` represents a computation that:

- Starts at rung `i`
- Ends at rung `j`
- Carries a payload of type `A`

The operations:

```
unit_i    : A → M i i A                          — stay at the same rung (identity)
bind      : M i j A × (A → M j k B) → M i k B   — sequential composition
```

The indexed monad enforces that composition only succeeds when the indices align. You cannot compose `design: M Designed Claimed DesignedPayload` with `step: M Active Converged ()` — the inner index `Claimed` does not match the outer input `Active`. The ladder declaration is the type-level graph; the monad's index discipline is the compiler's refusal to misalign rungs.

### The explicit indexed monad type

For the MetricOptimization ladder:

```
M: Rung × Rung × Type → Type

M Spec   Spec   SpecPayload                           — identity (start at Spec, stay at Spec)
M Spec   Active ActivePayload                         — design then activate (composed)
M Active Active ActivePayload                         — step (continue)
M Active ()     ConvergedPayload                      — step (converge)
M Active ()     StalledPayload                        — step (stall, recoverable)
M Active ()     BudgetExhaustedPayload                — step (budget exhausted)
M Active Active ActivePayload  (via recover_stalled)  — stalled → recover → active
```

Each valid path through the ladder corresponds to a well-typed `M i j A` value. Invalid paths (e.g. `M Spec Active ConvergedPayload` — skipping `claim`) are not in the type — they are unrepresentable.

### The monad laws

```
bind (unit_i a) f = f a                             — left identity
bind m unit_j = m                                   — right identity
bind (bind m f) g = bind m (λa → bind (f a) g)     — associativity
```

These hold trivially because the only valid compositions are those declared in the ladder. The free category's composition is the indexed monad's `bind`. The sealed constructors prevent any composition not generated by the declared arrows.

---

## 5. The writer monad — the provenance trace

> **Scope.** This section describes the *structure* of provenance, not the current
> Rust macro. The `ladder!` macro does **not** emit a trace; provenance tracing is
> realized in the Python reference interpreter (`.archive/python-poc/rung/interpreter.py`)
> and can be layered on by the caller. The writer-monad correspondence holds
> wherever a trace is accumulated.

Every transition appends to the provenance trace. This is a **writer monad**:

```
Writer Trace A ≅ A × Trace
```

where `Trace ≅ [TraceEntry]` is a monoid under concatenation (∅, ++). The writer operations:

```
tell  : TraceEntry → Writer Trace ()
return: A → Writer Trace A
bind  : Writer Trace A × (A → Writer Trace B) → Writer Trace B
```

Each transition `f: Token → Result<NewToken, Failed>` is lifted into the writer:

```
f_writer(token) = let result = f(token)
                  in tell(entry) >> return result
```

The trace is the **free monoid** on `TraceEntry`. The provenance trace is the writer monad's accumulated log. The `carry` is the unrestricted component that rides alongside the writer's output.

### Composing the writer with the indexed monad

The full type is a **graded writer monad**:

```
MW i j A ≅ M i j (A × Trace)
```

where `M i j` is the indexed monad and `Trace` is the writer's output. The grade `(i, j)` tracks rung position; the writer tracks provenance. The monad laws hold because the indexed monad laws hold and the writer is a monad transformer.

---

## 6. The dagger — recovery as a Prism's backward pass

> **Frame (Q7, resolved 2026-07-18).** The dagger below is the *operative* presentation — what the compiler enforces. Two independent expert reviews established the deeper structure it sits inside: **a rung transition is a Prism** (a dependent optic), and the recover edges are that Prism's *backward pass*. The dagger is not replaced — it is situated. See §6.1 and `questions/resolved/q7-effectful-bodies-which-monad.md`.

The `recover` block declares **reverse morphisms** — adjoints, not necessarily inverses — for verdicts and failures:

```
recover {
    stalled: Stalled       => Active    // verdict dagger
    cleared: Failed(Active) => Active    // error dagger
}
```

A category is **dagger** (†-category) if every morphism has a declared adjoint:

```
f:   A → B       (forward transition)
f†:  B → A       (declared recovery, IF it exists)
```

The ladder's dagger is **partial** — adjoints exist only where declared — and it comes in **two distinct flavors**, which the implementation keeps separate.

### Verdict daggers — required, guarded, *not* involutive

A recoverable verdict `Stalled => Active` is a dagger on a *coproduct injection*: `step` lands in the `Stalled` summand and `stalled: Stalled → Active` sends it back. These daggers are **mandatory** — every recoverable verdict MUST have a matching recover function (the recover-pairing check, SPEC.md G7). Terminal verdicts (`Converged`, `BudgetExhausted`) have no dagger; they are absorbing states.

But the verdict dagger is **not an involution.** A clean dagger obeys `f†† = f` and permits `f† ∘ f` to return to the same value; the ladder deliberately forbids the latter. The macro auto-injects a progress guard (SPEC.md G8) requiring the recovered `Active` to *differ* from the one that stalled — the round-trip step→stall→recover MUST decrease, or it panics (`f† ∘ f ≠ id`). This trades the dagger's **symmetry** for **well-foundedness**: recovery is a decreasing step, not a reversal, which is exactly what makes a stall loop terminate. The verdict dagger is a *well-founded recovery*, not a categorical inverse.

### Error daggers — optional, unguarded

A branching transition returns a **coproduct** `StepOutcome + Failed<A>`; the `Failed<A>` summand carries the unconsumed token. The error dagger operates on that branch:

```
f:    A → StepOutcome + Failed<A>    (forward, may fail)
f†:   Failed<A> → A                  (recovery from the error path)
```

An error dagger `Failed(A) => A` (SPEC.md G9) is the recover function for the failure branch. Unlike verdict daggers it is **optional** — a transition may return `Err(Failed { .. })` with no declared recovery, leaving the caller to handle it — and it is **unguarded**, because a retry after a transient error may legitimately reuse the same token (no progress required). The compiler does **not** require an error dagger for every `Failed` variant; error recovery is opt-in.

The involution `f†† = f` and contravariant `(g ∘ f)† = f† ∘ g†` laws are not verified (§9) — and for verdict daggers the involution is *intentionally violated* by the progress guard.

### 6.1 The frame the dagger sits in: transitions are Prisms

The dagger above is what the compiler enforces (recover-pairing, G7/G9). The "which monad?" question (Q7) asked whether a fallible transition body was instead a Kleisli arrow for an effect monad — and if so, whether async recovery was "half-written in the recover edges." Two independent expert reviews resolved it: **it is not a monad; a transition is a Prism** (a dependent optic — Capucci–Hedges).

A Prism from a whole `A` to a part `Σᵢ Bᵢ` is a forward/backward pair:

```
Match (forward):  A → Σᵢ Bᵢ + A     — exactly the transition signature; the residual is the input token
Build (backward): the successors route back via the §6 dagger,  Bᵢ → A
```

So **the recover edges *are* the Prism's Build pass**, and the failure-returns-input design — which reads as an oddity — is exactly the optic's *residual*. This is also why it is not a monad: composing `f: A → B + A` with `g: B → C + B`, a failing `g` hands back `B`, but the composite's domain is `A`; no monadic `bind` can route `B → A` — only an explicit backward edge can. (That is the falsifier the handoff proposed, triggering cleanly.)

Two things the frame makes precise:

- **G8 is the contraction.** A Prism's Build pass need not be involutive. The progress guard (G8) makes the verdict backward pass *contractive* (`f† ∘ f ≠ id`, strictly decreasing) rather than a symmetric involution — which is *why* `f†† = f` was deliberately broken above. Well-foundedness is the optic's contraction, not a defect.
- **Effects layer on the forward pass, not on recovery.** An effectful transition is a strong monad `M` on the Match pass: `A → M(Σᵢ Bᵢ + A)`. Async is `M = Future` (a strong commutative monad; tensorial strength `A ⊗ T(B) → T(A ⊗ B)` is exactly what proves linearity survives `.await`); a generative body is a **Markov kernel** — a probability/Giry monad in Fritz's sense, affine. Error recovery and effects are therefore *orthogonal gadgets*: error is the optic's backward pass; effects are monads on its forward pass. The Kleisli "unification" is **false** — async is a free-standing feature (tracked as Q8), needing no distributive law with the error structure.

Full resolution and both reviews: `questions/resolved/q7-effectful-bodies-which-monad.md` and its `_evidence/`.

> **One level up (Q9).** This Prism is Level 0 — an optic *within* a ladder. The same gadget reappears one level up: the *dependency* structure between whole items is a **dependent optic** too, and the coproduct that keeps *this* section honest is what keeps *that* level functorial. See §10.

---

## 7. Linear logic — Rust's affine type system as substrate

Rust's type system implements **affine logic** (linear logic without the requirement to use exactly once). The key correspondence:

| Linear logic | Rust |
|---|---|
| Resource `A` | Value of type `A` |
| `A ⊸ B` (linear implication) | `fn(A) -> B` (takes ownership, returns ownership) |
| `!A` (exponential, unrestricted) | `&A` (shared reference) or `A: Copy` |
| `A ⊗ B` (tensor product) | `(A, B)` struct |
| `A ⊕ B` (additive disjunction) | `enum { A(A), B(B) }` |

The ladder's linear consumption is the affine `⊸`:

```
activate: Designed ⊸ Active     — consumes Designed, produces Active
step:     Active  ⊸ Active ⊕ Converged ⊕ Stalled ⊕ BudgetExhausted
```

The borrow checker verifies that every resource is used at most once. The "at most once" is affine, not linear — you CAN drop a token (Rust drops it at end of scope), but you cannot use it twice. The `ladder!` macro adds `#[must_use]` to every rung and verdict type (SPEC.md G4), so a dropped token is a warning — an error under `#![deny(unused_must_use)]`.

### What linear logic would add

Full linear logic requires **exactly once** use — you cannot drop a token silently. If Rust had linear types, the compiler would refuse to compile code that drops an `Active` without consuming it through a transition or returning it in a `Failed`, closing the "silent drop" gap at the language level. The macro's `#[must_use]` (SPEC.md G4) is the affine approximation — but it is escapable (`mem::forget`, `let _ = token;`), so it is a *partial* close. A truly linear substrate would make it exact; see [Q3](questions/blocked/q3-true-no-drop-linearity.md).

---

## 8. The Curry-Howard correspondence for ladders

The Curry-Howard correspondence maps types to propositions and programs to proofs. For the ladder:

| Type | Proposition |
|---|---|
| `Designed` | "Design has been completed" |
| `Claimed` | "The resource has been claimed" |
| `Active` | "The work is active" |
| `Converged` | "The optimization has converged" |
| `Failed<Active>` | "The step failed, AND the Active token is preserved" |

A program of type `Spec → Converged` is a **proof** that there exists a valid sequence of transitions from `Spec` to `Converged`. The compiler verifies the proof. The provenance trace is the **proof term** — the explicit sequence of inference steps.

The indexed monad `M i j A` is the type of **proofs that from state `i` you can reach state `j` with result `A`**. The ladder declaration generates the proof rules; the transition implementations are the proof construction.

---

## 9. What the compiler actually verifies (and what it doesn't)

The compiler (rung checker + Rust borrow checker) verifies:

| Property | Mechanism |
|---|---|
| The category is freely generated by declared arrows | Sealed constructors |
| Composition is linear (intermediate states consumed) | Rust move semantics |
| Coproduct elimination is exhaustive | Rust `match` exhaustiveness |
| Carry is a product factor | Private struct field + `&Carry` projection (immutability enforced, SPEC.md G5) |
| Verdict daggers exist (partial) | Recover-pairing check (§6) |

The compiler does NOT verify:

| Property | Why |
|---|---|
| Functoriality of the program | Transition body correctness — what the function does, not just its type |
| Dagger laws (f†† = f) | No language support for equational reasoning — and verdict daggers *intentionally* break involution (§6, the progress guard) |
| Monad laws (associativity) | Holds by construction of the free category — no separate proof needed |
| Carry coassociativity | Carried data is explicitly threaded — no separate proof needed |
| Provenance (writer monad) | The Rust macro emits no trace — §5 is structural only |
| Recovery progress (well-foundedness) | Runtime guard (G8), not a compile-time check — panics on no progress |

---

## 10. The Level-1 superstructure — the dependency opfibration

> **Frame (Q9, resolved 2026-07-18).** §§1–9 describe *one ladder* — a category, its coproducts, its Prism transitions. But ladders (and questions, claims, decisions) do not sit in isolation; they *depend on each other*. That dependency structure is a genuine **Level 1** object — arrows between whole items, not within one — and the growth tower (`questions/_map.md`) predicted a superstructure would live here. Two independent expert reviews named it precisely. See `questions/resolved/q9-the-dependency-superstructure.md` and its `_evidence/`.

A dependency is an **arrow between items** — `X → Y` reads "Y depends on X." Collect the items as objects and the typed dependency edges as generating morphisms, and you have the **free category `B`** on the dependency graph. The superstructure over it is a **Grothendieck opfibration** `p : E → B`:

- **The fibre `E_X`** over an item `X` is **exactly its Level-0 ladder** — the local category of §1. The tower is self-similar: each object of the Level-1 base *contains* a Level-0 category.
- **The total space `E`** is items-in-specific-states.
- **A typed edge is an opcartesian lift.** When `X` changes state (a morphism in the fibre `E_X`), that change transports *forward* along `f : X → Y` via a **pushforward functor** `f_! : E_X → E_Y`. Forward transport along the direction of information flow is **opcartesian**, not cartesian — the orientation is load-bearing, and it is what distinguishes the opfibration from a plain fibration.

Each edge type is a distinct pushforward:

| edge type | pushforward `f_!` maps a change at X to… |
|---|---|
| `premise` | a **strict** lift → `Y`'s `MustReexamine` rung (obligatory) |
| `justification` | a **coproduct** lift → `ReviewRequired(Y) + Survives(Y)` (advisory) |
| `spawn` | `Y` exists only as `X`'s child — revisit |
| `citation` | a mechanical state-update, no human in the loop |

### Advisory edges do not break functoriality — the Q7 coproduct recovers it

The sharp falsifier: a `premise` edge maps `X → Y` deterministically, but a `justification` edge relies on a human judging "no change needed." A rigid state-assignment functor `B → Set` cannot map `X`'s recovery into a state `Y` is *already* in without violating the free-category laws — so advisory propagation appears to break strict functoriality.

**It does not, and the reason is §2 and §6.1.** An advisory pushforward does not land in `Y`'s base category of rungs — it lands in the **Kleisli category** of `Y`, delivering the **coproduct** `ReviewRequired(Y) + Survives(Y)`. The Level-1 superstructure rigidly and functorially *delivers that coproduct*; it is then `Y`'s own Level-0 machinery — the effectful transition body of §6.1, the progress guard of §6 — that **collapses the uncertainty.** And because every fibre is a **free category** (§1), `Y` retains agency to run a *vertical* morphism (a manual `Review → Resolved`) entirely inside its own fibre; the opcartesian lifts evaluate against `Y`'s *current* state, preserving functoriality over the total space.

So the coproduct that keeps §6 honest at Level 0 is *exactly* the structure that keeps the dependency level functorial at Level 1. The two levels are load-bearing on each other — the self-similarity is not analogy but shared machinery.

### Edges are dependent optics — the tower, self-similar

If a Level-0 transition is a **Prism** (§6.1 — an optic for *sum* types: forward Match + backward Build), a Level-1 dependency is a **Lens** (an optic for *product/context*: bidirectional transport across a boundary). And because the type of the backward pass — the blast radius — depends on the exact state transported forward, it is a **dependent optic**:

```
Forward (play):    covariant   — pushforward of a state-change to dependents (f_!)
Backward (coplay): contravariant — query exposure before you build; each Y answers
                                   from its CURRENT state, returning a typed cost
```

**Blast radius is the backward pass, not a file count.** Before modifying `X`, you send a query backward along the composite optic; the answer is an *exposure vector* — *"3 mechanical updates (cheap), 2 obligatory coproduct reviews (expensive)"* — the support of the composite dependent optic. This is what `questions/_reach.py` computes the *deflationary boolean shadow* of today: it walks reachability and prints a review checklist. The model it approximates is transport of typed obligations through an opfibration of optics; the store (frontmatter now, a graph store eventually) is inconsequential, but the model is named. See `EDGES.md` for the operative registry consequence.

### The tower iterates — the registry is fractal (Q10, resolved 2026-07-19)

§10 so far describes the opfibration over *one* base — the item graph of a single registry. **Q10** asked whether that structure iterates *up a domain hierarchy* (`relational-being ⊐ {memory, actions, …}`, each domain carrying its own registry), and two independent CT reviews converged: it does, exactly, with no new machinery. See `questions/resolved/q10-fractal-registry-hierarchy.md` and its `_evidence/`.

Let `q : B → B′` map items to their parent domains (its fibres are the local registries) and `p : E → B` be the opfibration above. **Opfibrations are closed under composition** (Bénabou; Jacobs, *Categorical Logic and Type Theory*, Lemma 1.1.4), so the composite

```
E  ─p→  B  ─q→  B′          (state ⊐ item ⊐ sub-domain ⊐ domain)
```

is a **single composite Grothendieck opfibration**. "An opfibration whose fibres are opfibrations" collapses into one unified opcartesian structure. Three consequences, all confirmed by review:

- **It is an iteration of Level 1, not Level 2.** Opfibrations are 1-cells in **Cat** and compose by ordinary functor composition; stacking domains stacks 1-categorical bricks. The reserved Level-2 slot (`_map.md` — natural transformations) stays **vacant** — filling it needs a genuine 2-cell *between* fibrations (a schema migration of the registry, a topology remap), which mere nesting does not introduce.
- **Obligation-transport is scale-invariant.** Optics compose (Capucci–Hedges; Spivak's *Poly*): the composite forward pass is `g_! ∘ f_!`, the backward pass is successive query-and-cost up the composite lens. So `_reach.py` (and its typed-exposure successor) crosses domain boundaries with **zero modification** — it need not know whether an edge is sibling-to-sibling or child-domain-to-parent; the same functorial backward pass runs at every scale.
- **Horizontal ≅ vertical.** Under the Grothendieck construction (`∫`), the indexed hierarchy of dependency graphs flattens into a single total graph: a "horizontal" edge (sibling composition) and a "vertical" edge (domain implication) are both generating morphisms in the composite base, lifting identically. Each fibre of a vertical opfibration *is* a horizontal opfibration of the layer below — **self-similar by construction.** The formal content of "the registry pattern is fractal" is precisely this scale-invariance.

The categorical claim is a *theorem* (composition of the structures §10 already names) and resolves on the reviews. **Building** an actual domain-registry hierarchy is a separate matter, gated by the third-instance rule (`_map.md`) on lived need, not on the proof.

---

## 11. Summary — why hide the category theory?

The ladder syntax deliberately hides the categorical machinery. The programmer writes:

```
ladder Work {
    Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveWork)
}
```

not:

```
Let W be the free category on the graph
  G = ({Designed, Claimed, Active}, {design: Designed→Claimed, claim: Claimed→Active})
with indexed monad M: W^op × W → Endo(RustTypes) and dagger † defined on {claim}.
```

The category theory is the **implementation** of the compiler, not the **surface** of the language. This is the same move that Rust makes with linear logic: Rust programmers don't write linear logic proofs — they write `let x = y`, and the borrow checker enforces the linearity. The ladder programmer thinks in rungs and transitions; the compiler thinks in categories, coproducts, and indexed monads. The abstraction holds.

---

*"The category theory is the compiler's internal representation. The ladder syntax is the surface. The programmer thinks in rungs; the compiler thinks in indexed monads. That's the gap."* — Forge ⚒️, 2026-07-16