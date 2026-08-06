---
id: q6
status: open
depends_on:
  - {on: SPEC:G2, kind: premise}
  - {on: SPEC:G8, kind: premise}
---

# Q6 — Genericity *(open)*

**Status:** OPEN

**Question.** Can ladders be parameterized over payload/carry types (a generic
ladder instantiated per use)?

**Why it's open.** Mostly a macro-engineering question (generic parameters through
the emitted module), but the interaction with the sealed-constructor visibility
rule (G2) and the progress-guard bounds (G8) needs design.
