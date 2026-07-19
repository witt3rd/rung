**CLAIM:** The failure path in a rung transition — a morphism \(f : A \to \Sigma_i B_i + \mathsf{Failed}\langle A \rangle\) that returns the input token packaged in the error summand — is captured by a Kleisli arrow for the `Result` (or exception) monad \(T X = X + E\).

**REQUIRES:**  
A Kleisli arrow for a standard exception/Result monad has the shape \(A \to T(B)\) where failure yields a fixed error object \(E\) (independent of \(A\)) and sequencing uses the monad’s `bind`/`join`. Recovery or retry is external handling, not a built-in reverse morphism that consumes \(\mathsf{Failed}\langle A \rangle\) and produces \(A\) again. The monad laws (associativity of bind, unit laws) must govern composition uniformly.

**VERIFICATION:**
- In the actual structure (RUNG-CT.md §6), failure explicitly returns \(\mathsf{Failed}\langle A \rangle\) *carrying the original linear token \(A\)* so that a declared recover arrow can map it back: \(f^\dagger : \mathsf{Failed}\langle A \rangle \to A\).
- This is **not** a fixed \(E\); the error object depends on the input resource. Standard `Result<T, E>` does not force \(E\) to carry \(A\); doing so would make the monad non-standard and break the usual Kleisli composition (bind would have to thread the carried token specially).
- RUNG-CT.md §6 explicitly classifies this as the **error dagger** on the coproduct injection for the failure branch:  
  \[
  f : A \to \mathsf{StepOutcome} + \mathsf{Failed}\langle A \rangle, \qquad f^\dagger : \mathsf{Failed}\langle A \rangle \to A.
  \]
  It is **optional** (a transition may omit the recover) and **unguarded** (no progress requirement, unlike verdict daggers). These properties do not arise from any monad bind.
- The dagger laws (\(f^{\dagger\dagger} = f\), contravariance) are deliberately **not verified** for error daggers. Monad laws are orthogonal and would not produce an optional, resource-preserving reverse arrow.
- The `Result` return type in Rust is merely the surface syntax for the coproduct + error summand; it does not induce a Kleisli category whose arrows are the rung transitions.

**VERDICT:** Does **not** hold. The structure is the **dagger** of RUNG-CT.md §6 (partial, resource-preserving recovery on the failure injection), not a Kleisli arrow for any standard effect monad \(T\). The “return the input” design is exactly what enables the dagger and is load-bearing for optional recovery.

---

**CLAIM:** Error recovery and async (`Future`) effects are two instances of the *same* Kleisli story, so once the framing is adopted async is “nearly free” (the answer to Q7 is half-written in the recover edges).

**REQUIRES:**  
Both must be uniformly expressible as Kleisli arrows \(A \to T(B)\) for (possibly different) monads \(T\) on the *same* base, with composition governed by the same Kleisli laws, and distributive laws (Beck) allowing stacking (e.g. `Future` ∘ custom-Result). The recover edges must supply the monad structure for the error case.

**VERIFICATION:**
- Error case: as above — dagger, not Kleisli. The recover edges supply an explicit reverse morphism \(f^\dagger\), not a monadic `bind` or `join`. The optional/unguarded/well-founded properties are dagger-specific.
- Async case: an `async` body yielding `Future<StepOutcome + Failed<A>>` *can* be read as a Kleisli arrow for the `Future` monad (or a graded variant). `Future` is strong; distributive laws with ordinary `Result` exist in many settings. But this is a *separate* monad layer; it does not inherit from or unify with the error dagger.
- The recover edges therefore do **not** supply the Kleisli story for error; they supply the dagger. The unification claim fails.
- From Fritz (Markov categories): stochastic/generative bodies (LLM calls) *do* fit Kleisli of an affine commutative probability monad (Giry etc.), yielding a Markov category with copy/delete. Rung’s affine symmetric monoidal base (linear payloads + unrestricted carry) is compatible in principle if the monad has tensorial strength (Moggi). This is orthogonal to the error dagger.
- From Capucci–Hedges (categorical cybernetics, parametrised optics): the forward transition + dagger recovery on the failure branch has a bidirectional flavour. The failure summand carries a “residual” (the original token) and the dagger supplies the backward pass — reminiscent of forward/backward pairs in optics (residual memory for feedback/rollback). However, the document deliberately uses the simpler dagger presentation rather than full optic machinery (coends, reparametrisations). It is optic-*like*, not a plain forward Kleisli arrow.

**VERDICT:** Does **not** hold. Error is dagger (RUNG-CT §6); async can be Kleisli for `Future` but they are distinct gadgets. The recover edges do not make error Kleisli, so the “same story” unification is false. The cheapest falsifier (does `Failed<A>`-returns-input fit Kleisli or dagger?) resolves negatively for the handoff claim.

---

**CLAIM (rider):** The whole transition (linear consumption of \(A\), coproduct of successor rungs on success, failure branch carrying \(A\) for recovery) is best modelled as a (dependent) optic/lens in the Capucci–Hedges sense rather than a plain Kleisli arrow.

**REQUIRES:**  
An optic supplies a coupled forward map (producing output + residual) and backward map (using residual for feedback/put). The failure-carrying-\(A\) + dagger must correspond to the residual + backward pass; composition must match optic laws (not just Kleisli bind).

**VERIFICATION:**
- Forward: the step morphism produces either a new rung payload or a failure token that *is* the original \(A\) (residual).
- Backward: the declared dagger \(f^\dagger\) on the failure summand exactly “puts back” the token — the optic-like backward pass.
- This matches the bidirectional/residual-memory pattern in parametrised optics (Capucci–Hedges), where residuals enable contingent feedback or rollback. The coproduct branching on success is compatible with dependent optics (fibre-wise).
- However, rung’s explicit design uses the **dagger** presentation (simple reverse arrow on one summand, optional, no full involution). Full optic machinery (coends over residuals, reparametrisation 2-cells) is heavier than needed for the current recover-pairing check (G7/G9). The dagger is the load-bearing implementation; the optic reading is a useful *retrospective* lens on why the structure works bidirectionally.
- No contradiction with Fritz/Markov for the *generative* case: a probabilistic body can still be a stochastic kernel (Markov morphism) whose success/failure branches are then handled by the dagger on the failure injection.

**VERDICT:** Holds *with modification*. The transition + recover pair has genuine optic-like bidirectional structure (forward produces residual on failure; dagger is the put-back). But the *operative* model in RUNG-CT is the **dagger** on the coproduct injection (partial, optional, resource-preserving). Optics provide a deeper unifying view for the cybernetic/feedback reading; they do not replace the dagger for implementation or the free-category presentation of the ladder.

---

**Summary of the decision tree (the refined precise question)**

1. **Is \(T\) a strong commutative monad on rung’s affine symmetric monoidal base?**  
   For `Future`/`Prob`: yes in the literature (Moggi strength + Fritz affine commutative monads → Markov categories). The base (linear tokens + carry + coproducts + daggers) requires care; strength ensures the Kleisli category remains monoidal/affine-compatible. This is now *secondary* — the dagger vs monad gate is already resolved.

2. **Do the effects admit distributive laws?**  
   Yes for common stacks (`Future` ∘ ordinary `Result`). Because the rung “Result” is actually a daggered coproduct + \(\mathsf{Failed}\langle A \rangle\), any composition must be layered *on top of* the dagger structure. Standard Beck distributive laws still apply to the effect layer.

3. **Is failure-returns-input captured by that monad, or by the dagger of §6, or by an optic?**  
   **By the dagger of §6** (primary, as written). Secondarily optic-*like* (bidirectional residual on failure). Not by a standard Kleisli monad. This is the cheapest, load-bearing falsifier.

**Implementation consequence (rung):**  
The current design is correct and precise. Keep error/recover as explicit partial dagger (optional, unguarded, progress only on verdicts). Add async as a Kleisli layer for `Future` (or graded) where needed — it does *not* unify with error via recover. For generative/LLM bodies, model the transition body itself as a stochastic kernel in a Markov category (Fritz). The optic reading explains *why* the dagger works bidirectionally but does not change the compiler’s free-category + dagger enforcement.

The refined framing cleanly separates what was bundled. The handoff conjecture (“error is Kleisli via recover edges”) is resolved in the negative by the dagger classification already present in RUNG-CT.md §6. Q7 is now actionable: async is a standard Kleisli extension; probability is Markov/Kleisli; error/recover stays dagger; optics supply the bidirectional intuition. The structure remains load-bearing.
