# rung principles for the review council

<!--
Read by every council reviewer from the PR's base commit, with AGENTS.md
(.github/workflows/review-council.yml calls them). The source of truth stays
rung's own AGENTS.md and docs/*-props.md; this is the reviewer-facing digest.
Only the owner changes this file.
-->

**Owner:** the maintainer, @witt3rd. A finding that contradicts a rule here
is a proposal to the owner, not a finding.

## Principles

1. **The compiler is the gate (P1).** A skipped transition, a dropped token on
   an error path, or a non-exhaustive match on verdicts must be a compile
   error, never a runtime guard. Flag any new runtime check that stands in for
   a graph property the types could carry. Flag any `compile_fail` doctest
   cited as evidence (trybuild `.stderr` pins refusals instead).
2. **The type is the evidence (P2).** Mid-ladder constructors are sealed and
   module-private (G2). Reject code that can fabricate a token, verdict or
   terminal state from outside its module, widens a constructor's visibility,
   or makes a token `Send`/`Sync`/`Clone` without the props allowing it.
   Tokens move by value and are `#[must_use]`; carry is immutable (G5);
   recover edges are paired (G7/G9).
3. **Normative documents govern (P3).** `docs/*-props.md` is law,
   `docs/*-notes.md` is derivation; props wins. Generated props and
   `docs/conformance.md` are never hand-edited (`render --check` and
   `docs/_props.py check/cited` must pass). A behaviour change that
   contradicts a proposition needs the proposition changed first, by the owner.
4. **Kernel versus product (P4).** `rung-std` admits only recurrent,
   domain-generic blocks (J2). Session catalogs, resume, isolation worktrees,
   background spawn, XDG config, memory providers and any host live in a
   product crate, never the kernel. Flag product concerns creeping into
   `rung`, `rung-macro` or `rung-std`.
5. **One-way dependency: rung knows no consumer (P5).** Rung is a general
   standalone harness. No consumer-specific names, types, ids, URLs, env vars
   or consumer-shaped assumptions in code, tests, fixtures, docs or error
   text; consumers adapt to rung through generic extension points (traits,
   ACP, MCP, args, env), never the reverse. The CI guard
   (`docs/_consumer_guard.py`, allow-list for archived history only) enforces
   the pattern list; a reviewer also catches what the pattern list misses (a
   consumer's product, class or person named in a comment or fixture). A new
   allow-list entry needs a stated reason.
6. **Generic extension points, default off (P6).** New capability arrives as
   a trait or a configured provider, off by default, selected through rung's
   own surfaces (`RUNG_*` env, `config.yaml`, CLI flag). Failures of optional
   extensions (memory, diagnostics) never fail a turn: typed outcome plus
   trace. The caller owns policy (tenancy, retention, who may see what); rung
   passes only opaque keys and bounded content.
7. **Observable contract stability (P7).** The binary name, release
   artifact, CLI flags and env, ACP surface and behaviour, typed terminal
   states and exit codes are a contract with every dependent. Changes are
   additive for hosts that ignore a new field. A refactor proves identity
   (existing tests unchanged, golden-transcript parity); it does not assert it.
8. **Secrets and safety (P8).** No credential in a committed file; providers
   name `api_key_env`, never the key. Redaction is extended through
   `RUNG_REDACT_ENVS`, never by hardcoding a consumer's variable. Never log,
   print or commit a key; ledgers hold costs, not secrets. Destructive
   operations in tooling and tests act only on named resources the change
   created.
9. **Unpublished (P9).** Rung stays unpublished: no crates.io publish, no
   announcement, no new public repo or package. New workspace crates are
   `publish = false`. A release is a GitHub annotated tag only, cut by the
   release procedure (own commit, version matches tag, CI green on the release
   commit before the tag), and only when the owner asks. Block any change that
   adds publish metadata, a publish workflow, or a public repo.
10. **Tests and CI (P10).** `cargo test --workspace` is the floor; the
    required check is `check` (fmt, clippy `-D warnings`, tests `--locked`)
    plus the propositions job. Every behaviour change has a test that fails
    without it; a claim of "live validation" says what actually ran. Fixtures
    captured from real runs are scrubbed of consumer identity and secrets
    before commit.
11. **Scope discipline (P11).** Caretaker changes stay in the routed scope;
    anything noticed outside it becomes its own backlog item, not PR prose.
    Project `AGENTS.md` additions are the owner's deliberate choice: flag an
    unrequested addition, and accept corrections of factually wrong lines.

## Rules in force

- **Reviewers invent no policy.** No new gates or rules from security or
  style instinct; never demand a runtime guard where a type would do; never
  ask for consumer-specific accommodations.

## Steward

The written law is `AGENTS.md` "Merits", "Mechanisms" (Commands, Spec and
CI, House git) and the documents its Map names; principles 3, 9, 10 and 11
are yours.

1. A PR that edits generated props or `docs/conformance.md` by hand, or
   changes a proposition's meaning without the owner (principle 3).
2. Publish metadata, a publish workflow, or a release outside the procedure
   (principle 9).

## Architect

Principles 4, 5, 6 and 7 are yours.

1. A consumer named, typed or assumed anywhere in the diff (principle 5),
   including what `docs/_consumer_guard.py`'s patterns miss.
2. A product concern in `rung`, `rung-macro` or `rung-std` (principle 4).
3. A change to the binary name, CLI flags, env, ACP surface, terminal states
   or exit codes that is not additive (principle 7).

## Inspector

Principles 1, 2 and 10 are yours: a runtime check standing in for a type,
a token that can be fabricated or duplicated, a test that would pass
without the change.

## Warden

Principle 8 is yours. Untrusted input: model output, tool calls and their
arguments, workspace files, ACP and MCP messages, and on CI anything a pull
request author controls.
