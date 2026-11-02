# Rung Architecture — PoC

## AST

```
Ladder
├── name: str
├── carry: Carry
├── rungs: list[Rung]
├── transitions: list[Transition]
├── recover_edges: list[RecoverEdge]
└── recover_fns: list[RecoverFn]

Carry
├── fields: dict[str, Type]       # name -> type (witness, never consumed)

Rung
├── name: str                      # e.g. "Spec", "Active"
├── payload_type: Type             # the type this rung carries

Transition
├── name: str                      # e.g. "design", "step"
├── from_rung: str                 # consumes this rung
├── to_rung: str | None            # None if branching
├── verdicts: list[Verdict]        # if branching, the possible outcomes
├── params: list[Param]
├── body: Expr                     # the transition logic

Verdict
├── name: str                      # e.g. "Converged", "Stalled", "BudgetExhausted"
├── is_terminal: bool              # True = no outgoing edges
├── recover_target: str | None     # rung name for recoverable verdicts

RecoverEdge
├── name: str                      # e.g. "stalled_to_active"
├── from_verdict: str              # the recoverable verdict name
├── to_rung: str                   # the rung it re-enters

RecoverFn
├── name: str                      # matches a RecoverEdge name
├── param_type: Type               # the verdict type
├── return_rung: str               # the target rung
├── body: Expr
```

## Type checking rules

### Linear context
A set of available token references. Tokens are consumed by transitions and produced by recover functions. At each control flow join point, the linear context must be accounted for (no silent drops).

### Transition call
- Input: a token of the `from_rung` type must be in the linear context
- On success (Ok branch): the input token is consumed; if the transition returns a single next rung, that token is added to context
- On failure (Err(Failed<tok, e>)): the input token is returned to context inside the Failed wrapper

### Verdict branching
For a `Result<NextRung | V1 | V2 | ..., Failed<Prev, E>>`:
- Ok branch with `NextRung`: token consumed, new token produced
- Ok branch with terminal verdict `V1`, `V2`: token consumed, no new token (ladder complete)
- Ok branch with recoverable verdict `V3 => NextRung`: token consumed, recover function must be present
- Err branch: token returned inside Failed<Prev, E>

### Recover function
- Input: a Failed<Prev, E> token or a recoverable verdict token
- Output: produces a new token of the target rung type
- Compiler verifies: every RecoverEdge has exactly one matching RecoverFn

### Carry data
- Carry fields are in the unrestricted context (never consumed)
- Transitions and recover functions may read carry fields but never modify them
- Structural sharing is permitted (immutable)

## Interpreter

State tokens are Python dataclasses with:
- A `_consumed: bool` flag
- Accessing a consumed token raises `LinearError`
- Transitions set `_consumed = True` on the input token
- Recover functions produce fresh tokens of the target type

The interpreter walks the AST and executes the MetricOptimization loop:
1. `design(spec)` → produces `Spec` token
2. `activate(spec)` → produces `Active` token  
3. `step(active)` → returns `Result[Converged | Stalled | BudgetExhausted, Failed<Active, E>]`
4. Match on result:
   - `Converged` → done
   - `Stalled` → `recover_stalled(stalled)` → `Active`, go to 3
   - `BudgetExhausted` → done
   - `Err(Failed(active, e))` → `step_failed(failed)` → `Active`, go to 3

The interpreter prints a provenance trace showing each transition and its outcome.