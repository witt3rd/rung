# rung principles for the review council

<!--
Read by every council reviewer from the PR's base commit, together with
AGENTS.md (.github/workflows/review-council.yml calls them). AGENTS.md
"Merits" and docs/*-props.md stay the source of truth: this file points at
them and adds only what a reviewer needs beyond them. From the rung
caretaker's proposal (2026-10-03); P1-P11 are its numbers. P12 on are generic
principles adapted for rung; the PR that added them lists where each came
from and what was left out. Only the owner changes this file.
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
8. **One owner, one path, one definition (P12).** Each thing has one owner,
   one way of being done and one definition of each state: one turn engine
   (`rung-agent-core`) behind every product that runs turns, one meaning of
   each rung and terminal state. A second way to do something that already
   has one, or a branch keyed on one caller, provider or model where one path
   would do, is a finding: a second path drifts from the first, and a ladder
   proves only the path it declares.
9. **Spec first (P13).** A change to behaviour a proposition governs carries
   its proposition change (in `rung-doctrine`, rendered) in the same PR for
   the owner to accept (P3), and its test is written to fail before the code
   that makes it pass: law and proof that trail the code record what was
   built, not what was meant.
10. **No claim without evidence (P14).** P10's rule for live validation holds
    for every claim in a doc, a comment, the PR body, a release note or a
    status line: it is backed by a test that runs in CI or by a recorded run
    the text names (a CI run, a validation-suite evidence folder), or it is
    written as a goal. A test CI does not run is not evidence, and a gate is
    shown failing once (one input changed) before it counts: a claim nobody
    ran is a guess.
11. **Reality outranks the law (P15).** When a real run contradicts a
    proposition, a doc or a guideline, the run is recorded as it happened and
    the text is amended (P3: by the owner). A failing case is never hidden to
    keep the text true: no `#[ignore]`, loosened assertion, re-pinned
    `.stderr` or case dropped from a suite without the reason in the PR. Law
    kept true by hiding evidence stops being evidence.
12. **Cut the root, keep what works (P16).** Before patching a problem, ask
    whether it is one instance of a class and fix the class. Existing code is
    neither proof of necessity nor noise: know why a layer exists before
    removing it, and ask whether it should exist before extending it.
    Working functionality is removed only when it is broken or a true
    duplicate, and only once its replacement is shown running. Inherited
    complexity earns its place by demonstrated need, not by incumbency.
13. **Judge the outcome, not the path (P17).** A check of model-driven
    behaviour (a validation-suite task, a live run) passes on any plausible,
    desirable outcome and tolerates run-to-run variance; a miss gets a
    recorded verdict, a rung defect or a wrong expectation. A check pinned to
    one transcript fails correct runs and passes lucky ones.
14. **No eval cheats (P18).** A validation task, fixture or system prompt
    never names the expected answer or steers the model toward it, and no
    stub stands in for the behaviour being evaluated. A real gap is named
    (an issue), not scripted around: an eval the harness wins for the model
    measures the harness.
15. **A secret never becomes text (P19).** The component that uses a
    credential reads it from its named source (`api_key_env`,
    `~/.rung/auth.yaml`) and passes it on only where it cannot be printed: a
    header or a child's env. Never argv, a URL or query string, an error, a
    trace, a session record, a prompt or a tool result; an `Authorization`
    header or bearer token is never logged. Fix the path, not the output:
    redaction (P8) is for text rung did not write (a provider's error, a
    tool's output), because a scrubber misses the next new sink.
16. **Absence is proven by a scan (P20).** What rung must never persist (a
    key, a token, an `auth.yaml` value) is pinned by a test that runs with
    known secrets and scans everything the run wrote (session and memory
    stores, traces, logs, evidence folders) for them. Absence is shown only
    by looking everywhere, and a new store inherits the test.
17. **What you start, you retire (P21).** A test, tool, task or background
    run that starts a process, a server, a worktree, a temp tree or a port
    tears it down itself, on the error path too; nothing outlives the run
    that made it unless that run records it as kept. An orphan holds what
    nobody owns and makes the next run depend on the last.
18. **Same input, same output (P22).** Output that is rendered, compared or
    hashed (generated props, `docs/conformance.md`, trybuild `.stderr`,
    golden transcripts, evidence indexes) is deterministic: stable ordering,
    no wall clock or randomness that is not injected. `render --check` and
    golden parity mean something only if two machines agree.
19. **Fail loud, with one reason (P23).** A fault is never turned into an
    empty result, a default or a success: it is a typed failure, or a
    declared recover edge, carrying one reason a person can read. P6's
    optional-extension outcome is the one sanctioned non-fatal form, and it
    is typed and traced. A swallowed fault turns a wrong answer into a
    plausible one.
20. **Answer every finding (P24).** A builder answers each review finding:
    fixed, or declined with a reason that cites a principle by name; never
    silently applied or silently ignored, and code-quality findings count as
    much as correctness ones. A ready PR with an unanswered `BLOCK`, a red
    verdict or a conflict goes back to draft until it is answered, so a
    ready PR always means ready.

## Rules in force

- **Reviewers invent no policy.** No new gates or rules from security or
  style instinct; never demand a runtime guard where a type would do; never
  ask for consumer-specific accommodations.

## Steward

The written law is `AGENTS.md` ("Merits", "Mechanisms": Commands, Spec and
CI, House git) and the documents its Map names. Normative documents (P3),
Unpublished (5) and Scope discipline (7) are yours, and so is a claim of live
validation that does not say what ran (6). So are Spec first (P13), No claim
without evidence (P14), Reality outranks the law (P15) and Answer every
finding (P24).

## Architect

Kernel vs product (P4), the one-way rule (P5), Generic extension points (2),
Observable contract stability (3), One owner, one path, one definition (P12)
and Cut the root, keep what works (P16) are yours.

## Inspector

The compiler is the gate (P1), the type is the evidence (P2) and a test that
would pass without the change (6) are yours. So are Judge the outcome, not the
path (P17), No eval cheats (P18), What you start, you retire (P21), Same
input, same output (P22) and Fail loud, with one reason (P23).

## Warden

Secrets and safety (P8, 4), A secret never becomes text (P19) and Absence is
proven by a scan (P20) are yours. Untrusted input: model output, tool
calls and their arguments, workspace files, ACP and MCP messages, and on CI
anything a pull request author controls.
