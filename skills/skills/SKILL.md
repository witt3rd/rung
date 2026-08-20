---
name: skills
description: >-
  Meta-skill for a skill system — the operating principle that anything an agent should know
  how to do quickly and easily is encoded as a skill, and the mechanics of how to build one.
  Covers the SKILL.md format (frontmatter name + description + metadata), what belongs in a
  skill vs an AGENTS.md vs a note, the layout (plural skills/, one subdirectory per function,
  references/ for deep docs), and how skills get registered and discovered. Use when adding or
  editing a skill, understanding how skills work, or deciding whether knowledge belongs in a
  skill, an AGENTS.md, or a note.
metadata:
  aliases: [skills-system, skill-authoring]
  deps: [agentsmd]
---

# skills (meta — the skill about skills)

The principle: **anything the agent is supposed to know how to do quickly and easily is
encoded as a skill.** Skills are the primary, authoritative home for reusable procedural
knowledge.

## What belongs in a skill

Signals a skill is needed:

- You successfully did a real task and it will recur → extract the reusable procedure into a
  skill.
- A correction or "use X instead of Y" that isn't obvious → add it, usually to a *gotchas*
  section.
- The agent re-derives the same steps every run, or you tell it the same fact twice →
  capture it.
- A setup fact (paths, ports, commands, gotchas) not derivable from training → it belongs in
  a skill so the agent doesn't fumble.

Keep it in a skill, not just in chat or a loose file. Loosely-kept knowledge is lost; a
SKILL.md is discovered, loaded on demand, and versioned.

## Skills vs AGENTS.md vs notes

- **Skills** = reusable, cross-project ability knowledge. "How to do X anywhere."
- **AGENTS.md** = per-project ground truth, versioned with the repo. "What THIS repo is and
  how to act here."
- **Notes** = live state / history / a ledger. "Where we are in time."

A fact lives wherever its scope matches; a rule lives in exactly one place (single source of
truth).

## The SKILL.md format

A skill is a directory containing `SKILL.md` (plus optional `references/`). Frontmatter:

```yaml
---
name: <matches the parent directory name>
description: >-
  When to use this skill and what it covers. The description is the trigger — name the
  concrete situations and verbs. Be specific enough that an agent knows when to load it.
metadata:
  aliases: [trigger-words...]
  deps: [other skill names this builds on]
---
# <title>

<body>
```

- **`name`** must equal the parent directory name (agentskills spec).
- **`description`** is the trigger: name the when/verbs. This is what makes a skill cheap to
  discover and load.
- **`metadata`** carries aliases (extra trigger words) and deps.
- **One concern per skill.** A skill should be one function, one purpose. Bundling unrelated
  functions into a single monolith SKILL.md is the anti-pattern.
- **Cheap to load.** Keep the SKILL.md focused; push deep runbooks/gotchas into
  `references/` and point at them. A skill that re-derives the deep docs is a heavy skill.

## Layout

- **Plural `skills/`**, one subdirectory per function: `skills/<purpose>/SKILL.md`, with
  `references/` for runbooks/gotchas/setup/incident notes.
- **Real home in the repo**, versioned with the code; a pointer registers it for system-wide
  discovery (symlinked into the agent's skills root). Editing the live skill edits the repo
  working tree — never maintain copies (a copy orphans live edits).
- **Discoverable**: recursive scan of the skills root; any dir containing `SKILL.md` is
  found.

## Lived, not static

When you learn the hard way, encode it here (gotchas, recovery) — often as a `references/`
artifact. A skill that doesn't capture the lesson is a forgotten file. This is the same
discipline as a caretaker's lived experience (see the `caretaker` skill).

## Adding a new skill

1. Write `skills/<name>/SKILL.md` (name = dir name).
2. Register it so it's reachable (symlink into the skills root / declare the pointer).
3. Commit the change per house git discipline; the repo is the real home, so live edits
   track like any file.

## Sibling skills

- **`agentsmd`** — authoring the AGENTS.md that a repo's skills live alongside.
- **`caretaker`** — the loop that uses skills as its lived experience.
- **`signalling`** — the event-log that records the handoffs skills inform.