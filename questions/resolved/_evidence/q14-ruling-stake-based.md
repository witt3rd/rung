**RULING (Q14):** What provenance does a model principal carry?

**JUDGED BY:** human last-resort principal (`donald`), out of band. Written down
afterwards — `tier: attested`, in the terms of `judgments/README.md`: an
exchange a reader can audit, because nothing here can tell a faithful
transcription from an invention.

**THE VERDICT:** stake-based π. Adopted. The three defective readings are
refused.

## The question, restated in the institution

In the judgmental institution, the belonging predicate for the judgmental gate
is

$$\mathcal{P}_{\mathrm{judg}}(\varphi,a)=\bigl\{p\in\mathcal{P}\bigm|\mathsf{capable}(p,\mathsf{role}(\varphi))\ \land\ \pi(p)\cap\pi(a)=\emptyset\bigr\}.$$

The floor forces $\pi(p)\supseteq\{\mathsf{id}(p)\}$. For continuous kinds the
floor is meaningful — identity tracks stake. For discontinuous kinds
($\mathsf{Kind::Llm}$, $\mathsf{Kind::Agent}$) the floor alone does not yet
determine what $\mathsf{id}(p)$ contributes, nor what else occupies $\pi(p)$.
Q14 asks for the missing definition.

## The three readings, refused

| reading | categorical defect |
|---|---|
| per-family | π becomes essentially global; the admissible set collapses almost to empty in any corpus the family has touched. The filter is real but over-strong. |
| per-invocation | π(p) is a fresh tag each call; disjointness holds vacuously for every argument. The constant-arrow hazard in pure form. |
| per-session | π is set by an external orchestration fact, not a property of the principal; the predicate ceases to be a predicate on the principal. |

None is admissible as a definition of the provenance map for a discontinuous
principal. The per-invocation reading is the dangerous one — it passes every
check the system has while making the guarantee decorative.

## The ruling

P0 is a **stake** constraint, not a continuity-of-being constraint. Therefore:

1. For every principal `p`, the supplier declares `authored(p)`: the set of
   artifacts produced under commissions in which `p` has acted.
2. The provenance map is the floor's derived object, unchanged in form:
   $$\pi(p)=\mathsf{authored}(p)\cup\{\mathsf{id}(p)\}.$$
3. For discontinuous kinds, `id(p)` is the **family identifier** — model
   name+version, or an agent's declared composition of tools+underlying
   families. It is stable across invocations and sessions.
4. The content that carries stake is `authored(p)`. The family tag does **not**
   poison every artifact the family has touched; only those recorded in
   `authored(p)` under the relevant commission(s) do.
5. Commission boundaries are harness state. A new commission begins with an
   empty authored set for that family; prior commissions remain only if the
   supplier explicitly carries them forward.

Thus π for a model is never empty, never a pure nonce, and never the entire
historical output of the weights.

## Why it satisfies the three conditions the question required

- **Decidable at qualification time.** `authored(p)` and the family identifier
  are facts the pool possesses; disjointness is ordinary finite-set comparison.
- **Non-vacuous.** Inside a commission, a model cannot judge any artifact it (or
  another instance of the same family) produced under that commission. The
  constant-arrow reading is closed.
- **Not total.** Artifacts written under earlier, closed commissions remain open
  to later instances of the same family.

The same definition applies unchanged to `Kind::Agent`: family identifier is the
declared composition; authored set is commission-local.

## What this closes, and what it does not

**Closes:** the definitional question — what π *must mean*.

**Does not close:** the carrier. Q14 is resolved *as a definition*; it does not
supply a term the pool can yet evaluate. There is no commission log, no harness
accounting, no dynamic `authored` source in the current code — the pool reads
only the static `PrincipalSpec::authored` field. That gap is stated, not
papered over. A follow-on obligation is recorded: supply a
commission-and-contribution carrier so `authored(p)` becomes a derived fact
rather than a static declaration. Until that carrier exists, **no guessed
entries are written into `population.yaml`**, and Stage 5 stays gated for any
model whose family has already touched the argument under the only provenance
the pool can see.
