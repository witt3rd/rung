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
> realized in the Python reference interpreter (`python-poc/rung/interpreter.py`)
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

## 6. The dagger category — recovery edges

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

Full linear logic requires **exactly once** use — you cannot drop a token silently. If Rust had linear types, the compiler would refuse to compile code that drops an `Active` without consuming it through a transition or returning it in a `Failed`, closing the "silent drop" gap at the language level. The macro's `#[must_use]` (SPEC.md G4) is the affine approximation — but it is escapable (`mem::forget`, `let _ = token;`), so it is a *partial* close. A truly linear substrate would make it exact; see [`questions.md`](questions.md) Q3.

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

## 10. Summary — why hide the category theory?

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