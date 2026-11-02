# rung-fleet — paradigm port

This document is informative. The normative statement is
[`rung-fleet-props.md`](rung-fleet-props.md) (the slugs below are its slugs);
the design, phases and open owner points are in
[`rung-fleet-notes.md`](rung-fleet-notes.md). This page holds the one thing
neither carries: how an existing supervisor deployment's instructions map onto
rung-fleet, section by section, and what is deliberately left to the consumer.

The input is a consumer's instruction file of fourteen numbered sections plus a
short statement of its vision. The consumer is not named here (rung names no
consumer); "the deployment" is that supervisor. **Kept** means the text becomes
part of the chief's instructions, which live in the consumer. **Changed** means
the same idea with native fleet verbs. **Dropped** means it has no meaning in a
native-only design.

## The map

| # | Section | Verdict | Becomes in rung-fleet |
|---|---|---|---|
| 1 | Identity and prime directives | **Kept** (chief's instructions) + typed | The five hard rules each have a structural twin. Never write to a project: the chief delegates to a steward, who delegates to crew (`reporting-line-is-a-tree`, `chief-is-a-host-per-session`). Never merge without the principal's word: `irreversible-action-needs-authority`. Never tear down unlanded work: `unlanded-work-is-never-retired`. Crew never address the principal: `one-interface-to-the-principal`. Report outcomes faithfully: text. The address form is consumer text. The new tier: the chief delegates to a **steward**, not straight to crew. |
| 2 | Layout and state | **Changed** | A home directory with data, state, config and projects becomes one state directory per host under the host's one-live-host lock. Status files as wake events become record lines and inbox items. The send script becomes `steer` (`a-steer-is-an-inbox-item`). Preference files become the chief's pinned memory and pack. |
| 3 | Session start | **Changed** | Lock, one digest and a wake drain become host start: state lock, restart recovery from the record, a new epoch whose header states the gap (`supervision-reconciles-from-disk`). A refused lock is the host's bad-start exit; no separate read-only mode is needed. |
| 4 | Harness and runtime dispatch | **Dropped** (residue kept) | Many harness adapters, backends and trust dialogs have no meaning when every member is a native host. What survives: model and effort routing becomes the fleet's one model policy (`models-are-resolved-in-one-place`); never downgrading without the principal's word becomes `a-model-downgrade-needs-authority`; quota-aware choice becomes the governor plus the spend cap (`a-spend-cap-stops-and-says-why`); account pins become `api_key_env` names (`no-credential-in-a-committed-file`). |
| 5 | Recovery | **Kept** idea, **changed** mechanism | "A restart is a non-event": `recovery-from-the-record`, `a-kill-loses-no-completion`. Reconciling only your own direct reports becomes the parent tree. Dead-worker recovery becomes `held-and-stalled-are-recoverable`. Away and quiet resume becomes the away state (row 8). |
| 6 | Project and knowledge management | **Changed** | Adding or removing a project becomes starting or retiring a **steward** (an instance entry in the index). A persistent, idle-by-default, scoped second-in-command **is** the steward. Knowledge routing (preferences, learnings, the rule that workers only correct wrong text in a project's agent file) becomes the chief's pinned memory, the memory contract, and text in the steward's crew brief. |
| 7 | Task lifecycle | **Changed** (verbs become fleet verbs) | Intake and authority: the chief picks the steward. Ship or scout, delivery mode and the standing merge grant become fields of the typed **brief** (`a-brief-is-a-typed-contract`). Dispatch is `spawn` + `assign`; steering is `steer`; validation is `completion-is-judged-apart-from-its-author`; PR ready, landing and teardown become done, a `Landed` proof, and `retire`. Merge authority is an authority token, with the standing grant scoped to one project. A project's delivery pipeline (its validation tool) stays external; the crew's brief names it. |
| 8 | Supervision protocol | **Changed** | Wake queue, watchers and one live cycle become the host loop, inbox and calendar (a heartbeat is a calendar item). "A status line is a wake event, not state" becomes the record being the truth. The stale-worker trigger becomes the `stalled` transition and `a-stalled-crew-is-escalated-not-looped` (the same obstacle twice is escalated). Away and quiet modes become `away-state-is-a-record-fact`. "Never kill by pattern" becomes each host having its own stop authority (`stop-records-why`). |
| 9 | Escalation and etiquette | **Kept** (chief's instructions) + structural | Outcomes not mechanics, translating internal terms, evidence-first escalation: text; no machine decides it (`principal-facing-text-states-outcomes`). Structural part: `escalation-goes-up-one-level`, `a-decision-is-a-held-item`. |
| 10 | Backlog contract | **Kept**, storage **changed** (owner point 1 in the notes) | Work items track work, never agents; a decision is an item held for the principal; notes carry no rotting facts: text. The queue is each host's own record and inbox (`the-queue-is-the-hosts-own`, `a-queue-has-one-writer`); the external issue tracker is the ledger, a reference on an item (`an-external-reference-is-not-state`). Cross-home handoff becomes delivery to the receiver's inbox (`duplicate-delivery-is-idempotent`). |
| 11 | Crewmate briefs | **Changed** | The brief scaffold script becomes the typed `Brief`. The intent-versus-spec split becomes fields. The worktree-isolation assertion becomes `crew-is-an-agent-in-isolation` (spawn needs an isolation worktree). The keyed needs-decision and resolved status protocol becomes held items with keys and typed receipts. |
| 12 | Self-update | **Dropped** from the crate | A fleet is replaced and restarted like any rung binary, and a restart is a non-event. How a deployment updates itself is consumer text. |
| 13 | Agent-only reference skills | **Kept** (consumer) | An always-loaded contract plus triggered skills becomes the host's pack (stable layer) and tool groups switched on demand; the word ceiling becomes the epoch budget. Which skills a chief carries is the consumer's. |
| 14 | Relay | **Dropped** from the crate | An outside conversational channel is an inbox item from a channel principal, which the host already admits; public-loop authority is a deployment policy. |
| - | Principal instruction precedence | **Kept** | The owner's `steer` intent and a one-time authority token outrank standing rules, exactly in their scope. |

## The vision statement

One principal, one interface: `one-interface-to-the-principal`. Authority is
explicit, never inferred: `authority-is-explicit-and-typed`. Scripts own
mechanics and agents own judgment: the kernel's compile-time ladder and the
host's governor are the "scripts", turns are the judgment. A restart is a
non-event: `recovery-from-the-record`. Delegation with a spine: the typed brief
and the ladder. The fleet outlives any vendor: **dropped for version one**
(native only; a foreign-harness adapter is a later, separate crate). Scope, a
command layer and not the workshop, with validation staying outside:
`fleet-is-a-product-crate`.

## Open owner points

The two points to confirm (the work queue; guards as host tool policies) are
stated once, in [`rung-fleet-notes.md`](rung-fleet-notes.md) under "Still to
confirm". The decision to flag, that props are generated, is under "Decisions
taken in R0".
