# Changelog

All notable changes to rung, from git history and the GitHub releases.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Numbers in parentheses are pull requests. Rung is unpublished; versions are
git tags only.

## [Unreleased]

### Added (live strata, host half)
- `rung-host`: the layers of a turn's context are recorded at its start. A new
  `turn.layers` line (the header and the recall block, whole and redacted, with
  the hashes of the stable and epoch layers) is written in the same write as
  `turn.started`, so the record holds them while the turn runs. The context door
  `GET /v1/turns/{n}/context` answers for a running turn with its first request
  as assembled (`live: true`, `calls: 0`, cache marks `unknown`) instead of 409;
  when the turn ends the same bands settle at the final record (the same text,
  with the provider's counts and cache marks added). A later call of a turn that
  has not ended is still 409 `turn_not_logged`; a turn whose host went away
  before it ended answers 200 with the same bands, `live: false` and a
  `problems` entry saying it did not end. The
  event stream already carries every record line, so `turn.layers` is the
  "layers ready" event; no new door, no new write path, no cap on any text.
  The recall block is written once, in `turn.layers`; `turn.log` no longer
  carries `recall_text` and `recall_hash` (it keeps `header`, `recall` and the
  messages), and the door reads the recall from `turn.log` for a record written
  before this change and from `turn.layers` after it.
  Readers older than this change lose the recall band on new records (rung is
  unpublished, so nothing is kept for them).

### Fixed
- ACP HTTP listener: the authenticated request body is streamed to a spool file
  (`<state dir>/spool/` in `rung-host`) instead of held in memory about five
  times over; memory stays flat for a very large body. No cap, no truncation.
  A RAM-backed spool dir defeats flat memory.
- `rung-host`: following the record door's `next` cursor is linear, not
  quadratic. Each page used to scan the whole record from the start to learn
  `total`; the host now keeps, per state directory, an index of the intact
  prefix (its length, the first damage, and a seek mark every 1024 lines) and
  reads only what was appended since, then seeks to the page. No cap on `limit`
  or on any text; the index is checked against each segment's identity (inode,
  length, modification time) and rebuilt if a segment was replaced, shrank or
  vanished, so a record replaced in place under a live process is never served
  stale. A page whose line numbers do not follow on is an error, not a short page.
  The first page of a record after a start reads it once (the exact `total` needs
  it); later pages cost about their size. An index unused for ten minutes is
  dropped and built again by the next page.

### Added
- `rung-host`, `rung-gateway`: intent on the queue item (S2). `POST
  /v1/queue` takes `intent` (`next`, the default; `steer`; `free`), refusing any
  other value in words with nothing written; the item carries it in the record
  (absent reads `next`) and the list shows it. A `steer` is read at
  the next boundary like `next`, is marked in the header the model reads, and cuts a
  long tool call through the same `OwnerWatch`. `free` is the owner's explicit
  exception to "owner items are read at the next boundary, always": it is held while
  the agent is committed and read in free time or at a natural break, never cuts a
  call, and the desk's limits and firm deadlines still win over it. Owner items stay
  above every other sender's, `free` after the ones read at the next boundary; the
  group a move stays in is the read rank (role and intent), so a move across an
  owner's `free` and `next` is refused in words with nothing written, and the turn
  header lists items in the order the list shows (it ignored moves). An
  owner's item's text in the header is now whole (it was cut at 1,000 characters);
  so is another sender's (the 1,000-character bound on a peer's text is removed;
  the redactor still runs on it).
  No caps on content in the model header: task, project title, question, done-when,
  digest, next step and message text all reach the model whole.
  The same holds for the owner's status report and the epoch's slow layer: the
  no-caps rule is applied to the whole render path, not only the per-turn header.
  A key's repeat must carry the same intent (`docs/rung-host-api.md`, S2).

- `rung-host`, `rung-gateway`: the stop and release doors (S7, host half).
  `POST /v1/stop` asks the host to stop and `POST /v1/release` lets go of the
  current commitment (optional `reason`), both the owner's only, through the one
  acceptance ACP's `_rung/stop` and `_rung/release` use, so each is a record line
  on disk before the answer. The check and the write are one step shared
  with ACP's two methods (`Host::owner_stop`, `Host::owner_release_control`): any
  number of concurrent stops write one line, and a release writes a line only
  when it releases something. A stop is idempotent (a stop of a host already
  stopping is `repeat: true` and writes nothing); a release is not named to a
  commitment, so it lets go of whatever is held when it arrives (nothing held is
  `repeat: true`, nothing written), and a retry after a lost answer can release a
  later commitment. The doors refuse other roles, other methods and unknown body fields in words,
  and a host that is stopping refuses a release 409 `not_running`. There is no
  start door: the unit manager starts a host. The gateway forwards them under
  the queue doors' write path, and a stopped instance answers in words. The page
  buttons are a separate change. Shapes in `docs/rung-host-api.md` (S7).
- `rung-host`, `rung-std`: a huge `web_fetch` body is saved whole to a
  workspace file (`docs/rung-host-web-fetch-spill.md`). A body of at least
  `web_spill_bytes` (default 32768, in bytes) is streamed as it arrives to
  `.web/NNNN-<hash>.md` and the tool result is the header line (final URL,
  status) and a notice with the workspace-relative path and exact bytes and
  lines; below the threshold the body stays inline. There is no cap: the file
  holds the body whole, and a write failure (disk full, `.web` not a plain
  directory, permission) returns the whole body with a note in words and a
  `web.spill_failed` line. Spilling needs the `read` group; without it the body
  is returned whole. `ws_read` takes `from_line`/`to_line` or
  `from_char`/`to_char` and states the file's totals; `ws_read`/`ws_list` on
  `.web/` with `read` off is refused naming the path. The host never removes
  spilled files. `rung-std` gains `fetch_url_streaming`, which hands a page on
  piece by piece so memory stays flat.
- `rung-host`, `rung-gateway`: reorder and cancel (S3). A new record line,
  `queue.moved` (`id`, `before`, optional `key`), and the waiting order is now
  the fold of acceptance and these moves, in the host (the order a turn reads
  what it admits, which was id order, and the order the decision desk is asked
  about) and in every reader of the record (the list): the owner's items stay on
  top, a move stays inside one group and one boundary, and it changes order only
  (the desk still decides when an item is shown). `POST /v1/queue/{id}/move`
  (`before`: an item id, or null for the end of its group; optional `key`) and
  `DELETE /v1/queue/{id}` (withdraws a waiting item through the one `withdraw`
  path; refused on the item being answered) answer only once the line is on
  disk, are idempotent, refuse in words with no line written, and are forwarded
  by the gateway's write path under S1's rules. Shapes in
  `docs/rung-host-api.md` (S3).
- `rung-host`, `rung-gateway`: the queue, list and add (S1). `GET /v1/queue`
  lists what waits for the agent and the one thing being answered, computed
  from the record (so a stopped instance lists too), every text whole and
  redacted; `POST /v1/queue` adds a message through the same acceptance as ACP's
  `_rung/stimulus` and answers only once it is on disk, a key given becoming the
  item id so a repeat adds once. The host serves both on its HTTP listener
  (owner token), refusing from the headers, before any body is read; the
  gateway reads the list from a native instance's state directory and streams
  the write to the running host with the key its own config names for that id
  (`instances[].key_env`; never a variable an instance names), refusing a
  stopped instance with "start it first". An `Authorization` header the gateway
  cannot read is 401, not "no token" (`rung_host::queue`; shapes in
  `docs/rung-host-api.md`). `rung-agent-core`'s HTTP listener gets a pre-body
  `RouteGate` and hands an admitted route its whole request body, with no limit.

### Changed
- No caps on errors, logs and tool output. A provider error body is kept whole
  in `HttpContext.body` and in `HttpFailure.body` (`HttpContext.body_truncated`
  and the 16 KiB cut are gone); the `raw` of an unparsable provider answer and
  the decider's HTTP error text are the whole text; event-log lines for a
  turn's end and a tool result carry the whole text; a record parse error names
  the whole offending line; `rung-agent --memory-scope ls` prints a record
  whole; the demo binaries, the live-run scripts and the harbor failure log
  print whole text (the demo `short()` could panic on a multi-byte boundary).
- `rung-agent-core`: the experimental ACP HTTP listener no longer refuses a
  request body over 16 MiB (413); the body is read whole.
- `rung-host`: no caps on prompt text. The agent's register lists (every open
  todo, project and question), an admitted message, a digest, the commitment
  lines and the previous epoch's outline are written whole: `render::clip`, the
  `take(5/8/3/8)` list cuts, the outline's 24 lines of 140 characters, the
  expectation and calendar digest's five-item cut, and the 200-character gist
  are gone (a gist is now the whole text on one line, credentials still
  redacted). A memory candidate keeps its whole text and none is dropped, and a
  pack segment's gist is the segment's whole text. The decision desk's `max_items`
  and `max_candidates` knobs are gone: every ask is sized to what the decider
  can take, the 32,000-token context window (a provider limit,
  `DECIDER_WINDOW_TOKENS`) and, at the default, the per-ask spend cap, which is
  reached first. The admit and inject asks carry the oldest waiting items that
  fit; an item left out of an ask is not dropped, but it is not asked of the
  decider later either: the rule decides it at that boundary. The consolidate
  ask carries the candidates that fit and the rest stay for a later round. The
  pack (rollover) ask carries as many whole segments as fit, newest first,
  leaving out those an open commitment or expectation refers to (kept anyway);
  the others are decided by the rule's keep, so the pack decider is still asked
  at the default epoch budget. An ask that cannot fit even its fixed part is
  refused whole (`TooLarge`) and the rule decides. `max_segments` stays: it
  groups the epoch into that many segments, every turn in one of them. With the
  register lists shown whole, an agent that never closes its todos and projects
  has a header that grows without bound, by design.

## [0.3.1] - 2026-10-11

### Added
- `rung-host`, `rung-gateway`: the request record and the context door (S4).
  The record keeps each layer of what is sent to the model once, where it is
  made, whole and redacted: `pack.layers` (tool definitions and system text,
  at start and at a swap), the epoch layer on `epoch.rollover`, and on
  `turn.log` the recall text, once (the header as sent is the header and the
  recall, rebuilt by the door). `GET
  /v1/turns/{n}/context?call=k` on the host's listener and the gateway's
  native door joins them by their hashes and answers the stacked bands of one
  model call, with the host's size estimates, the provider's counts and cache
  marks; a record from before answers with what it has. `Host::swap_stable`
  records a swap of the stable layer. `host.start` now passes the host's
  redactor too (`docs/rung-host-api.md`, S4) (#299).

- `rung-host`: `web: true` in `rung-host.yaml` wires the live `web_fetch` (group
  `web_read`) into the host-run agent: the whole-page `webfetch` of `rung-std`
  (markdown, final URL and status, 30 s whole-body deadline), page and error
  returned whole. Default off keeps the refusal "web_read is not wired in this
  build"; the sim stays on its fake web. The web tool fetches any address it
  is given; it is not a security boundary (#293).
- `rung-host`, `rung-gateway`: the event stream with replay and partial frames
  (H5, `docs/rung-host-api.md`), with the streaming engine adapter (#283).
- `ui`: the page follows a running instance live over the H5 event stream:
  record frames resumed from the last applied line, partial text after the
  replay, "Not running" when the host stops, and a reset on `cursor_ahead`.
  The page stays read-only (#295).

### Fixed
- `rung-host`: a live fetch is charged once against the 30 s tool deadline
  (the real clock slept the fetch's duration a second time).
- `rung-std`: `webfetch` no longer claims to refuse loopback and private
  hosts. The string-match refusal (it missed `127.1`, decimal and hex
  addresses, `::ffff:` forms, and redirects) is removed; only non-http(s)
  schemes are refused. The tool fetches any address it is given and is not a
  security boundary; the other agents using the shared tool (kernel tools, the
  `web` group) get the same behaviour.

### Changed
- `rung-host`: the rule-only decision desk now enables `workspace_write` in
  free time too (it was non-free turns only), so an agent can keep its own
  files in its sandbox in its own time. Nothing else widens: free time still
  has only `core`, `memory`, `read` and `workspace_write`, and `web_read` stays
  off unless asked for (#294).

## [0.3.0] - 2026-10-11

### Added
- `rung-host`, `rung-gateway`: the H3 read doors. The gateway answers the
  instance list, one instance's fold (`/v1/summary`) and its recorded events,
  paged (`/v1/record`), for the native hosts of a registry folder, read from
  their state directories, running or not, redacted and never writing
  (`rung_host::read`; shapes in `docs/rung-host-api.md`).

### Changed
- `rung-agent-core`, `rung-std`: the turn check sends every action's input and
  result whole, all actions of the turn and all prior-turn actions; the clips
  (`INPUT_CHARS`, `RESULT_CHARS`), the dropped middle (`KEEP_FIRST_ACTIONS`,
  `KEEP_LAST_ACTIONS`, the `actions_elided` state field) and `PRIOR_ACTIONS` are
  gone. The one limit is the decider context window, now the public
  `rung_std::decide::DECIDER_WINDOW_TOKENS` (32,000); above it the turn is
  Unchecked, never cut. `turn_check::STATE_TOKEN_LIMIT` (24,000) is removed.
- `rung-std` file tools return whole text. `read_file` returns the whole file
  by default and every line whole (a caller's own `offset`/`limit` window
  stays, with a size note that is a view fold, not a cut). `glob` and `grep`
  return every hit. The not-found suggestions and edit "nearby lines" hints
  list every candidate. `glob`/`grep` walk without a depth or count limit: real
  directories are always walked once, then a symlinked directory is followed at
  most once per target, so a symlink never hides a real directory and a web of
  symlinks costs linear work; and the `task` label
  is kept whole. The `04_nondeterminism` example no longer sets `max_tokens`.

### Removed
- `rung-host`: no content caps on the host record or its tools. `turn.ended`
  carries the whole `final_text` (was cut at 2,000 characters); `ws_read` and
  `web_fetch` return the whole text (was 4,000 characters, silently; the public
  constant `toolbox::RESULT_CHARS` is gone); `ws_write` no longer refuses text
  over 64 KiB. A size note, if any, is view folding on the consumer side, never
  a cut in the tool.
- `rung-std`: the agent loop no longer cuts a tool result (it was cut at 8,192
  bytes with a `[truncated N chars]` marker). `DEFAULT_TOOL_OUTPUT_LIMIT`,
  `cap_output` and `LoopState::tool_output_limit` are removed; the model sees
  each result whole. The reactive elision on a provider context overflow stays
  and now repeats within a turn (no per-turn limit; each pass strictly shrinks
  the older results, so it is bounded), but never elides a result the model
  has not yet seen (`LoopState::fresh_from` marks the messages this iteration
  appended): when nothing older is left to elide the turn ends with an
  overflow error saying so.
- `rung-agent-core`: the public constants `MAX_RECORDS` (5) and `MAX_CHARS`
  (4000), rung's own ceiling on recalled memory, are removed (a deliberate API
  break; pinned by the v0.1.13 surface suite until now). Nothing replaces
  them: the recalled quote is bounded by what the provider returns, a budget
  the provider declares (still honoured), and the model's context window.
- `rung-agent-core`, `rung-host`, `rung-memory`: the rest of the memory path's
  content caps, a deliberate API break like the one above. The public
  constants `TURN_CHARS`, `PROMPT_CHARS`, `CONTEXT_CHARS`, `CONTEXT_ITEMS` and
  the head-and-tail `bound()` are removed (replaced by `clean()`, which only
  redacts): a retained turn, the recall prompt and every earlier answer
  offered as context reach the provider whole. `Budget::default()` no longer
  declares 5 records / 4,000 chars (it declares none), the baseline provider
  no longer declines a record over the recall budget, and distillation's
  `CONCLUSION_CHARS` is removed (every prose paragraph is kept). A budget a
  provider declares is still honoured. The memory tests and docs that pinned
  these caps changed with them.

### Changed
- `rung-std`: `webfetch` returns the whole body; the 512 KiB cut and its
  `[truncated]` marker are removed. The body streams into one copy (the
  returned text); the 30 s limit covers the whole body read, not only each read, so a slow or
  endless server still ends in `webfetch: body not complete within 30s`.
- `rung-std`: `webfetch` output changes format. A header line gives the
  final URL (after redirects) and the HTTP status, then a blank line, then
  the body. HTML becomes Markdown (headings, `[links](url)`, lists, code,
  tables as ` | ` rows, images); text of `style`, `script`, `noscript` and
  `template` is dropped. Other content types are returned as is.

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

[Unreleased]: https://github.com/witt3rd/rung/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/witt3rd/rung/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/witt3rd/rung/compare/v0.2.3...v0.3.0
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
