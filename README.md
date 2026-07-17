# rung ⚒️

A proof-of-concept compiler for the `ladder` primitive — a first-class type ladder where the state machine *is* the type system and the compiler is a cryptographic signature for state transitions.

## What this is

A minimal implementation of the core primitive described in `notes.md`:

- **`ladder`**: a declaration of a linear state transition graph (`WorkSpec → DesignedWork → ClaimedWork → ActiveWork → Complete`)
- **`carry`**: witness data that rides alongside every rung, read-only, never consumed
- **`transition`**: the only legal constructor for the next rung — linear consumption of the prior state token
- **`recover`**: explicit re-entry edges from failure or recoverable verdicts back into the ladder
- **Verdict branching**: the final rung can produce terminal verdicts (no outgoing edges) or recoverable verdicts (explicit `| Name => NextRung` re-entry)

The compiler enforces:

1. No skipping rungs — a state token is only obtainable via the declared transition
2. No silent token drops — every error path returns the token or routes it through a declared recover edge
3. No re-entry from terminal verdicts — `Converged` and `BudgetExhausted` have no outgoing edges
4. Every recoverable verdict (`| Stalled => Active`) has a matching `recover fn`

## PoC scope

Deliberately minimal. One ladder, one file, hand-constructed AST, Python interpreter backend.

- **Parser**: none (hand-constructed AST for the PoC; a real parser is v2)
- **Type checker**: linear context threading across `Result` + verdict branching, single-pass
- **Interpreter**: Python dataclasses, enforces move semantics via token tracking at runtime
- **Test case**: `MetricOptimization` loop with `Stalled` recovery + one injected failure

## What we're testing

The PoC answers one question: **does the linearity checker stay simple across `Result` + verdict branching?**

Secondary questions:
- Can carry data be passed through without copying everything?
- Does the tagged-union representation for verdicts create hidden allocation pressure?
- Is the recover-pairing check trivial or does it require extra graph analysis?

## Structure

```
rung/
├── rung/           # core package
│   ├── ast.py       # AST nodes (Ladder, Carry, Transition, Recover, Verdict)
│   ├── checker.py   # linear type checker
│   └── interpreter.py  # runtime that enforces linear rules
├── examples/
│   └── metric_opt.py   # MetricOptimization loop as the test case
├── tests/
│   └── test_linearity.py  # test that the checker rejects invalid programs
├── notes.md         # the design conversation that produced this
├── ARCHITECTURE.md  # how the PoC is structured
└── README.md        # this file
```

## Build & run

```bash
cd rung
python3 examples/metric_opt.py
python3 -m pytest tests/
```

## Status

**2026-07-16** — PoC scaffolded. Building the AST, checker, and interpreter against the `MetricOptimization` example.