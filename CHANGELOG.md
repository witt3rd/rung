# Changelog

All notable changes to rung, from git history and the GitHub releases.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Numbers in parentheses are pull requests. Rung is unpublished; versions are
git tags only.

## [Unreleased]

## [0.2.3] - 2026-10-07

Patch bump: backward compatible changes; memory timeouts are withdrawn from
the 3 s gate.

### Changed
- Memory timeouts: the recall hook default is 30 s (#254, replacing the 3 s
  provider default of #240) and retain has its own timeout, default 60 s (#248).
- `rung-host`: default model selection is the named ladder (#250); runs on its
  own router key and treats 2xx router errors as classified failures (#249).
- Turn check sends the request and final message whole, no clip (#253).
- Memory retain runs after the reply (#242); clearer reason when a provider
  returns junk (#233).

### Fixed
- `rung-agent-core`: `memory_score` sets `retain_timeout`; master did not build
  after #239 and #248 merged together (#256).

### Added
- `rung-agent --memory-scope` inspects and deletes baseline memory scopes (#234).
- `rung-agent --memory-score` scores memory providers on real notes (#239).
- Memory: opt-in retain distillation, `RUNG_MEMORY_RETAIN=distill` (#226);
  opt-in `baseline:stem` recall ranking (#235).
- `rung-host`: opt-in free-time idle rule (#247).

### Documentation and CI
- Memory quickstart (#225) and loop walkthrough (#246); 2-hour live qwen run
  (#245); named-ladder soak evidence (#251); `rung_testkit::TempDir` in
  retain_quality and writeback (#252).
- Release commit (this one).

## [0.2.2] - 2026-10-06

Patch bump: backward compatible fixes and additions.

### Fixed
- Prompt cache: the recalled-memory block is persisted on the session line and
  replayed byte-for-byte, so the cached prefix holds. Live turn-2 cached tokens
  with memory on rose from 2432/4999 to 4864/5236 (#243).
- `rung-agent-core`: replay the bytes a turn sent, and keep system text and
  tools across session restart (#229, #230).
- `rung-std`: send `session_id` and `cache_control` markers on the
  OpenAI-compatible wire (#227).
- `rung-host`: cut long tool calls when an owner stimulus is waiting (#198);
  treat router-refused rungs as unavailable until the next listing (#200);
  reachable 25% pack break floor (#202); acknowledge owner messages at once
  while the provider backs off (#203); desk qid injective (#201).

### Added
- `rung-host`: model-free status report extension (#241); live run config,
  Shadow-mode decision desk (#199); keyed probe of each free ladder model
  (#214); cached identical memory recalls with bounded recall context (#232).
- Runnable notes and reminders example agents (#237); cache-hit probe script
  (#222); shadow-week audit script (#224).
- `rung-memory`: recall-quality fixtures and BM25 baseline scorer (#220).

### Changed
- One civil-date formatter in `rung-std` (#212); unused `Container` trait and
  `By::is_rule` deleted (#210, #211).

### Documentation and CI
- Host quickstart (#223), live cached-token proof (#236), prompt cache miss
  guide and ACP cache probe (#231), host testing conventions (#218).
- Testkit: shared scripted mock LLM server and `TempDir` scratch dirs (#213,
  #215, #216); offline ACP memory loop smoke test (#221); 24-hour simulated
  soak (#238); simulated-clock gate hardening (#195).
- Review council caller and principles, pinned to v0.1.0 (#174, #219).
- Release commit (this one).

## [0.2.1] - 2026-10-05

Patch bump: additions are backward compatible and `rung-host` is
`publish = false`.

### Added
- Memory: recall report lists injected record ids; optional bearer for MCP
  HTTP providers (#175). Marked context block stored in the session once (#184).
- `rung-host`: Engine adapter and ladder listing filter (#165, #177); ACP outward
  gate adapter and startup handoff (#178); ACP over Streamable HTTP with
  per-role bearer tokens (#182); startup-handoff ladder and `run --config`
  (#196).

### Fixed
- `rung-host`: non-owner `*.msg` files pinned to their own peer channel (#166);
  poisoned mutexes recovered via one shared lock helper (#194); kernel commit
  and release atomic under concurrency (#192).
- Leaked test temp dirs removed and temp-root CI check added (#185, #193).

### Documentation and CI
- `release-check` CI job: tag, lockfile, publish flags, release build (#187).
- MSRV 1.88 declared with an MSRV CI job (#189).
- Examples and doctests run in CI (#180); CI gates listed in AGENTS.md and
  skills (#179); docs map and CHANGELOG (#181, #188, #190); extra property,
  trybuild and carrier tests (#186, #191).
- Release commit (this one).

## [0.2.0] - 2026-10-04

### Added
- `rung-host`: single-agent continuous host crate, slice 1 (#153); Engine
  adapter, gate engine and HTTP sim (#165).

### Changed
- `rung-agent` split into the `rung-agent-core` library and a thin shell (#152).

### Fixed
- `rung-host`: governor, inbox, outbox and time arithmetic hardened (#171);
  gate flake evidence and load-scaled waits (#170); unit-test temp dirs
  guarded (#161).
- Turn check applies to nested task spawns (#156); a failed turn-check
  re-run keeps its steps (#155).
- Memory cue and retained user side built from unmarked prompt blocks (#169).

### Documentation and CI
- Rustdoc warnings fixed and gated in CI (#163).
- Consumer and person names genericised in tests; consumer guard widened to
  whole-word names (#154, #164).
- Release commit (#172).

## [0.1.13] - 2026-10-03

### Added
- `rung-agent`: pluggable memory provider with off/external/baseline/mcp
  authority (#150).

### Changed
- Consumer-specific names and redaction defaults removed (#148).

### Documentation and CI
- AGENTS.md: rung is standalone, no consumer dependency (#147).
- CI guard against consumer-specific names (#149).
- Release commit (#151).

## [0.1.12] - 2026-10-02

### Added
- `rung-agent`: oldest tool results elided once on context overflow, with a
  typed overflow report (#145).

### Fixed
- ACP carries typed terminal states on the wire (#143); `session/cancel` and
  `session/close` land mid-turn (#144).
- Error turns keep their steps and never replay the failure (#142).
- `--tools`, `--json` and `--stream` forwarded to the background child (#141).
- ACP session store resolved by session cwd, not process cwd (#140).
- Release commit (#146).

## [0.1.11] - 2026-10-01

### Added
- `rung-agent`: `TurnCheck` rung with a Jev-backed `Decider` (#135).
- `rung-std`: tool results carry images to models that accept them (#138).

### Fixed
- Runaway guards key on the host's cap and on lack of progress (#137).
- Model refusals and their reason surface as a visible error (#136).
- Judge verdict-space point and confidence sealed inside `Judgment`
  (Het 4.6) (#134).
- Het judge requires a word boundary after `FAILS` / `CANNOT-SETTLE` (#133).
- Token-limit (`finish_reason` length) replies reported as truncated (#132).
- Release commit (#139).

### Documentation
- Release procedure, session history replay, local trybuild drift.

## [0.1.10] - 2026-09-26

### Fixed
- Shell returns when bash exits rather than when every holder closes its
  output; a command past its limit is killed.

## [0.1.9] - 2026-09-26

### Changed
- Output uncapped: oracle default 0 (no cap); Anthropic native ceiling 128k.

## [0.1.8] - 2026-09-26

### Fixed
- LLM keepalives no longer reset the idle deadline; stalls retry; cut-off tool
  calls tell the model.

## [0.1.7] - 2026-09-22

### Added
- Session history replays each turn's tool calls and results (#128).

## [0.1.6] - 2026-09-22

### Added
- ACP: per-session system prompt; no `max_tokens` cap by default.

### Fixed
- MCP: never dispatch a tool call built from unparsed arguments.
- Formatting of `agent.rs` and `types.rs`.

### Changed
- `.agent/` inhabit and handoff ceremony dropped.

## [0.1.5] - 2026-09-02

### Added
- Raw model events streamed over ACP.

## [0.1.4] - 2026-09-02

### Added
- OpenAI-compatible path forwards OpenRouter reasoning deltas.

## [0.1.3] - 2026-09-01

### Added
- `HetOpt`, the worth-law extension (`rung-het-props.md` section 8).
- Reasoning visibility: `reasoning_content` deltas parse; `RUNG_REASONING`
  maps to `reasoning_effort`; ACP forwards thinking as `agent_thought_chunk`.

## [0.1.2] - 2026-08-27

### Added
- `RUNG_SYSTEM_PROMPT_FILE`, a system-prompt config surface.

## [0.1.1] - 2026-08-27

### Added
- `BEING_PREFILL` env channel for the system prompt.

## [0.1.0] - 2026-08-27

First tagged release; the foundation of everything above.

### Added
- `ladder!` macro and runtime: sealed constructors, immutable carry, recovery
  arms, `#[must_use]` tokens, `!Send + !Sync` rungs, long spines, gate markers
  and the authorial gate (G13, G14).
- `theory!`, the Het formalism and `rung-het` pool (judge vs author), the
  audit-rectify pass, panels, verify step and `HetOpt` groundwork.
- `rung-std` blocks: `llm` (streaming, structured output, content blocks,
  retry hardening), `agent` loop and nested `task` subagent, jailed `python`,
  `tools` (edit, glob, grep, apply_patch, todo, webfetch, skill), `questions`
  and `principals` theories.
- `rung-doctrine`: propositions encoded and rendered to generated props and
  `docs/conformance.md`; `docs/_props.py` checks.
- `rung-driver`: theory-blind carrier, audit and audit-rectify cycles, the
  `.het/` state sidecar.
- `rung-agent` CLI: headless task mode, XDG `config.yaml`, `--json`,
  `--stream`, tool groups and `--toolset`, background and sessions.
- ACP v1 on stdio (`--acp`) with tool-call updates, abort, fork and resume,
  image/audio prompts and MCP HTTP/stdio tools (#119, #125); experimental
  Streamable HTTP (`--acp-http`).
- Harbor adapter and the `rung_harbor` validation suite with timestamped
  evidence.
- CI: fmt, clippy `-D warnings`, tests, propositions job.

[Unreleased]: https://github.com/witt3rd/rung/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/witt3rd/rung/compare/v0.1.13...v0.2.0
[0.1.13]: https://github.com/witt3rd/rung/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/witt3rd/rung/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/witt3rd/rung/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/witt3rd/rung/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/witt3rd/rung/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/witt3rd/rung/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/witt3rd/rung/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/witt3rd/rung/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/witt3rd/rung/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/witt3rd/rung/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/witt3rd/rung/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/witt3rd/rung/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/witt3rd/rung/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/witt3rd/rung/releases/tag/v0.1.0
