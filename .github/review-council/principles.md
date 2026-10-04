# rung principles for the review council

<!--
Read by every council reviewer from the PR's base commit, together with
AGENTS.md (.github/workflows/review-council.yml calls them). AGENTS.md
"Merits" and docs/*-props.md stay the source of truth: this file points at
them and adds only what a reviewer needs beyond them. From the rung
caretaker's proposal (2026-10-03); P1-P11 are its numbers. Only the owner
changes this file.
-->

**Owner:** the maintainer, @witt3rd. A finding that contradicts a rule here
is a proposal to the owner, not a finding.

## Principles

The bold bullets of `AGENTS.md` "Merits" are rung's principles; cite them by
their bold names there: **The compiler is the gate** (P1), **The type is the
evidence** and **Linear consumption** (P2), **Normative documents govern**
(P3), **Kernel vs product** (P4), **No credential in a committed file** (P8)
and the one-way rule that rung takes no dependency on any consumer (P5).
This file adds:

1. **What the merits mean for a reviewer (P1-P5).** A new runtime check that
   stands in for a graph property the types could carry; a `compile_fail`
   doctest cited as evidence (trybuild `.stderr` pins refusals); code that can
   fabricate a token, verdict or terminal state from outside its module, a
   widened constructor, or a token made `Send`, `Sync` or `Clone` without the
   props allowing it; a hand edit to generated props or `docs/conformance.md`;
   a behaviour change that contradicts a proposition before the owner changes
   it; a product concern in `rung`, `rung-macro` or `rung-std`; a consumer's
   product, class or person named anywhere, including what
   `docs/_consumer_guard.py`'s patterns miss (a new allow-list entry needs a
   stated reason).
2. **Generic extension points, default off (P6).** New capability arrives as
   a trait or a configured provider, off by default, selected through rung's
   own surfaces (`RUNG_*` env, `config.yaml`, a CLI flag). A failure of an
   optional extension (memory, diagnostics) never fails a turn: a typed
   outcome plus a trace. The caller owns policy (tenancy, retention, who may
   see what); rung passes only opaque keys and bounded content.
3. **Observable contract stability (P7).** The binary name, release
   artifact, CLI flags and env, the ACP surface and behaviour, typed terminal
   states and exit codes are a contract with every dependent. Changes are
   additive for hosts that ignore a new field. A refactor proves identity
   (existing tests unchanged, golden-transcript parity); it does not assert it.
4. **Secrets and safety (P8).** Redaction is extended through
   `RUNG_REDACT_ENVS`, never by hardcoding a consumer's variable. A key is
   never logged, printed or committed; ledgers hold costs, not secrets.
   Destructive operations in tooling and tests act only on named resources
   the change created.
5. **Unpublished (P9).** No crates.io publish, no announcement, no new public
   repo or package; new workspace crates are `publish = false`. A release is a
   GitHub annotated tag only, cut by the release procedure (`AGENTS.md`
   "Commands"; own commit, version matches tag, CI green on the release
   commit before the tag), and only when the owner asks.
6. **Tests and CI (P10).** Every behaviour change has a test that fails
   without it. A claim of "live validation" says what actually ran. Fixtures
   captured from real runs are scrubbed of consumer identity and secrets
   before commit.
7. **Scope discipline (P11).** A caretaker change stays in its routed scope;
   anything noticed outside it becomes its own backlog item, not PR prose. An
   `AGENTS.md` addition is the owner's deliberate choice: an unrequested
   addition is a finding, a correction of a factually wrong line is not.

## Rules in force

- **Reviewers invent no policy.** No new gates or rules from security or
  style instinct; never demand a runtime guard where a type would do; never
  ask for consumer-specific accommodations.

## Steward

The written law is `AGENTS.md` ("Merits", "Mechanisms": Commands, Spec and
CI, House git) and the documents its Map names. Normative documents (P3),
Unpublished (5) and Scope discipline (7) are yours, and so is a claim of live
validation that does not say what ran (6).

## Architect

Kernel vs product (P4), the one-way rule (P5), Generic extension points (2)
and Observable contract stability (3) are yours.

## Inspector

The compiler is the gate (P1), the type is the evidence (P2) and a test that
would pass without the change (6) are yours.

## Warden

Secrets and safety (P8, 4) are yours. Untrusted input: model output, tool
calls and their arguments, workspace files, ACP and MCP messages, and on CI
anything a pull request author controls.
