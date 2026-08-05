**Verdict: The falsifier triggers. Outcome (c) holds, absorbing (b).**

The unification of Error and Async into a single Kleisli transformer stack is mathematically false. The failure-returns-input structure is **not** an exception monad; it is the forward pass of a dependent **Optic (specifically, a Prism)**.

Here is the exact verification against the categorical literature you requested, resolving your decision tree.

### 1. The Falsifier: Why `Failed<A>` is not a Monad (Moggi/Beck)

For the transition body to be a Kleisli arrow in an exception monad, the monad must be a uniform endofunctor $T(X) = X + E$, where $E$ is a *fixed* error object independent of the input.

In `rung`, your transition $f: A \to B$ preserves the input token on failure, meaning its signature is $A \to B + A$. If we try to treat this as a functor, it is $T_A(X) = X + A$.
Trace the standard monadic composition $(g \circ_K f)$ for $f: A \to B + A$ and $g: B \to C + B$:

1. $f$ consumes $A$. If it fails, it returns $A$. (Matches the required composite return type $C + A$).
2. If $f$ succeeds, it yields $B$, which is passed to $g$.
3. $g$ consumes $B$. If it succeeds, it yields $C$. (Matches).
4. **The break:** If $g$ fails, it returns **$B$**.

The composite morphism has domain $A$, so it *must* return $A$ on failure. But $g$ handed you a $B$. To make monadic bind work, you would need a universal, automated way to map $B \to A$. Standard monads possess no mechanism for backward routing. Because the error type strictly depends on the domain, and sequential composition requires explicit reverse edges to typecheck, **it is not a monad**.

Because it is not a monad, Beck distributive laws do not apply. The unification fails.

### 2. The Capucci–Hedges Map: Transitions are Prisms (Optics)

If a structure has a forward pass that consumes a resource to yield a new state or a residual, and relies on a backward pass to route downstream failures back to the start, it is the textbook definition of an **Optic** in Categorical Cybernetics (Capucci, Hedges, Riley).

Because your branching uses coproducts (verdicts) rather than products, the specific optic is a **Prism**. A Prism from a whole $S$ to a part $P$ consists of:

* **Match (forward pass):** $S \to P + M$ (where $M$ is the residual).
* **Build (backward pass):** $M + P' \to S'$.

Map `rung`'s linear states directly to a Prism:

* $S$ is your input token $A$.
* $P$ is your successor coproduct $\Sigma_i B_i$.
* The residual $M$ is the unconsumed input $A$.
* **Match:** $A \to \Sigma_i B_i + A$. This is exactly your transition signature!
* **Build:** The residual $A$ passes through unchanged, but what about the successor states $B_i$? This is where the $\S 6$ Dagger ($B_i \to A$) comes in. The recover edges act precisely as the backward pass, catching downstream failures and propagating them up the ladder.

Capucci's work on compositional game theory and open learners models linear/affine resources exactly this way: they route forward, and failures/utilities propagate backward via optics. `rung` is a category of optics.

### 3. The Fritz Map: Async and Generative are Effectful Optics

Since `Result` (with failure returning input) is an optic, what happens to `Future` (Async) and Probability (Generative)?

Tobias Fritz’s work on Markov Categories perfectly validates your affine base:

* **Async (`Future`)** is a standard strong commutative monad. (Tensorial strength $A \otimes T(B) \to T(A \otimes B)$ is the categorical property that proves linearity survives `.await`).
* **Generative (`Prob`)** is modeled in Markov categories via a probability monad (like the Giry monad), which is inherently an affine monad (it preserves the monoidal unit, meaning probability normalizes to 1 and discarding is legal).

You do not need a distributive law to merge them with Error. Instead, category theory provides **Effectful Optics** (or Monadic Optics). You layer the strong monad $M$ strictly on the forward pass of the Prism:


$$A \to M(\Sigma_i B_i + A)$$

### The Verdict for Q7

1. **Gate closed.** The claim "Q7's answer is half-written in the recover edges" resolves to **No**. They are orthogonal categorical gadgets.
2. **The $\S 6$ Dagger is vindicated.** Your RUNG-CT intuition was exactly correct. Recovery is the backward pass of a Prism. The "well-founded progress guard" (G8) simply ensures the optic's backward pass is contractive rather than a symmetric involution.
3. **Actionable path for Async:** Async is genuinely a free-standing feature (Kleisli-in-Future layered on the optic). You will need a distinct async driver—because the monad must be evaluated before the optic can route the token—but it will not require redesigning the algebraic laws of the recovery edges.

You can confidently update the Open Questions document, close the Q7 unification conjecture, and proceed with the blocking-client decision (parked) without fear of incurring architectural debt. Async is a new layer, not a missing piece of the error monad.
