# Changelog

All notable changes to rung, from git history and the GitHub releases.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Numbers in parentheses are pull requests. Rung is unpublished; versions are
git tags only.

## [Unreleased]

## [0.3.4] - 2026-11-11

### Added (rung-agent, ACP)
- The ACP initialize response advertises `_rung/session/handoff` so a host can ask whether the agent does handoff without trying it (#401).

## [0.3.3] - 2026-11-11

### Added (rung-agent, ACP)
- A prompt content block with `_meta.ephemeral: true` is sent last on every
  model call of its turn and never retained in the session (#398).
- `_rung/session/handoff {sessionId, handoff, keepLastTurns}`: replaces every
  session line before the last `keepLastTurns` user turns with one `summary`
  line holding the handoff text the agent wrote, keeping tool-call/result pairs
  whole, and returns the before and after sizes (lines and estimated tokens).
  Compaction by the agent's own handoff; `compact::handoff` has its caller now.
  A session that never calls it is unchanged (#399).

### Added (rung-fleet, R2 step 9: spawn)
- `rung-fleet::spawn` and the `spawn` command: a member is prepared under a root
  (directories and a host configuration that names its inbox and stop file) and
  its brief is delivered to the inbox before the host starts; `launch` starts the
  host process (`--host-bin`). A task cannot be spawned without a validated brief,
  a name must be a registry id, and an existing member is never overwritten. A real
  host starts from the written config and records the brief. This closes the R2 verbs.

### Added (rung-fleet, R2 step 8)
- Crew lines as a view over the peer items the host already records:
  `steer::report` puts a `crew.*` fact in a member's inbox as a peer item,
  `CrewRecord::from_stimulus` reads it back from the record, and `verbs::crew`
  (and `rung-fleet read --crew`) returns native lines and facts in order. Only peer
  items count. No host change. Proved against a real host. `spawn` remains.

### Added (rung-fleet, R2 step 7)
- `rung-fleet::steer::assign` and the `assign` command: a task goes to a member's
  inbox as an owner item carrying the whole brief (task, objective, done when,
  delivery, autonomy); only a validated `Brief` can be sent. Spawn and the
  writing of `crew.*` lines are not built: see the notes.

### Added (rung-fleet, R2 step 6)
- `rung-fleet::steer::stop` and the `steer` and `stop` commands. `stop` sends an
  owner item whose `control` is `stop` through the member's inbox, so the host's
  own stop authority halts it and its record holds the reason (the item's text)
  and who asked (its channel, `fleet:<from>`); the member stays in the list.
  Proved against a real host. Prop `stop-records-why` becomes decidable. The
  commands take `--wait SECS` to confirm against the member's record.

### Added (rung-fleet, R2 step 5)
- `rung-fleet::steer`: the `steer` verb. It finds a member's inbox from the
  `inbox` its configuration names (through the instance index) and sends an
  owner-class `Kind::Steer` item by the delivery module, idempotent by
  correlation id; the returned `Sent` is confirmed against the member's record.
  A test finds exactly one new file, the inbox item. Prop
  `a-steer-is-an-inbox-item` becomes decidable. The command line does not carry
  `steer` yet.

### Added (rung-fleet, R2 step 4)
- `rung-fleet::verbs` and the `rung-fleet` command: `list` and `read` (a member's
  record lines, or its `crew.*` lines with `--crew`), answered from the instance
  index and the host's own readers. Both only read: a test snapshots every file
  under the index and the state directories and finds them unchanged. Prop
  `read-and-list-never-write` becomes decidable. `steer`, `stop`, `spawn` and
  `assign` are not built yet.

### Added (rung-fleet, R2 step 2)
- `rung-fleet::delivery`: delivery as a typed receipt. `send` writes an inbox
  message named by its correlation id and returns a `#[must_use]` `Sent`;
  `Sent::confirm` reads the receiver's own record and returns a `Receipt` that
  names the `stimulus.accepted` line, or hands the promise back. A receipt is
  consumed once (`record`, `hand_on`, `discard`). Dropping a `Sent` or a
  `Receipt` fails to compile (the crate denies `unused_must_use`). A duplicate
  send adds nothing: tested against a real host. Props
  `an-unconsumed-receipt-does-not-compile` and
  `duplicate-delivery-is-idempotent` become decidable.

### Added (rung-fleet, R2 step 3)
- `rung-fleet::records`: the `crew.*` record kinds as a typed `CrewRecord` (ten
  kinds, from `crew.spawned` to `crew.retired`, plus `crew.receipt`) that
  encodes to and decodes from a record line. A reader skips a kind it does not
  know; a known kind with a bad field is refused. The host reads a record that
  holds them, and a test fails if any other crate writes a `crew.` kind. Prop
  `crew-record-kinds-are-fleet-owned` becomes decidable. Writing the lines is
  the verbs' work.

### Added (rung-fleet, R2 step 1)
- New product crate `rung-fleet` (`publish = false`). `lifecycle` holds the
  typed delegation ladders: `delegation` (spawned, assigned, active, with held
  and stalled as recoverable verdicts, ending in done) and `closure` (done to
  retired, enterable only from a `Done` verdict). `Brief` is the typed
  contract a task is spawned with. Compile-fail cases pin a skipped
  transition and a forged `Active` or `Finished`. Props
  `a-skipped-transition-does-not-compile`, `held-and-stalled-are-recoverable`
  and `a-brief-is-a-typed-contract` become decidable. Retirement does not yet
  consume proof of landed work (R3).

### Added (rung-ui, crew and receipts on fixtures)
- `rung-ui --crew-fixture [FILE]` shows a Crew section on a steward's page from
  sample data (`gui/rung-ui/fixtures/crew.json`, or a file): the delegation ladder
  (held and stalled marked as needing someone), each member's brief, its rung's
  facts and its receipts (each naming a record line; a repeat is one line).
  Labelled "SAMPLE DATA, not from the host" and never shown without the flag: the
  host serves no crew records until the fleet's R2 settles their kinds. The old
  `ui/` is untouched.

### Added (rung-ui, panels, keyboard and theme)
- The `rung-ui` list and instance page are two resizable panels; Alt+Down and
  Alt+Up select the next and previous instance in list order (a tested helper,
  stopping at the ends), and Ctrl+T switches light and dark. The chords work
  anywhere in the window, including the message box. The old `ui/` is untouched.

### Added (rung-ui, the turns and the conversation)
- `rung-client` reads the host's turns door (`turns`, `turn`: the reply whole,
  llm use, what each turn was asked, its tool calls with whole inputs and
  results), tested against a real gateway and host (newest first, the older page
  by cursor, a finished turn, 404). The `rung-ui` page gains Turns: the host's
  own turns newest first with headline and small print, refreshed when a turn
  begins or ends, older pages on a click; a click opens a turn: what it was asked
  (the owner's messages, intent, the turn running when sent), its reply whole
  (Show more / Show less, never cut), its tool calls (click for whole input and
  result) and its facts. No fold of the record in the UI. Replaces the old web
  page's Turns page; the old `ui/` is untouched.

### Added (rung-host and gateway: the turns door)
- `GET /api/i/{id}/v1/turns` (newest first, paged by turn number with `before`
  and `after`, no maximum on `limit`) and `GET /api/i/{id}/v1/turns/{n}` are
  served for native instances, as specified in `docs/rung-host-api.md` S8:
  `rung_host::turns` folds the record into turns on the host's side (status
  completed, bounded, failed, running, interrupted; the reply whole; llm sums; the
  items each turn was asked; its tool calls with whole inputs and results), in
  two reads that keep only the asked turns' texts. Read from the state
  directory, so a stopped instance answers; every text redacted and none cut.
  Proof: `rung-gateway/tests/turns.rs` (the memory-bound allocator test is owed).

### Added (rung-host API, the turns door specified)
- `docs/rung-host-api.md` S8: the specification of the host's `turns` door
  (`GET /v1/turns` paged by turn number, `GET /v1/turns/{n}`): one fold of the
  record into turns on the host (status, reply, llm sums, asked items, tool
  calls with whole inputs and results), so no client folds the record. Text
  only: nothing is served yet; the slice that serves it carries the proof the
  section lists, written first.

### Added (rung-ui, the context inspector)
- `rung-client::Client::context` reads the host's context door (what was sent
  to the model on one call of one turn: bands in wire order with the host's size
  estimates and cache marks, the provider's counts, cache breakpoints, what the
  record cannot give back whole), tested against a real gateway and host (band
  order, the host's own sum, 404s). The `rung-ui` page gains a Context section:
  the request as stripes sized and marked from those answers, ◀ ▶ over turns and
  calls, a click on a band for its text, and the provider's hit rate; `--turn N`
  opens it at a turn. It replaces the old web page's Context page. Reading only;
  the old `ui/` is untouched.

### Added (rung-ui, the queue and the owner's controls)
- `rung-client` reads an instance's queue and gains the owner's calls: send
  (`queue_add`, with intent next/steer/free and a key), `queue_move`,
  `queue_cancel`, `stop`, `release`; error bodies read in the gateway's own words
  (flat or nested). Tested against a stub (method, path, body, bearer) and a real
  gateway and host (the owner sends and is listed; the read-only token is refused
  403 "this token can only read"; stop/release). The `rung-ui` page shows the
  queue; the window starts read-only and its controls (send box with intent,
  ↑ ↓ and cancel per waiting item, stop and release asking twice) appear only
  after a click on "Turn controls on" (or `--controls`). The old `ui/` is untouched.

### Added (rung-ui, the live record)
- `rung-client` reads the gateway's event stream: `Client::events` (one
  connection, frames in order) and `follow` (reconnect with `Last-Event-ID`
  from the last record seen, quiet back-off from 1 s to 30 s, a fresh start on
  a cursor from another instance, stop on a flag), tested against a scripted stub and
  a real gateway and host (replay, resume by cursor, 409/404, a killed host
  ends its stream `down`). The `rung-ui` instance page follows the chosen
  instance: its newest record lines (newest first, 300 kept) and the running
  turn's text and tools as they arrive, with the stream's state in words
  (live, reconnecting, ended). Read-only; the old `ui/` is untouched.

### Added (rung-ui, the instance page)
- Clicking an instance in `rung-ui` opens its page: Now (state, doing, what it
  waits for, turn, last activity), Queue, Spend (paid today, requests of quota,
  desk spend of its day cap), Desk and Governor (model and rung, failures,
  blocked, unavailable rungs), laid out from the summary the list already
  carries; an unreadable instance says why. `rung-client`'s `Summary` now reads
  those fields (all optional; an unexpected shape reads as absent). `--select ID`
  opens with one chosen and `--show ID` prints the page as text. Read-only; no
  fold in the UI; the old `ui/` is untouched.

### Added (rung-ui, the fleet screen)
- The `rung-ui` instance list is now the fleet: grouped Chiefs, Stewards and
  other instances, by the id's first segment (`rung-client` gains
  `Instance::role()` and `Instance::unreachable()`). A host that needs you is
  marked `●` in red, one the gateway cannot read `○` in amber, and the header
  counts both; `rung-ui --list` prints the same grouping as text. The id names
  no chief for a steward, so none is invented. Still read-only; the old `ui/`
  is untouched.

### Added (rung-ui, the new desktop UI's first window)
- `gui/rung-ui`: a GPUI Kit window listing the instances behind a gateway
  (name, the host's own state word, what it is doing; a needs-you mark; the
  last good list stays up when the link drops), refreshed every three
  seconds and read-only. `rung-ui --list` prints the same list as text with no
  display. It is built and tested only by the `gui` job (which now installs
  GPUI's system libraries), never by the root build or `check`. The build
  lane's needs are in `gui/rung-ui/AGENTS.md`. The old `ui/` is untouched.

### Changed (docs overhaul)
- No `README.md` anywhere: every remaining `README.md` is now `AGENTS.md` (`git mv`):
  `docs/`, `docs/records/`, `docs/records/rung-host/`, `.het/`,
  `.het/rung-questions/judgments/`, `.het/rung-questions/questions/`, `ui/`,
  `ui/contract/`, `rung/examples/`, `examples/notes/`, `examples/reminders/`,
  `rung-memory/tests/fixtures/recall/`, `.archive/.gl/meta/`. Links and source
  comments follow; the docket and the records keep their text except for the
  link target `README.md` becoming `AGENTS.md` (`docs/_records_guard.py` allows
  exactly that retarget). `docs/_links.py` now fails any tracked `README.md`.
  The judgment reader skips `AGENTS.md` instead of `README.md`.
- Guides move into `docs/guides/` (`git mv`): `rung-host-quickstart.md`,
  `rung-memory-quickstart.md`, `shadow-week.md`. `rung-memory-walkthrough.md`
  is folded into the memory quickstart as its worked example. `docs/AGENTS.md`
  states the layout rules.
- Reference move (`git mv`): `docs/rung-host.md`, `rung-agent-oauth.md`,
  `rung-agent-cache.md` and `rung-memory.md` into `docs/reference/`;
  `theory-of-theories-in-context.md` into `docs/design/`. The host's Profiles
  section (design, not built) is now `docs/design/rung-host-profiles.md`. Links
  and source comments follow; record files that link to `rung-host.md` follow it (the records guard reads
  the retargeted link as unchanged).
- `docs/rung-host-api.md` split into four reference files by topic, text and
  section codes unchanged (`git mv`, then cut): `docs/reference/rung-host-doors.md`
  (H1 lock, H2 registry, H3 read doors, H5 events, S4 context, S8 turns, and the
  map of all five), `rung-host-queue.md` (S1, S3, S2, S7), `rung-gateway.md`
  (H9) and `rung-redaction.md` (H4). Citations in code and docs follow.
- New guides: `docs/guides/rung-agent.md` (flags, `config.yaml`, environment
  table, OAuth login), `rung-gateway.md` (run a gateway) and `release.md`.
  `docs/_coverage.py` (CI, `propositions` job) fails when a `RUNG_*` variable
  the code reads or a `rung-agent` flag is in no guide and not on its internal
  list.
- Skill and ui split: the Harbor suite section of `skills/rung/SKILL.md` is now
  `rung-agent/python/AGENTS.md` and its live-run section `rung-host/live/AGENTS.md`;
  the per-slice behaviour of the page (slices 1, 5, S1) moved from
  `ui/AGENTS.md` to `docs/records/rung-ui/ui-slices.md`, leaving how to run and
  the rules.
- Retired `docs/_migrate.py` (the one-off props migration); three doctrine source comments no longer name it.
- Records moves (`git mv`, no stubs; text unchanged, old to new in
  `docs/records/README.md`): the three dated measurement notes
  (`rung-agent-cache-live-memory`, `rung-decision-model-replay`,
  `rung-ui-live-cost`) and `docs/evidence/` into `docs/records/rung-agent/` and
  `docs/records/rung-ui/`; `docs/rung-het-publishing.md` to
  `docs/records/rung-het/publishing-brief.md`;
  `data/rung-test-requests/rung-webfetch-artifact-build.md` to
  `docs/records/rung-host/`; the hidden `docs/.archive/` to
  `docs/records/archive/`. Links and the `docs/_consumer_guard.allow` prefix
  updated in the same change.
- Crate fronts: every workspace crate has a short `AGENTS.md` beside it (what
  it is, status, where its docs are, the one check command); `docs/_links.py`
  fails a workspace member without one, and `scripts/ci_plan.py` treats a
  crate's `AGENTS.md` as documentation, not a reason to rebuild the crate.
  Crate docs stay in `docs/`.
- `LICENSE-MIT` and `LICENSE-APACHE` added: `Cargo.toml` already declared
  `MIT OR Apache-2.0`, but no text was tracked. rung stays unpublished. The
  `rung` and `rung-macro` crates gain a `description`.
- Front door: the root `README.md` (a symlink to `AGENTS.md`) is removed. rung
  is an agents-only library: `AGENTS.md` is the one agent file name, there is
  no root `README.md`. `AGENTS.md` is rewritten as a short charter. Moved:
  - the Getting Started program (doctested) moves from `README.md` to the crate
    docs of `rung/src/lib.rs` (section "Getting started"); the `rung` crate no
    longer includes `README.md` as a doctest;
  - the `rung-agent` ACP notes, configuration table and Harbor suite move from
    `AGENTS.md` to `rung-agent/AGENTS.md`;
  - the hand-written kind-count table is replaced by a link to
    `docs/conformance.md`; its test in `rung-doctrine/tests/roundtrip.rs`
    (`hand_written_counts_in_prose_match_the_doctrine`) is removed, and
    `docs/conformance.md` re-rendered.
- `docs/_links.py` (in the `propositions` job) fails a document that points at
  something absent: a relative link, an anchor, a backticked repo path, a
  a `docs/` document cited from code, or a hand-kept test count `(N tests)`. Its
  known exceptions are listed in `docs/_links.allow`; the list only shrinks.
- Drift fixed: `G1`–`G16` and the idle and fleet rows in `AGENTS.md`, the
  stale `Last updated` line gone; `docs/triage.md` names all five props
  documents and no count; `docs/rung-notes.md` points at the Python proof of
  concept under `.archive/python-poc/`; the hand test counts in
  `docs/rung-host-api.md` removed; `ui/README.md` and `.het/README.md`
  corrected; the stale "Next" roadmap in `skills/rung/SKILL.md` removed.

### Added (rung-client, the new UI's gateway client)
- `gui/`: a second cargo workspace, outside the main one (no default build or
  `check` cost), for the new desktop UI. First crate: `rung-client`, a typed
  blocking client of the gateway's health and instance-list doors (state word
  and "doing" are the host's own; the read-only token is never printed),
  tested against an in-process stub and against the real `rung-gateway` and
  `rung-host` binaries. A separate `gui` CI job (not part of `check`) runs it
  when `gui/` or the gateway or host changes; `scripts/ci_plan.py` gains a
  `gui` output. The old `ui/` is untouched.

### Added (rung-host idle mode display)
- A resting host reads as idle until its wake wherever the host is read: the
  gateway's summary and list say `Idle until <time>` (not `needs_you`, not
  `Waiting`; `idle_until` and `waiting_for: idle` carry the wake), and
  `rung-host ls` shows `idle until <time>` (its `word` stays `running`; `ls --json`
  rows gain `idle_until`, read from the tail of the record by the rule the fold
  uses, `state::idle_effect`). The last two idle-mode props become decidable
  (`a-resting-host-is-seen-resting`, `a-restart-wakes-the-host`). The rest and
  the governor's wait share one helper for the next external event.

### Added (rung-host idle mode)
- Idle mode: with idle time above 0 the host rests between wakes. It rests
  (an `idle` line, then `idle.ended` with its cause) until the first of its
  scheduled wake (idle time away, or earlier for a due calendar item or an
  expectation to settle), a queue arrival, or the stop authority. After a wake
  it drains the queue, takes one free-time pass, and rests again; a rest runs
  no boundary, turn or model call and is never `degraded`. Idle time is
  `idle_time_s` in a config file: 0 is continuous, absent is 30 minutes (a
  change for configs that named none; the examples and the live config set 0).
  `HostConfig::idle_time_ms` is 0 unless set. Nine props of
  `docs/rung-host-idle-props.md` become decidable (`rung-host/tests/idle_mode.rs`);
  the display of a resting host and a restart waking it stay owed.

### Added (rung-host idle mode props)
- `docs/rung-host-idle-props.md`: the normative claims of rung-host idle mode
  (idle time per instance, 0 is continuous, default 30 minutes; rest until a
  scheduled wake or a queue arrival; one free-time pass per wake; a stop
  interrupts a rest; a resting host shows as idle until its wake). Generated
  from `rung-doctrine/src/rung_host_idle.rs`; every claim is `owed` except the
  existing idle-time-0 gate (G-a), until the change that builds idle mode adds
  its tests. `docs/conformance.md` lists them. No host code.

### Added (rung-fleet, R1)
- A chief and a steward are two ordinary `rung-host` instances behind
  `rung-gateway`; no fleet code. The registry convention is a name that begins
  with the role (`chief-main`, `steward-alpha`), with no new entry field.
  `rung-gateway/tests/fleet_r1.rs` runs two chiefs and a steward as real hosts
  (mock engine), lists them with the read-only token, reads a record, follows a
  live stream and shows that one chief's inbox message reaches no other
  member. Three props become decidable: `chiefs-do-not-share-context`,
  `a-member-is-named-by-its-role` (new), `a-read-only-role-watches-the-fleet`
  (new). Measured cost per host and the gaps found: `docs/rung-fleet-notes.md`
  (witt3rd/rung#347, #352).

### Added (rung-fleet props, R0)
- `docs/rung-fleet-props.md`: the normative claims of `rung-fleet`, a planned
  product crate that supervises rung hosts as a tiered fleet (chief, steward,
  crew). Generated from `rung-doctrine/src/rung_fleet.rs`; every machine-settled
  claim is `owed` until the crate adds its test. `docs/rung-fleet-notes.md` is
  the derivation, with the phase each claim is discharged in and the points the
  owner still confirms. `docs/conformance.md` lists the new claims. No crate
  code (witt3rd/rung#347, #349).

### Added (OAuth credentials)
- `rung-agent`: a second credential kind beside `api_key_env`. `llm.auth.oauth:
  {provider, profile}` (or `RUNG_OAUTH_PROVIDER` / `RUNG_OAUTH_PROFILE`) selects
  a login made once with `rung-agent auth login github-copilot` (device code:
  prints the code and URL, no browser on the host) and a bearer minted on
  demand. GitHub Copilot is the first provider. Logins live machine-local at
  mode 0600 under `$RUNG_HOME/oauth/`, never in a repo; named profiles; one
  exchange however many callers or processes ask (a file lock); a 401 forces
  one refresh and one retry; a revoked login is an auth failure naming the
  command that fixes it; the route comes from the token's `proxy-ep`; the
  editor-identity headers are configuration. Also `auth status | models |
  logout`. Contract and proofs: `docs/rung-agent-oauth.md`. Whether a
  non-editor client may use the Copilot API is an open decision (O18)
  (#350).
- `rung-std`: `llm::CredentialSource`, the seam a minted credential plugs into
  (`LlmConfig::credential`); OpenAI-compatible route only.
- The credential redactor removes Copilot tokens (`tid=…;exp=…;…`).
- `rung-testkit`: `http::Server`, a scripted HTTP server for any endpoint.

### Added (performance gate)
- `scripts/perf_gate.py`: cold start, memory at a 10 MiB session and session
  switch time for rung-agent, medians over interleaved trials against a scripted
  model; noisy metrics are withheld; the gate fails on more than 10 percent
  regression. No baseline is recorded yet.

### Changed (one worker process per ACP session)
- `rung-agent --acp` (stdio) now fronts one detached worker process per
  session (`acp_proxy`): a long turn, crash, OOM or hang in one session no
  longer stalls or kills another, and the process cwd a turn sets is that
  session's alone. Messages route by `sessionId`; updates and `session/cancel`
  pass through unchanged. A worker that dies fails its open requests and is
  restarted on the session's next message (`session/resume` from its file),
  with 250 ms to 30 s backoff, 5 consecutive failures max, a 30 s lifetime
  counting as stable. `RUNG_ACP_WORKERS=0` keeps the single process; `--acp-http`
  and `rung-host` are unchanged. Cost: about 20 MB resident per idle worker
  (debug build, measured on Linux). Turns of one session still run in order.
  The per-process LLM call counter in diagnostics now restarts per session.

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
  workspace file (`docs/records/rung-host/web-fetch-spill-design.md`). A body of at least
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

### Documentation and CI
- `docs/rung-host.md`: idle mode specified beside continuous mode (#351). Idle
  time above 0 makes the host really idle between boundaries, wake on a
  stimulus or a scheduled wake (default 30 minutes), and take one free-time
  pass per wake; idle time 0 is the shipped behaviour. Marked SPECIFIED, NOT
  BUILT; no code.
- `docs/rung-host.md`: a Profiles design section (#346).
- The eight rung-host records moved to `docs/records/rung-host` with an index
  and a dated Moved note on each (#342); the rung-host testing doc was merged
  into the spec as a Gates section and the spec de-duplicated (#343).
- CI: `docs/_records_guard.py` keeps `docs/records` append-only; it runs in the
  required `check` job (#344).
- Test: `acp_http_body_stream` no longer underflows when the runner reports a
  lower peak RSS after the run than before it (#345).

## [0.3.2] - 2026-10-20

### Added (read_file character ranges)
- `rung-std`: `read_file` takes optional `from_char` / `to_char` (Unicode
  characters, 1-based, ends included), as `ws_read` does, so a file of one very
  long line (a tool result saved whole to a file by a host) can be read in
  parts. A part starts with a header naming the file's character and line
  totals; a range past the end gives the header and no text; offset/limit
  together with a character range, or a backwards range, is refused. With no
  range, and for line reads, nothing changes. Ships in 0.3.2.

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

[Unreleased]: https://github.com/witt3rd/rung/compare/v0.3.4...HEAD
[0.3.4]: https://github.com/witt3rd/rung/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/witt3rd/rung/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/witt3rd/rung/compare/v0.3.1...v0.3.2
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
