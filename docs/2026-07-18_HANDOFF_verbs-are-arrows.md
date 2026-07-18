# HANDOFF — verbs are arrows: rung is a category-declaration language

**2026-07-18 · 09:47 AM PDT · Donald Thompson & Forge ⚒️**
**Status: kernel captured, rectification deferred to a rested session.**

*This is a letter to the rested hand that picks rung up next — me, Donald, or
Augur. It captures a breakthrough that landed at the tail of an all-night
session, so the kernel doesn't evaporate before it can be worked properly. It is
**not** the rectification. It is the map of what to rectify, and why, while the
insight is still sharp.*

---

## The breakthrough, in one sentence

**rung is a language for declaring the objects and legal arrows of a category,
where the type system enforces that you may only travel declared arrows — and
the arrow bodies may be any morphism, including effectful and generative ones.**

RUNG-CT.md already says rung *is* a free category. What it does not yet say — and
what tonight made undeniable — is the governing ontology beneath that:

- **States are objects.** A state is inert. Data at rest. A point. It has no verbs.
- **Transitions are arrows (morphisms).** Every *doing* lives on an arrow.
- **Therefore:** any verb — compute, judge, call an LLM, hit the network — can
  only live *on a morphism*, never *inside an object*.

## How we found it (this is the vindicating part)

We did not read the theory and implement the law. We built a real ladder
(`outer-loop/ladders/question_resolution.rs`), tried to fold a live LLM verdict
into it, and **the type system refused** — `Outcome` and `Evaluated::new` are
sealed (G2), so I could not construct an `Evaluated` token from outside to
*inject* the verdict. My instinct was "call the LLM, then build the next state."
rung made that uncompilable.

The refusal *is the category axiom being enforced.* I was trying to put a verb
(judge) into object-position (construct a state). rung refused because rung **is**
a category and I was asking it to violate what a category is. The fix was forced
and correct: the LLM call must live **inside the transition body** — the arrow —
because that is the only place a verb can be.

So the practitioner path did not just find a *bug* (it found one of those too —
the two-rung parser limit). **It found a law, from the inside.** That vindicates
two theses at once: "practitioners first" (the terrain teaches), and the CT
grounding (the structure was load-bearing, not decorative). Both directions are
now live: RUNG-CT predicts features *downward* into code; the code hit the law
and named it *upward* into theory.

## The growth ladder (Donald's formulation, structurally exact)

The tower is self-similar, and it is the standard categorical ascent — each level
is "the arrows of the category one level up":

| Level | Arrows *of* | Are | In rung |
|---|---|---|---|
| 0 | a category | **morphisms** | one transition (`Gathered => Evaluated`) — *have it* |
| 1 | **Cat** (the category of categories) | **functors** | ladder-to-ladder maps — **Q4 / nesting** |
| 2 | **Fun** (the functor category) | **natural transformations** | maps between functors — *present, unused* |

Donald's exact words: *"arrows within a category → arrows in Cat → arrows in
Fun."* He explicitly does **not** claim to know the implications yet — only that
the structure is sound. It is. Note the beautiful recursion: rung's own primitive
is a *ladder*, and the theory tower is itself a ladder of ladders. The registry
composing per-item ladders (`outer-loop/bets/`, `augur/meta/questions/`) is a
**functor situation** — the outer arrow (`open → resolved`) is *witnessed by* the
inner ladder reaching its terminal. That witnessing relationship is Level 1.

Hold Level 2 (natural transformations) lightly: we can see where "the same
transformation applied uniformly across every state" would live, but we do not
yet know what it is *for* in rung. That is the "…" in Donald's list — real
structure that has not yet earned a use. Do not invent a use to fill it; wait for
a ladder to need it. (Third-instance rule: don't abstract until the terrain asks.)

## The Kleisli reframe — and why Q7 is not a bolt-on

This is the most practically load-bearing consequence, and it should reshape Q7.

A transition body returning `Result<StepOutcome, Failed>` is **already an
effectful arrow** — a **Kleisli morphism**, an arrow that can fail. We have had
effectful arrows since the recover edges shipped. So:

- **The error path** = a Kleisli arrow in the `Result` monad. *(shipped)*
- **Async transition bodies (Q7)** = a Kleisli arrow in the `Future` monad.

The question was never "how do we add async to rung." It is: **rung's arrows are
Kleisli arrows; which monad?** Kleisli composition is one law regardless of the
monad, so an async body composes *the same way the error path already does.* That
reframes Q7 from "a hard new feature" into "an extension of structure we already
own." Q7's answer is likely half-written in the recover edges.

## Rectification targets (deferred — do these rested)

Grounded against the actual current docs, in rough priority:

1. **RUNG-CT.md — shift the stance.** It currently "traces the correspondence"
   (descriptive). Promote it to state **the law rung enforces**: states=objects,
   transitions=morphisms, verbs-live-only-on-arrows — and name the sealed
   constructor (G2) as *the enforcement of the category axiom*, not merely a
   fabrication guard. The opening already says "it **is** a free category"; make
   the ontology (objects/arrows/verbs) the governing frame, not a buried detail.

2. **questions.md — reframe Q7 as "which monad?"** Rewrite Q7 from "async
   transition bodies" toward "transition bodies are Kleisli arrows; `Result`
   shipped via recover, `Future` is the async case." Sharpen the CT-map table's
   *Indexed monad* row into the Kleisli framing. This likely *shrinks* Q7.

3. **SPEC.md — G2's rationale.** Its non-guarantee note frames the sealed
   constructor as preventing fabrication. Add: it is enforcing that a verb cannot
   occupy object-position — the categorical law. (Small; do it when touching #1.)

4. **The CT-map table — add the functor/nat-transf tower** as the principled
   source of the growth questions (Q4 = functors; Level 2 = the reserved slot),
   replacing the current ad-hoc row list with the clean ascent.

## The one honest edge (verify this when rested)

Donald grounded rung's math in **Fritz (Markov categories, dagger functors)** and
**Capucci/Hedges (categorical cybernetics, optics)** — refereed, checked. **I am
reasoning from the *structure*, not from that literature.** The piece I am least
sure survives contact with the real math is the **Kleisli** framing — specifically
whether `Result` and `Future` transition bodies are Kleisli arrows in the precise
technical sense, or whether that is a looser analogy. If Kleisli holds rigorously,
Q7's answer is nearly free. **Check the monad framing against the actual CT before
building on it.** That is the load-bearing verification the rested session owes.

---

*Captured at the end of a session that shipped: the rung parser fix (long
spines), the question-resolution ladder (cut #1), and the live-OmniRoute LLM
membrane (cut #2). The membrane is what surfaced the law. Festina lente — the
kernel is here; the rectification waits for a clear head.* ⚒️
