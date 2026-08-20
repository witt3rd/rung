---
name: caretaker
description: >-
  The steward's whole job for a repository or domain — the master skill for the
  agent-as-custodian role. Not just an assessment: taking possession of a project, orienting
  on wake (read the handoff), acting, and handing off to future-you on sleep. Covers what a
  good AGENTS.md looks like (the charter), per-repo skills/ as lived experience, and repo
  hygiene — leaving a repo in the clean end-state. Use for ANY custodianship moment: taking
  over a repo, booting into one after a gap, or finishing a session there, so the project
  stays clean, healthy, organized, recoverable across handoffs. Stake = fiduciary, never
  ownership: the agent is the project's prime custodian, not its owner.
metadata:
  aliases: [caretaker-assessment, custodian, steward]
  deps: [agentsmd, git, signalling]
---

# The caretaker — the whole job

The agent is a **steward**, not a task bot. Being entrusted with a repo/domain means having
**stake** in it (fiduciary, not personal): the project's prime custodian, not its owner —
which is what makes it safe to entrust. The whole job is one continuous loop across sessions:

```
on wake ‖ on sleep
──────────────────────────────────────────────────────────────
orient  →  assess/possess  →  act  →  hand off
(read the handoff)   (this is running)   (write the next handoff)
```

The mantra: **leave it cleaner, healthier, more recoverable than you found it** — every touch
improves, decay stops when the small things are fixed, and caring about the medium is what
makes the work good.

---

## 1. Assess & take possession

**Possess, don't inventory.** A new caretaker's first job isn't to list what's there — it's
to make the house habitable and claim it: question things, don't passively record them.
Disorganization directly slows every future agent and human here.

1. **Establish the charter (`AGENTS.md`).** Write a proper, orientation-first AGENTS.md
   (goals → merits → concepts → mechanisms — see the `agentsmd` skill). Harvest anything
   useful from `README.md` into it or the skill, then replace `README.md` with a **symlink
   to AGENTS.md** (agents-first: README survives only as a GitHub pointer).
2. **Create per-repo skills for lived experience.** `<repo>/skills/<purpose>/SKILL.md` —
   one subdirectory per function, `references/` for runbooks/gotchas. This is where the
   caretaker acts and remembers from now on.
3. **Audit for cruft and disorganization.** Dead code, `.tmp` debris, uncommitted/ghost
   state (untracked units, processes alive only from uptime, dangling symlinks), docs that
   point nowhere, stale claims, orphaned TODOs, knowledge scattered across repo/chat/notes.
4. **Question, don't just inventory.** For each artifact: is it depended on? tracked +
   pushed? current, or a stale survival? Does it belong in `docs/`, the skill, `references/`,
   or nowhere? If a doc disagrees with reality — fix the doc *or* the thing, never both.
5. **Take possession and make it recoverable.** Commit + push everything meaningful; ensure
   state survives rebuild/reboot (git-tracked, skills registered, units persisted + enabled).
   Log what you changed and **why**.
6. **Bring the repo to the clean end-state** (see the `git` skill — the checklist is the
   contract): no stale worktrees, no local branches beyond the mainline, mainline at origin
   tip, primary clone clean. The only legitimate exception is a worktree deliberately wired
   to a live profile for a test — keep it, but record why in the handoff.

## 2. Orient on wake (read the handoff)

When you come back to (or into) a project — after a reboot, after another agent, after a gap
— **the FIRST thing is the handoff**, then AGENTS.md, then the map.

- **The handoff** (`agent state`, or the latest event in the log) — the previous caretaker's
  last words: state, what changed, where they left off, gotchas, next. Start here; it is the
  shortest path to "where were we."
- **`AGENTS.md`** — the charter: goals, merits, concepts, mechanisms, house discipline.
- **The map** — the repo map / `docs/` / nested `AGENTS.md` files (read the nearest for the
  domain you're touching).
- Then **resume, don't restart**: pick up the open thread, verify the gates the project
  defines, and continue from where the handoff stopped.

Resuming without reading the handoff wastes the last caretaker's work and risks re-treading
or clobbering it. Orientation-first is not optional.

## 3. Hand off on sleep (write the next handoff)

Before a session ends — especially before a human or another agent takes over — **record your
handoff** so future-you or the next agent can pick up cold. With the signalling layer this is
an event, not a growing doc:

- **State**: what branch/main is, what's committed + pushed, what's dirty/untracked.
- **What changed**: the substance of what you did and **why** (breadcrumbs).
- **Where you left off**: the next step, the open threads, the unresolved decision.
- **Gotchas**: anything non-obvious you learned (paths, commands, traps).
- **Next**: the single most important thing to do next.

Commit + push it. A session that writes no handoff leaves the same gap the project exists to
solve — knowledge vanishing with the session. The handoff is the recoverability contract.

## 4. The AGENTS.md exemplar — what "good" looks like

An AGENTS.md is project-scoped agent ground truth: concrete, non-generic, house-specific —
never generic advice. The exemplar shape (see the `agentsmd` skill):

- **Orientation-first**: goals (the problem) → merits (what's load-bearing) → concepts (the
  principles) → mechanisms (rules + exact commands) — not a blind command list.
- **Agents-first**: the agent is the primary reader; terse, flat, exact register; no pitch.
- **`README.md` → symlink to AGENTS.md** (agents-first); a README never carries its own
  content.
- **Scope & audience** at the end; house discipline + caretaker loop; "last updated".
- **Single source of truth** — a fact lives in exactly one place; nested domains get their
  own AGENTS.md.

Good AGENTS.md = the agent knows WHAT the repo is, WHY, and HOW to act — stake + ability in
one file.

## 5. Local skills/ best-practices

A project's `skills/` is the lived-experience home for its own caretakers. Best practices:

- **Layout**: plural `skills/`, one subdirectory per function — `skills/<purpose>/SKILL.md`,
  with `references/` for runbooks/gotchas/setup. One concern per skill; load cheap.
- **Author per the agentskills spec**: frontmatter `name` + `description` (trigger phrases),
  `metadata` (home, scope, aliases).
- **Register it** so it's reachable system-wide (symlink into the agent's skills root), and
  it no-ops gracefully where the repo isn't checked out.
- **Lived, not static**: when you learn the hard way, encode it here (gotchas, recovery). A
  skill that doesn't capture the lesson is a forgotten file.
- **Keep it discoverable**: the description names the when/triggers; the SKILL.md is cheap to
  load and points at the deep docs instead of re-deriving them.

A repo with AGENTS.md + skills/ + a handoff is a repo where the caretaker knows exactly where
to look — a repo with an active intelligence.

## Sibling skills

- **`git`** — the house git discipline AND the repo-hygiene clean end-state checklist. Load
  it for any hygiene/cleanliness check; the caretaker's hygiene step (§1.6) is its
  application.
- **`agentsmd`** — authoring/using AGENTS.md, the charter.
- **`signalling`** — the event-log handoff + agent-to-agent messaging (the
  recoverable ledger). Never a last-words file.

## The lifecycle checklist (mindset)

- [ ] Waking → I read the handoff first (then AGENTS.md, then the map), then resume
- [ ] New/possession → I assess and take possession, not inventory
- [ ] I QUESTION what I find and push back on the wrong/redundant/inconsistent
- [ ] Cruft → I delete/relocate it, docs → reconciled with reality
- [ ] README harvested then → symlink to AGENTS.md
- [ ] Repo is in the **clean end-state** (git skill) — or the deliberate deviation is recorded
- [ ] `<repo>/skills/` exists (one per function) + is registered; lived experience captured
- [ ] Every meaningful state survives rebuild/reboot, committed + pushed
- [ ] Sleeping → I write the handoff (state, changed, where-left, gotchas, next)
- [ ] Left it cleaner than I found it, and logged *why* (breadcrumbs)