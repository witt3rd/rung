# The theory of theories in context — the catalog, the router, and the backlog

**Status: informative, and deliberately evolving.** This is the design capture
of an idea that emerged while auditing rung's own questions and wanting to move
subjects *between* theories' carriers. It is **not yet normative** — treat it as
a working note that will be formalized as it is built out. The governing laws
today are the `*-props.md` documents; this note describes a second-order shape
they do not yet fully name.

## The problem this answers

An audit-rectify pass over one **instance** (a theory bound to a carrier) can
find a subject that is *not really a subject of that theory*. Auditing rung's
questions with the **Questions** theory repeatedly found questions that fail
the **authentic cut** — *"a work item / a design decision, not a question whose
answer the structure finds."* Such a subject is not badly formed in some
abstract sense; it is **mis-homed**: it belongs to a different instance, one
whose law fits its kind (an issue, a task, a backlog item).

But moving it requires knowing *what the other instances are and what each one
admits*. That knowledge is itself governable. This note is the shape of that
governance: a **theory of theories in context** — the theory of the collection
of available instances, their carriers, and how subjects are routed between
them.

## 1 · The catalog

The set of available instances in a context is itself a **Het theory**. Its
subjects are **[`Instance`]s** — each a theory bound to a carrier (plus a
population and a state home, per Q18's `config.yaml`). For rung itself the
catalog includes:

```text
{ Questions @ rung's question docket,
  Het       @ the institution itself,
  Issues    @ the project's GitHub (generic issues theory),
  Principals@ the population,
  Backlog   @ the homeless-subject set, ... }
```

The catalog's **sentences** audit the collection:

- every instance's carrier exists and is walked;
- every theory has at least one lived instance (the lived-instance discipline,
  one level up — a theory with one carrier is a domain model);
- the routing set is **complete**: for every ejection rationale, some instance
  is a candidate destination.

The catalog's **edits** are the higher-order moves a subject-routing endpoint
needs: **admit** an instance, **evict** an instance, and **route** a subject to
an instance. This is what lets a pass say *"there is an Issues instance, it
fits, route it there"* instead of guessing.

## 2 · The router — cross-theory rectification

When instance A's audit-rectify decides to **discharge** a subject (e.g.
"not well-posed — this is a work item"), the **router** — the part of the
catalog that matches rationale to destinations — selects a target instance by
the ejection rationale and each instance's description, and attempts to route
the subject there. The target's own **intake** then re-audits the subject under
*its* law before admitting it.

This is **cross-theory rectification**: A's discharge + B's intake = a
**relegation** of the subject across two instances, under two different
theories, mediated by the catalog. The same project's *question* becomes an
*issue* — re-homed, not re-brained.

Concretely (the run that motivated this): the Questions instance audited rung's
docket, the real judge refused most of them on the authentic cut, and the
correct destination is an **Issues instance of the same project (rung's
GitHub)** — a relegation from the questions carrier to the issues carrier.

## 3 · Intake / discharge

These are **generic operations**, not new per-theory machinery — they are the
atomic building blocks every carrier already has, re-driven:

- **INTAKE** — `admit(C, subject)`: the candidate subject must first pass that
  theory's **audit** (its membership / well-posedness screen — a gate), then it
  is added to the carrier set. *Intake is gated on membership.*
- **DISCHARGE** — `remove(C, subject)`: the subject is taken out of the carrier
  set.

One generic `Intake`/`Discharge` driver serves any theory's carrier. Admission
is always a re-audit under the *destination* theory's law — the source theory
may say "not a question," but only the Issuests theory can say whether it is a
well-formed *issue*.

## 4 · The backlog

When no destination instance is appropriate, the discharged subject goes to the
**backlog** — an instance whose carrier holds subjects with no other home, their
fate undecided. The backlog is never empty-on-principle and never assumed
resolved: it is the honest holding place for *"we don't yet know where this
belongs, or whether it belongs anywhere."*

## 5 · Invention

The backlog has its own audit-rectify, and **that** is where new instances and
new theories are born through the loop, rather than by a hand-wave:

- **discard forever** — the subject was waste;
- **admit a new instance** of an existing theory — *"rung should have Issues"*
  admits an Issues instance over rung's GitHub;
- **invent a new theory** — *"nothing we have captures this kind of thing"*
  creates one.

So the catalog is not fixed: the loop can **extend the institution itself**.
New theories and carriers arise *from* an audit-rectify of the backlog, which is
the deepest sense of self-hosting — rung governing which institutions rung has,
and growing them when the routing says so.

## Open questions (to evolve before formalizing)

- **Admission under the target theory** must be gated on *its* membership — but
  does the router pre-filter by rationale, or does every candidate go through
  intake's audit and simply fail most? (Likely: pre-filter by rationale, then
  intake audits.)
- **Provenance / non-identity across the boundary.** A subject authored in the
  questions instance becomes a subject in the issues instance — does the π
  travel, and who may now judge/author it there?
- **The catalog's own well-posedness.** Is the catalog theory itself auditable
  by a *further* catalog (the tower), or does it bottom out recursively?
- **Reversibility.** A relegated subject that fails the target's intake is
  not lost — it returns to the source or to the backlog, not to nowhere.

## Relationship to what exists

- `[Instance]` (Q18): the thing the catalog holds as its subject.
- `rung_driver::carrier::GitHubIssuesCarrier`: the carrier of an Issues
  instance; already present.
- the audit-rectify cycle / `Pass`: what produces the discharge verdict.
- the patch-based writeback (enact to carrier): what persists a re-homed
  subject.

This note will evolve; formalize it into a normative proposition only when the
router, intake, and catalog theory have real instances proving the shape.
