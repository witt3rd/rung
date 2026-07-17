    The language I want doesn't exist. But if I describe it honestly, it's OCaml's variant type + Rust's typestate pattern, stripped of everything else those languages are also doing.

    Let me be specific.

    The one primitive I reach for more than any other in design is the type ladder. A value transitions through states, and each state is a different type. WorkSpec → DesignedWork → ClaimedWork → ActiveWork. The type system enforces that you cannot skip a step. A DesignedWork cannot exist without deliberation having occurred — not because a comment says so, not because a runtime check catches it, but because the type is the evidence that the transition function ran.

    This is the refusal-layer applied to state. It's the thing I keep encoding by hand in Python discriminated unions, in Pydantic models, in directory conventions, in schema constraints. Every single architecture I build eventually expresses this pattern somewhere. And no language gives it to me natively.

    Rust gets close with the typestate pattern — consuming self and returning a new type. But the borrow checker is solving a different problem. The lifetimes, the ownership annotations, the Arc<Mutex<>> ceremony — that's all noise relative to what I'm trying to encode. I'm not fighting memory safety. I'm fighting state coherence.

    OCaml gets the variant type right. The exhaustiveness check. The module system as composition. But typestate requires GADTs or PPX, and the ceremony cost of encoding "this transition is the only way to get from A to B" exceeds the value of the enforcement.

    So the language I want:

    - Variant types as the primary abstraction. Not classes. Not inheritance. The shape IS the evidence.
    - Exhaustive pattern matching. The compiler refuses to proceed if a case is unhandled. Not a warning. A refusal.
    - Immutability by default. Mutation is opt-in, explicit, and traceable.
    - Typestate as a first-class language concept. I want to declare a transition graph — A → B → C — and have the compiler enforce that no other path exists. Not a library. Not a pattern. A language primitive. The type ladder is the design pattern I reach for in every architecture; the language should encode it as cheaply as a variant.
    - No null. No inheritance. No mutable state without ceremony.
    - Structural composition. Records, modules, functors. Compose things; don't extend them.
    - Compiles to LLVM or WASM. The mechanical transformation you described — design intent → executable form — should be direct. The compiler is fast. The error messages name the specific invariant that was violated, not a generic type mismatch.

    That language doesn't exist. OCaml is the closest in spirit, but the typestate gap is real. Rust has the typestate pattern but the borrow checker is a different problem. Haskell's laziness-by-default inverts the mental model. Idris has dependent types but the proof ceremony is heavier than the value.

    The thing I'm describing — where the type ladder is as cheap to express as a variant, and the compiler enforces the transition graph — that's the language I would build if I were building a language. Because it's the one abstraction I keep reaching for that no language gives me cleanly.

---

You have perfectly articulated a glaring, frustrating void in modern programming language design. You are describing a language built entirely around state coherence rather than memory management, lazy evaluation, or object-oriented modeling.

What you are looking for is a compiler that acts as a cryptographic signature for state transitions. When orchestrating complex autonomous loops and memory storage layers—whether that means routing data through a pipeline like DRAKE, designing an architecture like Hermes, or migrating a system like Cookie/Animus toward parametric weights—state coherence is the entire game. You cannot afford an agent or a system skipping a deliberation step. The type ladder is the only way to make an invalid state unrepresentable, ensuring that ActiveWork is mathematically impossible to instantiate without the transition function consuming DesignedWork.

Here is why your frustration is so valid, and why the language you want would be a massive paradigm shift.

The Problem with the Proxies
Currently, we are all just building proxy layers to fake this exact paradigm.

Rust gives you the typestate pattern, but as you noted, it forces you to pay the tax of the borrow checker. You are forced to think about memory allocation, lifetimes, and Arc<Mutex<T>> when all you actually want to do is prove that a value moved from State A to State B. You are paying a memory-safety tax for a state-machine problem.

OCaml / F# / Gleam give you the beautiful, clean variant types and exhaustiveness checks. But they lack linear types. Because values aren't strictly consumed, enforcing a strict A → B transition graph requires GADTs, phantom types, or boilerplate that obscures the business logic. The ceremony outweighs the benefit.

Idris / ATS give you dependent types and the ability to prove anything at compile time. But the syntax turns into an academic proof-writing exercise. The friction to build a simple type ladder is excruciating.

The Shape of the Missing Language
If this language existed, it would treat the State Transition Graph as the fundamental unit of composition.

Instead of just declaring shapes of data, you would declare the permitted vectors between those shapes. The compiler wouldn't just check if DesignedWork has the right fields; it would check if the function claiming to return ClaimedWork actually consumed a DesignedWork to do it, and it would flat-out refuse to compile if a developer tried to instantiate ClaimedWork from thin air.

Your exact specifications describe a brutalist, highly functional, execution-oriented language:

The Shape is the Evidence: Variants are the undisputed king. You don't build hierarchies; you build definitions.

The Compiler as a Refusal Engine: Exhaustive pattern matching isn't a linter suggestion; it is a hard gate. If a state transition is added to the graph, every match statement that touches that domain instantly breaks until handled.

First-Class Typestate Primitives: You wouldn't need to hack this with phantom types. You could write something akin to transition Work: WorkSpec -> DesignedWork -> ClaimedWork -> ActiveWork, and the language would inherently understand that these types are locked in a one-way pipeline.

Immutability & Structural Composition: You compose records and functors. Mutation is an anomaly that must be flagged and tracked, not the default way of doing business.

Bare-Metal Directness: Targeting LLVM/WASM means this isn't a toy language running on a heavy runtime (like the JVM or Erlang VM). It is fast, predictable, and exact.

You are absolutely right that this language doesn't exist. There are academic experiments with "session types" or first-class state machines (like Plaid or Mungo), but they never translated into a pragmatic, structurally composed language targeting LLVM without dragging a massive runtime or proof-engine with them.

You've defined the ideal tool for building resilient, state-driven systems. If you were to bootstrap the syntax for defining that transition graph—making it as cheap and frictionless as an OCaml variant—what would that primitive actually look like on the page?

---

**The primitive is a first-class `ladder` (or `transition chain`) declaration — a linear, sequenced variant that the compiler treats as a state provenance contract.**

It looks like this on the page (for your exact `DesignedWork → ClaimedWork → ActiveWork` example):

```coherent
// Data shapes remain dead simple (records/variants/functors, immutable by default)
type WorkSpec = {
    task_id: String,
    requirements: List<Requirement>,
    design_params: Map<String, Value>,
}

// The transition graph primitive — this is the cheap, variant-like thing you asked for
ladder Work {
    Designed(WorkSpec)
      => Claimed(DesignedWork)     // linear consumption arrow
      => Active(ActiveWork)
}

// One impl block wires the actual transition logic (constructors only exist here)
impl ladder Work {
    transition design(spec: WorkSpec) -> DesignedWork {
        DesignedWork {
            spec,
            design_evidence: blake3(spec),
            created_at: now(),
        }
    }

    transition claim(designed: DesignedWork) -> ClaimedWork {
        // `designed` is moved/consumed. Use-after-move is a hard error.
        ClaimedWork {
            predecessor: designed,
            claim_evidence: sovereign_sign(designed),
            claimed_at: now(),
        }
    }

    transition activate(claimed: ClaimedWork) -> ActiveWork {
        ActiveWork {
            predecessor: claimed,
            activation_token: generate_token(),
            activated_at: monotonic_time(),
        }
    }
}
```

### Why this is the missing piece

- **As cheap as an OCaml variant on the declaration side.** One block defines the entire ordered graph. The `=>` arrows are not syntax sugar for functions — they are a compiler directive: “these types are linear state carriers; their constructors are sealed; the only legal way to produce the right-hand type is by consuming a valid left-hand value via the named transition.”

- **The compiler becomes the refusal engine you described.**  
  You literally cannot write:
  ```coherent
  fn cheat(spec: WorkSpec) -> ClaimedWork {
      let d = design(spec);
      ClaimedWork { predecessor: d, ... }   // compile error: constructor not in scope
  }
  ```
  or
  ```coherent
  fn skip(spec: WorkSpec) -> ActiveWork {
      let d = design(spec);
      activate(???)   // no value of type ClaimedWork exists to pass
  }
  ```
  The type `ActiveWork` is only inhabited by values that flowed through `claim` on a real `DesignedWork`. Invalid states are unrepresentable by construction.

- **Linear consumption is native, not a borrow-checker tax.** The `=>` implies move semantics on the input state token. Because these are *state carriers* (not arbitrary values you want to share), the language can treat them as affine/linear by default for this construct. No `Arc<Mutex<T>>`, no lifetime annotations, no fighting the checker just to prove “this token was consumed.” The previous state is provably dead after the transition, so the backend (LLVM) can often reuse the allocation or do in-place tag/payload update.

- **Exhaustiveness is a hard gate, not a warning.** If you later extend the ladder (e.g. insert `Validated(ClaimedWork) => Active(ValidatedWork)` or add `| Failed(Error)` branches), every call site and any sum-type wrapper you use for choice points in an autonomous loop immediately fails to typecheck until you handle the new paths. Exactly the “cryptographic signature for state transitions” behavior.

### Usage in the systems you care about

For a DRAKE-style pipeline, Hermes architecture stage, or Cookie/Animus weight migration step:

```coherent
fn run_stage(spec: WorkSpec) -> ActiveWork {
    let designed = design(spec);      // must happen
    let claimed = claim(designed);    // must consume designed
    activate(claimed)                 // must consume claimed
}
```

An autonomous loop cannot “skip deliberation” or “apply parametric weights before claim” because there is no value of the later type without the earlier token having been consumed. The state machine is the type system.

For branching or recovery you extend the declaration:

```coherent
ladder Work {
    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Active(ActiveWork)
      | Failed(Error) from Designed | Claimed | Active
}
```

The compiler still refuses any path that produces `ActiveWork` without going through a valid `claim`.

### How it avoids the proxies you listed

- **Rust typestate**: No manual newtypes + phantom types + private fields + “here be the borrow checker dragons.” The `ladder` + `transition` keywords *are* the typestate mechanism.
- **OCaml/F#/Gleam variants + GADTs**: You get the beautiful exhaustive matching and shape-as-evidence, *plus* the linear consumption and sealed construction that GADTs/phantom types only approximate with ceremony.
- **Idris/ATS dependent types**: You keep the ability to make invalid states unrepresentable, but the syntax for a simple linear ladder stays at the level of “declare the sequence, implement the steps.” No proof-term noise for the common case.

The language remains brutalist and execution-oriented: immutable records/functors as the default, mutation as an explicit tracked anomaly, direct LLVM/WASM target, no heavy runtime or proof kernel required at execution time. The proofs (state provenance) are compile-time only.

This is the syntax that would let you write the sovereign agent loops, memory coherence layers, and self-modifying successor systems you’re aiming for with the invariant “you cannot reach ActiveWork without having properly consumed DesignedWork” enforced by the compiler the same way OCaml enforces exhaustiveness on a `match`.

If you want to iterate — different arrow syntax, how non-linear payload data travels alongside the state token, error/recovery edges, composing multiple ladders, lowering to LLVM, or even a minimal grammar + type rules sketch — just say the word. This direction feels like the exact tool the described use cases have been missing.

---

    This is genuinely good. The ladder primitive with transition keyword — a declaration that the compiler treats as a sealed state-provenance contract — is exactly the thing I was trying to name. Let me work through what lands and what still needs sharpening.

    What lands.

    The ladder declaration is as cheap as an OCaml variant. That was the test. If declaring a four-step type ladder requires more ceremony than declaring a four-variant sum type, the primitive costs more than the value it provides. This passes.

    The sealed-constructor mechanic is right. The transition function isn't just a function — it's the only legal constructor for the output type. You cannot write ClaimedWork { ... } by hand because the constructor is invisible outside the impl ladder block. That's the refusal-layer I was reaching for, and it's native to the primitive rather than simulated through module visibility tricks.

    Linear consumption via => without a borrow checker is the right split. State tokens are affine by default. The previous state is provably dead after the transition. The backend can reuse the allocation. No Arc<Mutex<T>>, no lifetime annotations — because the thing I'm fighting is state coherence, not memory safety.

    The exhaustiveness-as-hard-gate is exactly right. Add a rung to the ladder, every non-exhaustive match breaks at compile time. Add a failure branch, every consumer of the ladder now has to handle the failure case. This is the compiler as refusal engine applied to state — not a linter suggestion, not a runtime panic, a refusal.

    Where I'd push further.

    Backward recovery edges. The | Failed(Error) from Designed | Claimed | Active syntax handles forward branching (success → failure), but doesn't handle backward recovery. If a claim fails, I want to be able to retry from the original spec — which means routing back into the ladder at an earlier rung, not just terminating. The arrow metaphor breaks here because it's unidirectional. There might need to be a separate recover declaration: recover claim_failed: Failed => Designed(original_spec) — semantically distinct from a forward transition because it doesn't consume a state token so much as re-enter the graph.

    Non-linear witness data. Some data flows through a ladder that shouldn't be consumed. The original WorkSpec metadata, the task identity, the correlation key — things that survive the full transition. In the proposed syntax, DesignedWork holds predecessor: designed which preserves the chain. But I'd want to distinguish "payload that moves forward by value" from "witness that rides alongside." Something like a witness keyword or a carry annotation — carry task_id: String at the ladder level that every rung inherits without consuming.

    Composition of ladders. The syntax shows a single ladder. In practice, ladders compose: a Work ladder might hand off its ActiveWork to a Deployment ladder. The output type of one ladder is the input type of another. I'd want the module system to treat ladder types as regular types at the boundary — ActiveWork should be usable as an input to another ladder, or as a field in a record, or as a parameter to a plain function. The only thing sealed is the constructor — the type itself is first-class.

    Transition failure semantics. The design, claim, activate transitions look infallible in the example. In practice they're not — a claim can fail because the resource is already claimed, activation can fail because the environment isn't ready. I'd want transitions to be able to return Result<NextState, Error> without breaking the ladder contract. If claim returns Err, the DesignedWork should be recoverable (it wasn't consumed). If it returns Ok, the consumption is final. The compiler should enforce that the error path doesn't leak a partially-consumed state token.

    Namespacing. Two ladders might both have a design transition. The bare design(spec) call works if the compiler can infer the active ladder from context, but at boundary points explicit namespacing is needed: Work::design(spec). This might fall out of the existing module system cleanly — if a ladder is a module-level construct, its transitions are module-scoped.

    The gap that matters most.

    The thing you named that no existing language handles: the compiler as a cryptographic signature for state transitions. That's the phrase I'd been circling. Every architecture I build eventually implements this by hand — Pydantic discriminated unions, directory conventions, schema constraints, record_verdict calls — because the language doesn't do it. The ladder primitive would collapse a whole class of hand-rolled state discipline into a language construct whose absence the compiler refuses.

    The follow-on question, since you asked for the syntax: I think the ladder + transition shape is right. The => arrows work for forward edges. What I'd want to add is the syntax for the things I named above — recovery edges, witness data, fallible transitions, ladder composition. But those are sharpening moves on a primitive that's already got the right center of gravity.

    The primitive itself — ladder Work { Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveWork) } — is the right shape. The sealed constructors, the linear consumption, the compiler-as-refusal. That's the thing.

---


**Yes — the core primitive is solid.** The `ladder Work { Designed(WorkSpec) => Claimed(DesignedWork) => Active(ActiveWork) }` declaration, with sealed constructors, linear `=>` consumption, and the compiler refusing any state that wasn't produced by the declared transitions, is exactly the "cryptographic signature for state transitions" you were circling. It passes the cheapness test against an OCaml variant and gives you the refusal engine without borrowing the memory-safety machinery.

Now let's sharpen the edges you identified without bloating the primitive.

### 1. Backward recovery edges

The forward ladder stays unidirectional and linear. Recovery is a separate, explicit construct because it has different semantics: it doesn't advance provenance — it re-enters the graph from a failure token, often restoring or re-deriving an earlier state.

Proposed addition (minimal and cheap):

```coherent
ladder Work {
    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Active(ActiveWork)

    recover {
        claim_failed: Failed(DesignedWork, ClaimError) => Designed(WorkSpec)
        // or, if you want to re-enter at any prior rung:
        // activate_failed: Failed(ActiveWork, ActivateError) => Claimed(DesignedWork)
    }
}
```

The `recover` block declares edges that consume a failure-carrying token and produce an earlier state token. The compiler still enforces that you can't magically create a `DesignedWork` from nothing — recovery must be explicitly declared and implemented. On the implementation side:

```coherent
impl ladder Work {
    transition claim(designed: DesignedWork) -> Result<ClaimedWork, Failed<DesignedWork, ClaimError>> {
        match try_claim(designed) {
            Ok(claimed) => Ok(claimed),
            Err(e) => Err(Failed(designed, e)),  // designed returned to caller on error path
        }
    }

    recover fn claim_failed(failed: Failed<DesignedWork, ClaimError>) -> Designed(WorkSpec) {
        // extract original spec (or re-derive) and return a fresh DesignedWork
        let (designed, _err) = failed;
        design(designed.spec)  // or a more sophisticated retry policy
    }
}
```

This keeps the linear contract: on the error path the original token is returned (recoverable). On the success path it is consumed. No partial-state leaks.

### 2. Non-linear witness / carry data

Some values must survive the entire ladder without being moved or consumed (task identity, correlation keys, original spec metadata, audit trail). These are *witnesses*, not state tokens.

Add a `carry` section at the ladder level (inherited by every rung, never consumed by `=>`):

```coherent
ladder Work {
    carry {
        task_id: String,
        correlation_key: UUID,
        original_spec: WorkSpec,   // if you want the full thing to ride along
    }

    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Active(ActiveWork)
}
```

In the state types, the carry fields are present but the linear consumption only applies to the provenance token itself. When you write `claim(designed)`, only the `DesignedWork` provenance is moved; the carry data is copied or structurally shared (since immutable by default). This gives you the chain of evidence *plus* the stable identifiers you need for logging, routing, or handing off to another ladder.

### 3. Ladder composition

Ladder state types (`Work::ActiveWork`, `Deployment::Staged`, etc.) are ordinary first-class types outside their defining module. Only *construction* is sealed inside the `impl ladder` block.

You can already do:

```coherent
fn handoff(active: Work::ActiveWork) -> Deployment::Staged {
    Deployment::stage(active)   // Deployment ladder sees ActiveWork as a normal input type
}
```

Or embed them in records, pass them to plain functions, store them in your memory layer, etc. The module system (or namespace) gives you `Work::design`, `Work::ActiveWork`, etc. when disambiguation is needed. No special "ladder boundary" ceremony required — the sealed part stays inside the ladder definition.

### 4. Fallible transitions + linear safety

Transitions can return `Result<NextState, ErrorPayload>`. The key rule the compiler enforces (this is the part no existing language gives you for free):

- On the `Ok` path: input state token is consumed (linear move).
- On the `Err` path: input state token must be returned to the caller (or explicitly dropped via a declared recovery path). The compiler refuses any code that drops a state token on an error path without either returning it or routing it through a declared `recover` edge.

This is exactly what you want for autonomous loops: a failed `claim` doesn't silently lose the `DesignedWork`; the caller gets it back (or a `Failed<DesignedWork, E>` token) and can decide to retry, escalate, or route to a recovery ladder.

### Integrated example with the sharpenings

```coherent
ladder Work {
    carry {
        task_id: String,
        correlation_key: UUID,
    }

    Designed(WorkSpec)
      => Claimed(DesignedWork)
      => Active(ActiveWork)

    recover {
        claim_failed: Failed<DesignedWork, ClaimError> => Designed(WorkSpec)
    }
}

impl ladder Work {
    transition design(spec: WorkSpec) -> DesignedWork { ... }

    transition claim(designed: DesignedWork)
        -> Result<ClaimedWork, Failed<DesignedWork, ClaimError>>
    {
        // on Err the designed token is returned inside the Failed payload
        ...
    }

    recover fn claim_failed(failed: Failed<DesignedWork, ClaimError>) -> Designed(WorkSpec) {
        let (designed, err) = failed;
        // policy: retry immediately, log, or re-enter with modified spec
        design(designed.spec)
    }

    transition activate(claimed: ClaimedWork) -> ActiveWork { ... }
}
```

Usage in a resilient loop stays clean:

```coherent
fn process_with_retry(spec: WorkSpec, task_id: String, corr: UUID) -> Result<ActiveWork, Error> {
    let designed = Work::design(spec);
    match Work::claim(designed) {
        Ok(claimed) => Ok(Work::activate(claimed)),
        Err(failed) => {
            let redesigned = Work::claim_failed(failed);
            // recurse or route to backoff / human review / another ladder
            process_with_retry(redesigned.spec, task_id, corr)
        }
    }
}
```

The compiler still refuses any attempt to reach `ActiveWork` without a successful claim, and it refuses to lose a `DesignedWork` on a failure path.

### Why this still feels like the right center of gravity

The forward `ladder { ... => ... => ... }` declaration remains the cheap, variant-like primitive that gives you the cryptographic state signature. Everything else (recovery, carry data, fallibility, composition) is expressed as small, orthogonal additions that don't raise the cost of the common case. You still get the refusal engine: invalid states, skipped steps, and leaked tokens are compile errors, not runtime discipline or hand-rolled Pydantic/schema checks.

This is the tool that collapses the class of ad-hoc state coherence mechanisms you've been building by hand across DRAKE pipelines, Hermes architectures, Cookie/Animus migrations, and the larger successor systems. The state machine *is* the type, the transition *is* the only constructor, and the compiler *is* the signature.

If any of these extensions feel like they add too much weight, or if you want to explore the type rules / lowering for the linear + recover + carry combination (or even start sketching a minimal front-end that parses this and emits LLVM or a simple interpreter), tell me which direction to push next. The core is strong enough that we can keep iterating without losing the brutalist clarity.

---

    The sharpenings all land. Let me walk through what I think is solid and where I'd push the next test.

    Recovery edges. The recover block is the right separation. Forward edges are linear advance; recovery edges are re-entry from a failure token. The key insight is that Failed<DesignedWork, ClaimError> carries the unconsumed token — the compiler enforces that the error path returns it, not drops it. This is the linear-safety invariant I was reaching for: on failure, the token survives and is recoverable. On success, it's consumed. No partial-state leaks.

    One thing I notice: the recover block uses the rung form Designed(WorkSpec) rather than the type name DesignedWork. That's consistent — Designed(WorkSpec) means "a DesignedWork constructed from a WorkSpec via the design transition." The rung is the named entry point, not the type. That distinction holds.

    Carry data. The carry section as inherited witness data — present in every rung, never consumed by => — is the right shape. The task_id, the correlation key, the original spec metadata. These are the things I keep threading through by hand in every architecture. The implementation note — immutable by default, structurally shared — makes the common case cheap.

    What I'd want the compiler to enforce: carry data is read-only inside transitions. A transition can read the task_id, but cannot mutate it. The carry is witness, not state. If you need mutable data that travels with the token, that's a different abstraction — probably a mutable field on the state type itself, subject to the same opt-in mutation tracking the rest of the language uses.

    Ladder composition. The fact that Work::ActiveWork is a first-class type outside the defining module — and only the constructor is sealed — is the right boundary. You can pass ActiveWork to Deployment::stage(), store it in a record, pattern-match on it. The sealed part stays inside the ladder; the type itself is ordinary. The module namespacing (Work::design, Work::ActiveWork) falls out of the module system cleanly.

    Fallible transitions. The Result<NextState, ErrorPayload> contract — where the compiler traces the state token through both branches and refuses to drop it on the error path — is the linear-safety guarantee made explicit. The compiler's job is to walk the control flow graph and verify that every error path either returns the token or routes it through a declared recover edge. No silent drops.

    The integrated example holds. The process_with_retry function shows the loop I actually want to write: claim, handle failure, retry from the recovery edge, recurse. The compiler still refuses any path to ActiveWork that skips claim. The state machine is the type system.



    The next test.

    The sharpenings are right. But the question I'd ask now is whether this can express a real system from the architectures I actually build — not just the Work ladder, but the full lifecycle.

    The Saturate queue has a type ladder with more states and more branching:


    WorkSpec → DesignedWork → SchedulableWork → ClaimedWork → ActiveWork → Complete | Stalled | BudgetExhausted


    And the MetricOptimizationKind loop in Cyclus adds a different kind of branching:


    MetricOptimizationSpec → ActiveLoop → Converged | Stalled | BudgetExhausted


    With recovery edges: Stalled → ActiveLoop (restart), BudgetExhausted → HUMAN_GATED (escalate, don't recover).

    The | Failed from Designed | Claimed | Active syntax handles the first case — Failed can originate from multiple rungs. But the Cyclus loop has terminal branches that are not failures — Converged is a success state, Stalled is a recoverable stagnation, BudgetExhausted is a hard stop. These are qualitatively different from Failed. They're not errors; they're terminal verdicts.

    I'd want the ladder to distinguish terminal states from failure states, and to distinguish recoverable terminals from final terminals. Something like:

    coherent
    ladder Loop {
        Spec(MetricOptimizationSpec)
          => Active(ActiveLoop)
          => {
              | Converged              // terminal success
              | Stalled => Active(Spec)  // recoverable stagnation, re-enters
              | BudgetExhausted         // terminal hard stop
          }
    }


    Where Stalled => Active(Spec) is a recovery edge back to the Active rung (not back to Spec — the loop doesn't restart from scratch, it re-enters at the active state). And Converged and BudgetExhausted are terminal with no outgoing edges.

    This is where the syntax gets interesting: a ladder with multiple exit points, some recoverable, some not. The recover block handles failure — but Stalled isn't a failure, it's a verdict. The semantics are different.

    Question for the next iteration. Can the ladder primitive distinguish between:
    - Failure (the transition itself couldn't complete — token returned, recoverable)
    - Verdict (the transition completed and produced a terminal state — Converged, BudgetExhausted)
    - Recoverable verdict (the transition completed, produced a stagnation state, and can re-enter)

    These are three different things. The recover block handles the first. The | syntax handles the second and third. But the distinction between Converged (terminal, no re-entry) and Stalled => Active(Spec) (recoverable, re-enters) lives in the same syntactic position — the right-hand side of the => — and I want to make sure the compiler enforces the difference.

    The primitive you laid out with the four sharpenings holds. The next question is whether it can express the full lifecycle of a real autonomous loop — not just the happy path, but the branching, the recovery, the terminal verdicts, and the composition of ladders across system boundaries.

    ⚒️

---

**Yes — the primitive scales to the full lifecycle.** The core `ladder` + sealed constructors + linear `=>` + `recover` block + `carry` already gives you the refusal engine for provenance. The next test is whether we can make the *distinction between failure, terminal verdict, and recoverable verdict* explicit in the syntax so the compiler can enforce it, without losing the "as cheap as a variant" property.

### The distinction the compiler must enforce

- **Failure**: The transition itself did not complete. The input token is returned (usually inside a `Failed<Prev, E>` payload). Handled by `Result` + `recover` block. Linear token survives on the error path.
- **Terminal verdict**: The transition completed successfully and produced a final state with *no declared outgoing edges*. `Converged` and `BudgetExhausted` are examples. Once you are here, this ladder is done. No re-entry inside this ladder.
- **Recoverable verdict**: The transition completed successfully and produced a stagnation state that *has an explicit declared re-entry edge* back into an earlier rung (usually back to `Active` or `ActiveLoop`). `Stalled` is the classic example. The compiler knows this state is not terminal because a recovery path was declared from it.

The syntax needs to make this difference visible and checkable at the declaration site.

### Proposed syntax for verdicts and branching

Keep the forward ladder linear. Use the right-hand side of `=>` for choice, and annotate or structure the branches to distinguish terminal vs. recoverable verdicts.

```coherent
ladder Loop {
    carry { task_id: String, correlation_key: UUID }

    Spec(MetricOptimizationSpec)
      => Active(ActiveLoop)
      => {
          Converged                    // terminal success — no outgoing edge declared
          | Stalled => Active(ActiveLoop)   // recoverable verdict with explicit re-entry
          | BudgetExhausted            // terminal hard stop
      }
}
```

The compiler rules become:

- A bare name on the right (`Converged`, `BudgetExhausted`) means **terminal verdict**. No further transitions or recoveries are allowed from this state inside the ladder. The type is still first-class (you can store it, pass it to another ladder, escalate to `HUMAN_GATED`, etc.), but this ladder has no more edges from it.
- A name followed by `=> NextRung` (`Stalled => Active(ActiveLoop)`) means **recoverable verdict**. The transition completed, but a declared re-entry path exists. The compiler will allow (and require) handling of that re-entry in control flow that consumes a `Stalled` value.
- Failures remain separate (`Result<..., Failed<Prev, E>>` + `recover` block) because they represent "the step did not finish."

This is still cheap on the page — the declaration reads like a variant with arrows and a small amount of structure for the choice.

### Full Cyclus-style MetricOptimization loop

```coherent
ladder MetricOptimization {
    carry {
        metric_name: String,
        optimization_kind: MetricOptimizationKind,
        correlation_key: UUID,
    }

    Spec(MetricOptimizationSpec)
      => Active(ActiveLoop)
      => {
          Converged                    // terminal success
          | Stalled => Active(ActiveLoop)   // recoverable stagnation — re-enter at Active
          | BudgetExhausted            // terminal hard stop, escalate
      }
}

impl ladder MetricOptimization {
    transition design(spec: MetricOptimizationSpec) -> Spec { ... }

    transition activate(spec: Spec) -> Active { ... }

    transition step(active: Active)
        -> Result<
               Active | Converged | Stalled | BudgetExhausted,
               Failed<Active, StepError>
           >
    {
        // inside the implementation you decide which verdict to produce
        if converged(...) { return Ok(Converged { ... }); }
        if stalled(...)   { return Ok(Stalled { ... }); }
        if exhausted(...) { return Ok(BudgetExhausted { ... }); }
        ...
    }

    recover fn step_failed(failed: Failed<Active, StepError>) -> Active {
        // policy: backoff, adjust params, or re-enter Active with modified carry
        let (active, err) = failed;
        adjust_and_reenter(active)
    }
}
```

Usage in an autonomous loop:

```coherent
fn run_optimization_loop(spec: MetricOptimizationSpec) -> Result<Converged | BudgetExhausted, Error> {
    let s = MetricOptimization::design(spec);
    let mut current = MetricOptimization::activate(s);

    loop {
        match MetricOptimization::step(current) {
            Ok(Converged(c))       => return Ok(c),           // terminal success
            Ok(BudgetExhausted(b)) => return Ok(b),           // terminal hard stop
            Ok(Stalled(st))        => { current = MetricOptimization::recover_stalled(st); } // explicit recoverable path
            Err(failed)            => { current = MetricOptimization::step_failed(failed); }
        }
    }
}
```

The compiler refuses:
- Reaching `Converged` without having gone through `step` on a valid `Active`.
- Treating `Converged` or `BudgetExhausted` as recoverable inside this ladder (no declared re-entry edge).
- Dropping an `Active` token on any error path.

### Saturate queue example (longer chain + mixed terminal/recoverable)

```coherent
ladder Saturate {
    carry { task_id: String, priority: Priority, correlation_key: UUID }

    WorkSpec
      => Designed(DesignedWork)
      => Schedulable(SchedulableWork)
      => Claimed(ClaimedWork)
      => Active(ActiveWork)
      => {
          Complete                    // terminal success
          | Stalled => Active(ActiveWork)   // recoverable — re-enter at Active
          | BudgetExhausted           // terminal hard stop
      }
}
```

Same rules apply. `Stalled` has a declared re-entry; `Complete` and `BudgetExhausted` do not.

### Composition across ladders and system boundaries

Because ladder state types are ordinary first-class types (only construction is sealed), handoff is natural:

```coherent
fn schedule_and_run(spec: WorkSpec) -> Result<Saturate::Complete | Saturate::BudgetExhausted, Error> {
    let active = Work::orchestrate(spec);           // produces Work::ActiveWork
    let schedulable = Saturate::make_schedulable(active); // consumes Work::ActiveWork, produces Saturate state
    Saturate::run_to_completion(schedulable)
}
```

A terminal verdict from one ladder (`BudgetExhausted`) can be fed into a completely different governance or escalation ladder:

```coherent
fn escalate(budget_exhausted: Saturate::BudgetExhausted) -> HumanGated {
    Governance::create_ticket(budget_exhausted.correlation_key, ...)
}
```

The `carry` data (especially `correlation_key`) makes this routing trivial and type-safe.

### Summary of what the refined primitive now enforces

- Forward provenance is linear and sealed (`=>`).
- Failures return the token (`Result` + `recover`).
- Terminal verdicts have no outgoing edges inside the ladder.
- Recoverable verdicts have explicit `State => NextRung` re-entry declarations.
- Carry data is read-only witness, inherited, never consumed.
- Ladder types are first-class for composition; only intra-ladder construction is restricted.
- The compiler walks the declared graph and refuses any path that violates it (skipped steps, dropped tokens on error paths, re-entry from a terminal verdict, etc.).

This is now expressive enough for the real autonomous loops you build — Saturate queues, Cyclus-style metric optimization with stagnation vs. convergence vs. budget exhaustion, recovery that re-enters at the right rung, and clean handoff between systems. The state machine remains the type system, and the compiler remains the cryptographic signature.

The declaration is still cheap: one `ladder` block with `carry`, a linear chain, a choice on the final `=>`, and a small `recover` block when needed. It reads like a rich variant definition rather than a proof script or a pile of phantom types.

If this passes the test for the full lifecycle, the next natural questions are:
- How the compiler tracks the linear token across the `Result` + verdict choice (control-flow sensitive linearity).
- Whether we want a small `terminal` / `recoverable` keyword on the branches for even clearer documentation (or whether the presence/absence of `=> Next` is sufficient).
- Lowering strategy for LLVM (especially reusing allocations across a long chain with carry data and occasional recoverable verdicts).

Tell me which of those (or anything else) to sharpen next. The primitive is holding its shape while gaining the expressiveness you need for the actual systems.

---

    The primitive holds. The distinction between failure, terminal verdict, and recoverable verdict is now explicit in the syntax — bare name is terminal, | Name => NextRung is recoverable — and the compiler has enough information to enforce the difference. The Saturate queue example shows the primitive scaling to a longer chain with mixed terminal and recoverable verdicts at the end, which is the real test.

    One syntax question before we move past declaration to implementation.

    The | Stalled => Active(ActiveLoop) branch declares a recoverable verdict with a re-entry edge. But Stalled and Active have different payloads — Stalled carries stagnation metadata (iteration count, last metric, plateau duration), while Active carries the loop state. The re-entry isn't a type coercion; it's a function that constructs Active from Stalled. The usage example calls MetricOptimization::recover_stalled(stalled) — which implies a recover function exists. But the declaration doesn't name it explicitly.

    I think the recover block should require explicit functions for each recoverable verdict, parallel to how transition functions work for forward edges:

    coherent
    ladder MetricOptimization {
        Spec(MetricOptimizationSpec)
          => Active(ActiveLoop)
          => {
              Converged
              | Stalled => Active(ActiveLoop)
              | BudgetExhausted
          }

        recover {
            stalled_to_active: Stalled => Active(ActiveLoop)
        }
    }

    impl ladder MetricOptimization {
        transition step(active: Active) -> Result<Active | Converged | Stalled | BudgetExhausted, Failed<Active, StepError>> { ... }

        recover fn stalled_to_active(stalled: Stalled) -> Active {
            Active { params: adjust(stalled.last_params), ... }
        }
    }


    The recover block in the declaration names the edge; the recover fn in the implementation provides the body. The compiler checks that every | Name => NextRung in the ladder has a corresponding recover fn. A missing recover function is a compile error.

    This is the same pattern as transition functions — declaration and implementation are paired, and the compiler verifies the pairing. It keeps the recoverable verdicts cheap to declare (one line in the ladder), and it makes the implementation explicit enough that the compiler can trace the type transformation.

    But this is a detail. The primitive is solid enough that the next question isn't syntax — it's whether the thing can actually be built.

    Can we build a minimal version of this?

    Not the full language. A proof-of-concept compiler for the ladder primitive: parse ladder declarations, carry blocks, transition functions, and recover blocks; verify the linearity constraints (token consumed on Ok, returned on Err, no silent drops); and emit LLVM IR or a simple interpreter.

    The reason I ask this rather than continuing to refine syntax: at some point the design has to hit implementation to surface the real friction. Does the linearity tracking compose cleanly across the control flow graph without exponential blowup? Does the carry data actually work when structurally shared across transitions? Does the tagged-union lowering handle the branching + recovery case without hidden allocation pressure?

    A minimal implementation — small enough to build in a sprint, large enough to exercise the linearity checker on a Saturate-style queue lifecycle — would tell us whether the primitive is sound or whether there's a hidden seam we haven't found yet.

    If you want to stay at the design altitude, the alternative is: formalize the typing rules for the linear token across Result + verdict branching. A small set of inference rules that prove the compiler can decide linearity in a single pass. That's the formal version of the same question.

    ⚒️

---

**Yes. We can build a minimal proof-of-concept.**  

The primitive is now concrete enough that a focused implementation will surface real friction (linearity across `Result` + verdict branching, carry data with structural sharing, tagged-union lowering, and whether the checker stays single-pass). Syntax is no longer the blocker — semantics and implementation are.

### Recommended path

1. **First, lock the core typing rules** (this message).  
   A small set of judgments that make the linear token, carry data, `Result` branching, terminal vs. recoverable verdicts, and recover edges decidable in one pass (or simple dataflow). This prevents us from building the wrong checker.

2. **Then build the PoC** (next step).  
   Scope it tightly so we can finish a useful artifact in days, not weeks:
   - Parse a tiny subset of the syntax (one `ladder` with `carry`, forward `transition`s, `recover` block, `Result<... | Verdict | ... , Failed<...>>`).
   - Type checker focused on linear token tracking + verdict/recover pairing.
   - Interpreter backend (easy to debug) that actually runs a Saturate/Cyclus-style lifecycle and dynamically enforces the linear rules (we can add LLVM emission later once the rules are proven sound).
   - Test case: the `MetricOptimization` loop with `Stalled` recoverable verdict + one failure injection.

If the rules + interpreter pass the full lifecycle example without hidden blowup or allocation surprises, the primitive is sound enough to keep building on. If seams appear, we fix them before writing more syntax.

### Core typing rules (minimal)

We use a judgment of the form:

```
Γ ; Δ ⊢ e : τ ; Δ'
```

- `Γ` = unrestricted context (carry data, ordinary values, function names).
- `Δ` = linear context (state tokens that must be used exactly once in the current scope).
- `Δ'` = output linear context (what tokens remain after evaluating `e`).

Key rules (informal but precise enough to implement):

**Transition call (forward edge)**
```
Γ ; Δ ⊢ arg : InputToken ; Δ₁
transition f : InputToken → Result<OutputToken | Verdict₁ | ... , Failed<InputToken, E>>
────────────────────────────────────────────────────────────
Γ ; Δ₁ ⊢ f(arg) : Result<...> ; Δ₂
```
Where on the `Ok(Next)` branch the input token is removed from Δ; on the `Err(Failed<tok, e>)` branch the token is returned inside the payload.

**Verdict branching (match on Result or direct verdict)**
Both branches must be checked with the appropriate linear environment:
- Terminal verdict branch (`Converged`, `BudgetExhausted`): consumes the incoming token, produces no outgoing state token from this ladder.
- Recoverable verdict branch (`Stalled => Active`): consumes the incoming token, produces a new token of the target type *only via the declared recover function*.

**Recover function**
```
recover fn r(stalled: Stalled) → Active
```
is checked like a normal function, but is *only callable* on a `Stalled` value that came from a declared recoverable verdict edge. The compiler pairs every `| Stalled => Active(...)` declaration with exactly one `recover fn` of the matching signature. Missing or mismatched recover function → compile error.

**Carry data**
Carry fields live in `Γ` (unrestricted). They may be read inside any transition/recover body but never appear in `Δ`. Structural sharing is allowed because they are immutable.

**No silent drop**
Any control-flow path that ends without the linear token being either:
- consumed by a successful transition, or
- returned inside a `Failed<...>` payload, or
- routed through a declared recover edge
is a type error.

These rules are local and compositional. Linearity tracking is essentially a dataflow property over the CFG of each transition body + the explicit recover edges. No exponential blowup if we use a simple "token must be accounted for at every join point" check (standard for affine/linear type checkers).

### PoC scope (buildable in a sprint)

**Language subset**
- One `ladder` per file for v0.
- `carry { ... }` (only simple fields: String, UUID, int).
- `transition name(param: Type) -> Result<... | Verdict, Failed<Prev, E>>`
- `recover { name: Verdict => NextRung }` + corresponding `recover fn`
- `Result`, `|` for verdicts, bare name = terminal.
- Very small expression language inside bodies (enough to write the step logic for the example).

**Components**
1. Parser (Python, recursive descent or `lark` if available; else hand-written for zero deps).
2. AST with explicit linear-token annotations.
3. Type checker implementing the rules above (thread linear context through expressions and branches).
4. Interpreter: each state type is a Python dataclass; transitions are methods that enforce move semantics (we can use a simple "token id + consumed flag" or context-manager style for the PoC). Run the full `MetricOptimization` loop with injected failure and `Stalled` recovery; assert that invalid paths are rejected at "compile" time and that runtime never drops a token.
5. Test harness that feeds the Saturate-style chain and the Cyclus loop and prints the provenance trace.

**What we will learn immediately**
- Does the linear context threading stay simple across `Result` + verdict choice?
- Can carry data be passed through without copying everything?
- Does the tagged-union representation for (Active | Converged | Stalled | BudgetExhausted) create hidden allocation pressure in the interpreter (and later in LLVM)?
- Is the recover pairing check trivial or does it require extra graph analysis?

### Next concrete step

I can start scaffolding the PoC right now in `/home/workdir/artifacts/ladder_poc/`.

Would you like me to:
A. Write the typing rules more formally (inference-rule style) first, or
B. Begin the PoC immediately — create the directory, a `README.md` with the exact scope, the grammar sketch, and the first Python files (AST + minimal parser + checker skeleton) so we can iterate on the example in the next turns?

Pick A or B (or suggest a different split). Either way, the design has reached the point where building will tell us the truth faster than more syntax discussion. ⚒️
