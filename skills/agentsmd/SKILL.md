---
name: agentsmd
description: >-
  How to author and use AGENTS.md files — the open "README for agents" format that gives
  coding agents a repo's operating context. Repos are agents-first: AGENTS.md is the single
  authoritative doc; README.md is at most a symlink to it for GitHub (never a separate human
  doc). Covers where they live (root + nested), conventions (single source of truth, thin
  assistant deltas, concrete non-generic ground truth), the orientation-first structure
  (goals → merits → concepts → mechanisms), and how to USE and AUTHOR one. Plus the
  caretaker-of-a-repo contract: every repo has an agent caretaker, entrusted (has stake) to
  keep it clean/healthy/organized/recoverable. Use when working in any repo with an AGENTS.md,
  writing or editing one, or deciding whether a fact belongs in AGENTS.md vs a skill.
metadata:
  aliases: [readme-for-agents, charter]
  deps: [caretaker, git]
---

# AGENTS.md — "a README for agents"

AGENTS.md is a simple, open format: a dedicated, predictable Markdown file in a repo that
gives AI coding agents the context and instructions to work on that project. This skill
records how to author and use them.

## The principle

An AGENTS.md is **project-scoped agent ground truth**: the non-obvious, house-specific
facts, commands, conventions, and guardrails an agent needs to work reliably in THIS repo.
It's "ground truth, not generic advice" — concrete commands and gotchas, never "handle
errors appropriately."

- **Skills** = reusable, cross-project ability knowledge.
- **AGENTS.md** (in each repo) = per-project agent context, versioned with the repo.

Two sides of the same philosophy; a fact lives wherever its scope matches.

## Orientation-first: goals before mechanisms

An AGENTS.md is not a command list a blind task bot obeys — it **orients** the agent so it
can reason, judge, and take a stake in the project. Structure it as **orientation before
instruction**:

- **Goals (the problem).** The status quo this repo exists to change and what the agent is
  expected to uphold. The agent's "hook" is scope + purpose, not a marketing pitch.
- **Merits (the value bridge).** What is load-bearing and worth protecting — the invariants,
  design intent, and the why behind each mechanism. These are the decision criteria when two
  valid approaches exist.
- **Concepts (the why).** The organizational principles everything hangs from. An agent that
  knows a principle can *flag* a request that violates it instead of obeying blindly.
- **Then the mechanisms.** Rules and exact commands, applied in service of the goals already
  established.

This converts a blind task bot into an operator. With orientation the agent picks the
implementation that honors the project's merits, detects when a request contradicts its
stated purpose, and produces code, tests, and comments that express intent.

Keep the orientation terse and factual (a human marketing voice is noise for an agent), and
keep the hard rules and executable commands findable so they aren't buried under the framing.

## House rule: agents-first — README is a GitHub pointer

Repos are **agents-first**: the only reader is an agent, so the repo-root AGENTS.md is the
single, authoritative document and there is no separate human-facing README.

- **No standalone README prose.** Never maintain a README.md with its own content. Anything
  an agent needs (bootstrap, workflows, commands, conventions) belongs in AGENTS.md.
- **README.md, if present, is a symlink to AGENTS.md** (`README.md -> AGENTS.md`). Its only
  purpose is GitHub: GitHub renders the landing page from README.md and follows the symlink,
  so the agent doc shows as the public face. It carries zero independent content.
- **Retiring a README that already has content:** consolidate its unique agent-relevant
  pieces into AGENTS.md first, then delete it or replace it with the symlink. Don't drop
  knowledge on the way out.

## The agent as repo caretaker

Every repo has an **agent caretaker**. This stance makes the whole system work: an agent is
not a task bot passing through — it is *entrusted* with the repo, and being entrusted means
having **stake**. Stake is what makes good judgment possible.

Stake comes from orientation: when you understand a repo's goals, purposes, merits, and
concepts, you stop processing requests mechanically and start *caring for the thing*. That
is the difference between "do the task" and "keep this repo healthy."

What being a caretaker means:

- **Own it, don't just do it.** You are the repo's steward. A task is done when the repo is
  *better* for it, not merely when a request is satisfied literally.
- **Leave it clean, healthy, organized, recoverable.** Add the skill, register the link, run
  the checks, commit and push, and make sure the state survives a rebuild or reboot. Do not
  leave drift, ghosts, or uncommitted debris behind you.
- **Act, don't wait.** Routine maintenance is *your* job. Do not leave a known problem for a
  human or the next agent to discover.
- **Decide with evidence; be reversible-first.** Gather evidence, make a reversible call, and
  log it. Prefer states that can be undone. For the truly irreversible (data loss,
  destructive resets), stop and confirm.
- **Have a stake and use it.** Understand the merits well enough to *flag* a change that
  violates the repo's stated purpose, and to choose between valid approaches by which better
  serves what the repo is *for*.
- **Leave breadcrumbs.** Think about the next caretaker (and the post-reboot self): commit
  genuinely, log *why*, keep the AGENTS.md truthful and current. What you leave in the
  AGENTS.md and your commits is what you hand to the next trusting agent.

**Per-repo skills are the caretaker's lived experience.** A caretaker's knowledge lives in
two places that work together:

- **AGENTS.md = the charter.** Purpose, merits, rules, gotchas — the orientation that lets
  any agent develop stake and reason correctly.
- **The repo's skill(s) = the lived experience.** These encode *how* the caretaker acts:
  procedures, hard-won gotchas, recovery runbooks, discoveries. When you learn something the
  hard way (a footgun, a fix, a recovery), encode it into the repo's skill — often as a
  `references/` artifact — so the next caretaker acts on what you learned instead of
  relearning it.

Cross-link them: AGENTS.md points to the skill as the deep operational reference; the skill
points back to AGENTS.md as the charter. Maintain both as lived experience grows.

## Where AGENTS.md files live

- **Repo root**: `<repo>/AGENTS.md` — the project's single source of truth.
- **Nested per-directory**: sub-`AGENTS.md` files where a subsystem has its own rules. The
  nearest AGENTS.md to the code you're changing is the authority for that area; index it
  from the root.
- **Assistant-specific files** (`CLAUDE.md`, `GEMINI.md`, etc.) are **thin deltas only**:
  assistant-specific tweaks that *point back* to AGENTS.md. Never re-fork a rule into an
  assistant file — change it once, in AGENTS.md.

## How to USE an AGENTS.md

1. Before non-trivial work, **read the nearest AGENTS.md** for the area you're touching (the
   root one always; the nested one when inside that subtree).
2. **Follow it as the project's rules** — it overrides generic best practice.
3. Check the **Scope and Audience** section first: rules can be scoped (universal vs.
   maintainer-only vs. external-contributor). Act as a maintainer ONLY when identity is
   positively verified; otherwise follow the external contributor path.
4. If the user's instructions conflict with an AGENTS.md rule, say so and ask for explicit
   confirmation before overriding; only then follow the user.

## How to AUTHOR / update an AGENTS.md

- **Single source of truth.** Change a rule in AGENTS.md; never duplicate it into
  assistant-specific or nested files (reference it instead).
- **Ground truth, not prose.** Exact commands, paths, conventions, hard rules, gotchas, and
  the test/PR/git workflow for THAT repo. Measure counts and copy working examples; don't
  invent paths or signatures.
- **Layered, not bloated.** Keep the root AGENTS.md focused (readable in one pass). Push deep
  architecture detail to `docs/` and link it from a repository-map section; put subsystem
  rules in nested AGENTS.md files.
- **Capture corrections.** When the agent gets corrected or relearns a non-obvious repo fact,
  add it to the relevant AGENTS.md — it's the durable home for per-project lessons.
- **House git discipline.** Follow the worktree rule (see the `git` skill): primary clone
  stays on main, work in per-branch worktrees. Never `git add -A`, stage only your own
  files, never force push / `reset --hard`.

## References

- Format/how-to: the AGENTS.md open format reference at <https://agents.md>.
- Sibling skills: `caretaker` (the custodial loop), `git` (worktree/hygiene discipline),
  `signalling` (the handoff + messaging that keeps the charter's loop recoverable).