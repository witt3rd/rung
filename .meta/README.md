# rung — outside-judge orientation

**Status:** `.meta/` — informative, not an object of the register. Written 2026-07-23.

This document is for whoever audits rung's `.gl/schema.json` charter — the outside
reader who must judge whether a given ladder declaration satisfies the belonging-law.
Read it before reading the charter. Read the charter before reading any ladder.

---

## What rung is

`rung` is a Rust proc-macro that enforces typestate for sequential state machines.
You declare a ladder of named rungs and verdicts; the macro emits Rust types and
enforces — at compile time — that each state can only be reached by traversing a
declared transition, never fabricated. The deep grounding: Lawvere's fixed-point
theorem is instantiated in the sealed constructor. A verb cannot occupy
object-position. A state is reached only by traversing an arrow.

The canonical reference is `docs/SPEC.md` — normative, binding, machine-enforced.
The design judgments (J1/J2) are in `docs/JUDGMENTS.md` — normative, binding,
oracle-enforced. You are the oracle.

---

## The two halves of the belonging-law

**Equational half (machine-closes):** The ten static-semantics rules (S1–S10 in the
charter) and the eleven guarantees (G1–G11). These are either enforced by the macro
at compile time or by rustc via move semantics, PhantomData, and must_use. A ladder
that violates any of them will not compile. When auditing: confirm the declaration
text satisfies each structural law, and that the guarantees are not being relied on
in ways that fall into the five explicit non-guarantees (NG1–NG5).

**Judgment half (oracle-closes):** J1 and J2. These are undecidable-χ — no machine
settles them. You must read the ladder's context and judge:

- **J1 — Tower bottom.** Does the ladder terminate where structural enforcement stops
  buying correctness gains? The floor is not at a library/our-code boundary; it is
  at the point where adding another rung would add ceremony without catching any wrong
  transition the existing type system doesn't already catch. Look at what sits below
  the lowest rung. Ask: would a ladder over *that* catch anything?

- **J2 — rung-std membership.** (Only relevant when the ladder is proposed for
  `rung-std`.) Does it recur across unrelated domains, and is the canonical statement
  better than any domain's derivation? The test: would a project that needed this
  computation be better off depending on the canonical statement than writing their
  own? If domain-specific vocabulary is load-bearing, it stays in the domain.

---

## What the register's objects are

Every `ladder!()` invocation in the codebase is a potential object of this register.
The register is not a list of files — it is a governed population of ladder
declarations. The `.gl/` charter's belonging-law ranges over those declarations.

---

## What a finding looks like

Name the law (S3, G2, J1, ...), quote the relevant declaration text, and state what
violation you see or suspect. For judgment laws, explain your reasoning — J1 and J2
require argument, not just citation.

The five non-guarantees (NG1–NG5) are explicitly out of scope. Do not file a finding
because a ladder doesn't prevent `mem::forget` or doesn't prove general forward
progress — those are known and accepted.

---

## Key files

| Path | What it is |
|---|---|
| `docs/SPEC.md` | Normative spec — the equational half. Machine-enforced. |
| `docs/JUDGMENTS.md` | Design judgments J1/J2 — the oracle half. You enforce these. |
| `docs/RUNG-CT.md` | Category-theoretic grounding — informative, not a register. |
| `.gl/schema.json` | This charter in machine-consumable form. |
| `rung/src/lib.rs` | Conformance doctests (compile_fail and compile_pass). |
| `rung/tests/` | Full conformance suite. |
