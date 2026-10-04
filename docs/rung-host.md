# rung-host — the continuous host

`rung-host` is a product crate (J2): one long-lived agent and everything
around its turns. It imports `rung-agent-core` as a library (never the
`rung-agent` binary). Nothing in it is kernel; nothing in the kernel
changed for it.

This document is informative. It describes slice 1 — the host against a
scripted mock engine, a fake world and a fault injector, at $0 — and what
slice 2 adds: the real engine adapter, the model ladder's listing filter,
ACP outward and the startup-handoff ladder (below). The first live run (slice 3) is in
[`rung-host-live-v1-prereg.md`](rung-host-live-v1-prereg.md) and `rung-host/live/`. Delegation to workers is a final
extension; only its extension point exists (the `crew` group name and the
`crew.*` record kinds are reserved, and the inbox admits external
completion items).

## The loop

```text
Waking(Recovered) => Boundary(Edge) => { Again -> Boundary | Halted(Why) }
```

There is no resting rung. After every turn the next boundary begins. The
only waits are ones the world imposes (provider backoff, a quota, a
blocked credential), and each is recorded as `degraded`, never as rest.
Only the stop authority reaches `Halted`: a signal, a stop file, or an
explicit stop. A boundary:

1. checks the stop authority;
2. polls the stimulus sources (memory, `*.msg` directory, the fake world)
   — each item is recorded and fsynced on arrival. A `*.msg` file never
   speaks as `host` (it is downgraded to `peer`), and only an `owner` file
   may name its channel (any other file is pinned to `peer:<id>`); only
   unparseable files are moved to `rejected/`;
3. fires due calendar items and settles due expectations;
4. asks the decision desk one combined question set (Admit, Inject,
   Tools), falling back to each family's rule;
5. picks the turn kind: **Responding** (an item was admitted now),
   **Committed** (the agent committed to a project) or **Free** (the
   default);
6. lets the governor make it wait (backoff, quota, pacing);
7. admits the batch (consuming the boundary's sealed `Edge`);
8. when the pack's gate opens, asks Pack and Consolidate, retains, and
   rolls the epoch over;
9. appends one turn header to the pack and runs one bounded turn;
10. records every model call, the turn's messages and its end (fsync),
    disposes the batch or requeues it, and goes again.

## Free time and the kernel

Free time is the default mode. A free-time turn shows the agent its own
material — todo items, projects, open questions, open expectations,
integrity facts, recent traces — in creation order, never ranked. The
agent picks. Only the agent's tools change the mode:

| tool | effect |
|---|---|
| `commit` | mode → Committed; one commitment at a time |
| `progress` | the next committed header shows the agent's own next step |
| `release` | mode → Free; a natural break |
| `trace` | ends a free session; a retain candidate |
| `want_tools` | a request; the Tools family decides at the next boundary |

The owner's one override is a release (`released_by: owner`). The host
never times a commitment out; its header states facts (turns committed,
turns since progress, an `until` passed).

A persistent copy loop (a trace or answer over 0.8 trigram Jaccard
similarity to a recent one on three consecutive turns) triggers a
mechanical context intervention: an early rollover and one owner message.
It changes the context, never the topic.

## The decision desk

Five families decide the host's mechanics, each with bounded state, atomic
Noul and Choice questions (`rung_std::decide`), composition in code, hard
bounds, a rule fallback and a logged provenance:

| family | decides | guard that always wins |
|---|---|---|
| Admit | interrupt now, wait for a break, or show as a digest line | owner items are `now` without asking; firm due calendar items are `now`; every kind has a maximum deferral; at most 6 interruptions of a commitment per hour |
| Inject | recall block and cue; expectation and calendar digests | one recall block per turn; everything goes in the newest header |
| Tools | which groups are callable | `core` is always on; never outside the operator ceiling; hysteresis |
| Pack | append, roll over now, or at the next break; which segments stay verbatim | roll over at 85% of the epoch budget; never below 40% (25% at a natural break) except on a copy loop; kept text ≤ 15% of the budget |
| Consolidate | whether to offer the agent a note line; what to retain | only the agent's own text and host observations are retained |

The backend is any `Decider`: Jev, `Recorded` replay, or the `Scripted`
test decider. A decider that is unavailable, times out (1.9 s, inside the
2 s boundary budget), answers `Undecided`, or would break the spend cap
gives way to the rule, and the record says why (`by: {"rule": "timeout"}`).
`Shadow` mode lets the rule decide and logs the decider's answer beside it.

## The pack

```text
STABLE   tool superset (canonical JSON, sorted) · system: identity, host contract, free-time rules · pinned memory
SLOW     epoch header (id, start, gap, model) · carried note · register digest · previous-epoch outline + kept segments
LOG      turn k header · the turn's messages, verbatim · turn k+1 header · …   (append-only within the epoch)
```

- Every request inside an epoch is a byte-prefix extension of the one
  before (gate G-l). The time and other volatile facts ride in each turn's
  header, at the end.
- Tools are enabled by a host-side call gate; their definitions never
  change, so the cached prefix holds on every provider. A disabled tool's
  call returns `{"error":"not enabled this turn","group":…,"ask":"want_tools"}`.
- A rollover starts a new epoch (and a new session id). It rebuilds the
  slow layer from the record; nothing in the stable layer changes. The
  record is fsynced before anything is evicted.
- A model switch on the ladder also starts a new epoch.

## ACP outward

There is one agent; `session/new` opens a **channel** to it — owner, peer
or observer (`_meta.rung.role`; a peer or observer names itself with
`_meta.rung.channel`, becoming `peer:<name>`). The transport's principal
caps the role (`rung-host sim --acp [--acp-role …]` serves one local
client on stdio as the operator's configured role, owner by default). Each
channel is a `channel.opened` record line, so `session/list` and
`session/load` survive a restart.

- `session/prompt` is a stimulus from that channel, recorded and fsynced
  before anything else. It is answered when the turn that disposes it
  ends: the agent's `send`s to that channel in that turn stream as
  `agent_message_chunk`s (the turn's final text when it sent none), the
  turn's tool calls as `tool_call`s, and `_meta.rung` names `{item, turn,
  disposition, status, admitted_with}`. An observer cannot prompt.
- A channel sees only output of work it owns. When a turn admitted items
  (prompts or stimuli) from several channels, the final text and tool calls stream only to the
  highest-role channel among them (a host item owns no turn); if that
  channel's item is a no-reply stimulus, no prompt gets them (owner > peer > observer; a tie goes to
  every session of that one channel); any other channel gets only what the
  agent explicitly `send`s to it, and `admitted_with` lists only item ids of
  the answered channel. This is the safe default; loosening it is a
  deliberate decision.
- An agent-initiated message (a send with no open prompt from that channel
  in its turn) is an `_rung/outbox` notification `{sessionId, channel,
  text, turn, source}` to clients that opted in at `initialize`
  (`_meta.rung.outbox: true`), otherwise the head of the channel's next
  response.
- `session/cancel` withdraws a pending stimulus (`stimulus.disposed`
  `withdrawn`); one already in the running turn is answered `cancelled`,
  and an owner's cancel also cuts that turn. A halt answers every open
  prompt `cancelled` with `_meta.rung.halted`.
- Extensions: `_rung/status` (the now set: turn, mode, project, epoch,
  rung, model, ladder availability, quota, degraded, desk; any role);
  `_rung/stimulus` (a stimulus that asks no reply; owner or peer; on disk
  before its ack); owner only: `_rung/stop`, `_rung/release`,
  `_rung/calendar` (`{id, at | in_s, text, firm}`).

The bridge reads the record through an observer hook and decides nothing
the loop decides.

Two transports serve it. `--acp` serves one local client on stdio.
`--acp-http ADDR` serves Streamable HTTP (`rung-agent-core`'s stack,
loopback by default) with one operator bearer token per role, each read
from the env var a `--acp-token-env ROLE=ENV_VAR` flag names (a token never
rides the command line or reaches the record; two roles may not share a
token value, exit 2). A request without a known
token is refused (401); a connection's principal is the role of the token
it initialized with, and a later request on it with another token is
refused (403). Without at least one token the HTTP surface does not
start.

## The startup-handoff ladder

`rung-host run --config rung-host.yaml` starts a host through a ladder:

```text
Configured(Plan) => Listed(Plan) => Recovered(Opening) => { Handed(Handoff) | Refused(Refusal) }
```

- **Configured**: the file is read (unknown fields refused), checked, the
  keys read from the env vars it names, the engine built. A refusal
  (exit 2) names the problem and touches no state.
- **Listed**: the router's models are listed at start, before the record
  is opened. The host records that listing (`ladder.listed` with
  `at_start: true`) at its first boundary, before its first turn; a failed
  listing does not stop the start.
- **Recovered**: the record is opened and replayed; a restart recovers
  here.
- **Handed** to the Presence loop, with ACP outward when configured; or
  **Refused** when the record cannot be opened.

Each stage is a rung: mid-ladder tokens have no public constructor, and a
`Plan` is built only by configuring, so no stage can be skipped or forged.

```yaml
state: /var/lib/rung-host            # required; workspace defaults to <state>/workspace
engine:
  kind: agent                        # or mock
  base_url: https://openrouter.ai/api/v1
  api_key_env: OPENROUTER_API_KEY    # the env var's name, never the key
  reasoning: medium                  # pinned for the agent's life
ladder: [ ... ]                      # default: the ruled free ladder
listing: true                        # list at start and every 6 h (default for agent)
probe: true                          # with each listing, one keyed probe per standing rung (default with listing)
quota: { rpd: 1000, rpm: 20 }        # optional
memory: true                         # baseline memory under the state dir
acp: { http: "127.0.0.1:7878", tokens: { owner: RUNG_HOST_OWNER_TOKEN } }   # or { stdio: owner }
```

Other optional keys: `workspace`, `identity`, `owner_channel`,
`epoch_budget_tokens`, `turn_bound_s`, `backoff_base_ms`, `seed_projects`
(`[{id, title, why}]`), and under `engine`: `step_cap`, `timeout_s`.

For a bounded run with real stimuli:

```yaml
inbox: /run/rung-host/inbox          # a *.msg directory source
stop_file: /run/rung-host/STOP       # the stop authority also halts on this file
run_for_s: 1800                      # stop at 30 min (a wait ends there too)
calendar:                            # owner entries, seeded on the first start only
  - { id: standup, in_s: 600, text: "Stand-up: say what you are on", firm: true }
desk:                                # rule-only when absent
  mode: shadow                       # decide | shadow | rule_only
  decider: jev                       # System One at base_url (default OpenRouter)
  api_key_env: OPENROUTER_API_KEY    # the env var's name, never the key
  cap_usd_day: 0.25                  # default 0.25; per ask 0.001 (cap_usd_ask)
  kill_file: /run/rung-host/DESK_OFF # while it exists the decider is not asked
```

With the kill file present every ask is `desk.ask{outcome: killed}` and
every family decides by its rule (`by: {"rule": "killed"}`). An ask that
timed out or came back undecided counts its estimate against the cap, as
it may have been billed without saying so.

## The engine adapter

`rung_host::adapter::AgentEngine` runs each turn on `rung-agent-core`'s
`Engine`, against an OpenAI-compatible route (a router, or a local
server):

- the host's toolset (the stable superset and its call gate) is the
  turn's whole toolset; the engine's own roster is empty;
- the caller owns the thread: the turn gets the pack's thread and gives
  back what it added, verbatim — nothing the host sent is shortened or
  rewritten. If the engine had to elide old tool results after a context
  overflow (its last resort), `turn.ended` says `rewritten: true`;
- each call carries the turn's model (the ladder's rung), the epoch's
  session id (`session_id`, a router's sticky-routing key) and two explicit
  cache breakpoints, at the end of the stable layer (the system text) and
  of the slow layer (the first message). The reasoning effort is pinned
  (`medium` by default) for the agent's life;
- every call is recorded as served (`llm.call`): the model, usage, cached
  and cache-write tokens (`prompt_tokens_details`), cost, and latency on
  the host clock. `provider` is the route's host: a stream does not name
  the provider behind a router;
- a 429 is the platform's when it carries `X-RateLimit-*` and no upstream
  provider metadata (its `X-RateLimit-Reset` becomes `reset_at`; a body too
  truncated to parse still counts as carrying it if it names
  `provider_name`/`provider_code`); otherwise
  it is the provider's. Only a provider's steps the ladder down.

The key is read by the caller from the environment variable its
configuration names; it never reaches the record.

## The model ladder

The ladder is configuration: model ids, best first
(`rung_host::ladder::OPENROUTER_FREE_LADDER` is the free ladder the
operator ruled for a first live substrate). At the first boundary and
every six hours the host lists the router's models — keyless GETs of
`{base}/models` and `{base}/models/{id}/endpoints` — and keeps a rung only
when it is listed, its `expiration_date` has not begun (that date is the
first day it is gone), its prompt and completion prices are zero, it takes
`tools`, and one of its endpoints has a status of at least 0. The verdicts
are one `ladder.listed` line. The walk skips the rest; a listing that takes
the current rung away switches to the best standing rung at once (a
`model.switch` whose `why` starts `listing:`), which starts a new epoch. A
listing that fails keeps the previous verdicts and is tried again in 15
minutes; it never stops the host. If no rung stands, the host keeps its
current one.

A rung the router refuses for this account (a 404 naming
`ineligibility_reasons`: data policy, guardrails) is unavailable until the
next successful listing, exactly like a rung the listing dropped: one
`ladder.refused` line, and no step-down or probe lands on it. A failed
listing keeps it unavailable. If nothing below a refused current rung
stands, the host switches to the best standing rung (a `model.switch`
whose `why` starts `refused:`).

With every successful listing — so at start and every six hours — the host
also probes, with the route's key, each rung the listing left standing (so
only free models): one tiny chat request (one user word, at most one output
token, no tools). A refusal for this account is a `ladder.refused` line
(`by: probe`) before any turn can land on the rung; every verdict
(`routes`, `refused`, `unknown`) is one `ladder.probed` line, and each
probe counts as one request against the day's quota. `probe: false` turns
probes off; with `listing: false` there are none.

## The governor

- **Pacer**: a daily request quota (`rpd`), a per-minute limit (`rpm`), a
  share reserved for Responding turns (25%), released linearly over the
  UTC day. Waits are `degraded: paced`, interruptible by stop and by an
  owner item the reserve can serve.
- **Backoff**: a provider 429, 5xx, transport failure or timeout waits
  max(`Retry-After`, jittered exponential), capped at 15 min, and steps
  down the model ladder to the next rung the listing left standing
  (cooldown 2 min doubling to 30 min); the next boundary after the cooldown
  probes back up to the nearest standing rung above that has cooled down
  (a rung still cooling does not hide a cooled one above it). A platform 429
  (`X-RateLimit-Reset`) waits for its reset and does not step down. A
  router 404 that names why the model's endpoints were excluded for this
  account (data policy, guardrails: `ineligibility_reasons`) is
  `unroutable`: the rung is unavailable until the next listing (above),
  and the walk steps past it after the base backoff. A refusal does not
  grow the backoff, since it says nothing about a provider's load. An auth
  failure is `degraded: blocked`: probe every 15 min, one owner message per
  incident, never exit.
- **Owner acknowledgement**: during a wait the owner cannot cut (a
  provider backoff, a quota, a blocked credential), each waiting owner item
  gets one line from the host at once, with no model call: it was received,
  the provider is unavailable (why), and when the host expects to answer
  (`outbox.queued`, `source: host:ack`, `item`). The agent's real answer
  follows when a turn can run.
- **Spend cap**: for paid providers only; reaching it halts
  (`Halted(SpendCap)`).

## Stop and supervision

`StopAuthority` takes SIGTERM/SIGINT, a stop file, or an explicit stop. It
is checked at each boundary, raises the running turn's cancel flag, and
every wait polls it. `sd_notify`: `READY=1` at start, `WATCHDOG=1` from the
loop thread at each boundary and during waits, `STOPPING=1` on halt. A
wedged loop misses its pings and the supervisor restarts it; the record
makes any kill recoverable.

## Durability and restart

The record is the truth; every register is a projection of it. On waking,
the host replays the record, cuts a torn last line, requeues any stimulus
that was admitted to a turn that never ended (`interrupted_by_restart`),
fires calendar items missed during the gap once (late), restores the mode
from the `kernel.*` lines, and starts a new epoch whose header states the
gap: *not running from T1 to T2*. Each `turn.ended` line carries the hash of
the projection as it stood before that line, so a replay can prove the
rebuilt state equals the state the live process had.

## The record

One NDJSON line per event, canonical JSON (sorted keys), `{seq, at, kind,
…}`; `at` is milliseconds since the Unix epoch on the host clock. Fields
named `wall_*` are wall-clock measurements and differ between runs.

| kind | holds |
|---|---|
| `host.start` | `pid`, `config` (`engine`, `turn_bound_ms`, `epoch_budget_tokens`, `ladder`, `ceiling`, …) |
| `recovered` | `gap_ms`, `last_at`, `torn_bytes`, `requeued`, `mode` |
| `boundary` | `n`, `mode`, `pending` |
| `stimulus.accepted` | `item {id, kind, role, channel, at, due?, urgency?, firm, text, fact?}` |
| `stimulus.admitted` | `turn`, `boundary`, `ids` (shown now), `digests` (one header line each) |
| `stimulus.requeued` | `ids`, `why` |
| `stimulus.disposed` | `id`, `disposition` (`answered`, `digested`, `withdrawn`, `control`), `turn` |
| `channel.opened` | `session`, `role`, `channel` (an ACP channel) |
| `stimulus.rejected` | `file`, `why` |
| `calendar.added` / `.fired` / `.skipped` / `.removed` | an entry; a fire has `id`, `due`, `late_by_ms`, `missed`, `firm`, `item_id` |
| `decision.<family>` | `boundary`, `turn`, `input_hash`, `questions_hash`, `answers`, `choice`, `by` (`{"jev": {backend, model, cost_usd}}` or `{"rule": why}`), `rule_choice` and `agree` in shadow mode, `wall_us` |
| `turn.started` | `turn`, `turn_kind`, `mode`, `project`, `model`, `rung`, `epoch`, `pack_tokens`, `header_tokens`, `enabled`, `wall_boundary_us` |
| `tool.call` / `tool.refused` | `turn`, `name`, `group`, `ok`; a refusal has `why` (`disabled`, `overran`, `owner_waiting`) and `message` |
| `llm.call` | `turn`, `call`, `epoch`, `rung`, `model_requested`, `model_served`, `provider`, `prompt_tokens`, `cached_tokens`, `cache_write_tokens`, `completion_tokens`, `reasoning_tokens`, `cost_usd`, `latency_ms`, `prefix {s_hash, l_hash, log_len_bytes, expected_cached_tokens}` |
| `cache.break` / `cache.cold` | `turn`, `call`, `cause` |
| `turn.log` | `turn`, `header` (recall stripped), `messages` (verbatim, as sent) |
| `turn.ended` | `turn`, `turn_kind`, `status`, `calls`, `elapsed_ms`, `rung`, `failure {class, origin, retry_after_ms, reset_at, unroutable}`, `rewritten` (only when true), `cost`, `projection`, `wall_post_us` |
| `kernel.commit` / `.progress` / `.release` / `.trace` | the agent's own tool calls (`via`, `released_by`, `similarity`) |
| `note.written`, `todo.*`, `project.added`, `question.*` | the agent's registers |
| `expectation.made` / `.revised` / `.settled` | the expectation register; a settlement has `state`, `p`, `surprise`, `settled_by`, `calibration` |
| `outbox.queued` | `channel`, `text`, `source` (`agent`, or the host's: `host:blocked`, `host:ack` with the owner `item` it acknowledges) |
| `tools.wanted` | `group`, `why` |
| `memory.recall` / `memory.retain` | the provider's report |
| `degraded` / `degraded.ended` | `class` (`paced`, `quota`, `backoff`, `blocked`), `until`, `why`; `waited_ms` |
| `model.switch` | `from`, `to`, `direction` (`down`, `up`), `why` (`provider …`, `probe: …`, `listing: …`) |
| `ladder.refused` | `rung`, `model`, `reasons` (the router's `ineligibility_reasons`), `by` (`probe` when a keyed probe found it): unavailable until the next listing |
| `ladder.probed` | `probes`, `rungs [{rung, model, verdict, reasons?, error?}]` (`verdict`: `routes`, `refused`, `unknown`) |
| `ladder.listed` | `ok`, `error` (when not), `at_start` (the startup ladder's listing), `rungs [{rung, model, available, why}]` (`why`: `ok`, `not_listed`, `expired`, `not_free`, `no_tools`, `endpoint_down`; `kept` / `kept_unavailable` after a failure), `available`, `next_at` |
| `epoch.rollover` / `pack.swap` | `from`, `to`, `cause`, `by`, `kept`, `tokens_before`, `l1`, `gap_ms` |
| `copy.guard` / `copy.loop` | the copy guard's flag; the intervention |
| `halted` | `why` |

## Gates (slice 1)

The gates are frozen in `rung-host/src/gates.rs` (thresholds and
evaluators) and run by `rung-host/tests/`. All are automated, seeded and
free.

| id | measure | pass when |
|---|---|---|
| G-a no rest | 30 min, no stimuli, no quota pressure | idle wall time outside turns (excluding declared `degraded`) ≤ 2%; boundary → next turn p99 ≤ 50 ms; every boundary has `decision.*` lines |
| G-b responsiveness | owner stimuli under load | admission p95 ≤ p95 turn + 100 ms; no turn past its bound; scripted long work refused with the commit-or-steps message |
| G-c free-time kernel | 2,000 scripted turns | no admitted item and no commitment → free turn, 100%; after `commit`, committed until `release` (interruptions return); material order identical under 100 random register scores; no `kernel.commit`/`kernel.release` except from the agent's tools or the owner's release (trybuild) |
| G-d schedule | due and missed items | each due item fired at the first boundary after due, once, lateness recorded; items missed during downtime fired once; firm items admitted at that boundary regardless of the decider |
| G-e expectations | scripted expectations | only the host (or a disjoint judge) settles (trybuild); surprise and calibration recomputed from the record match exactly |
| G-g provider failure | the injected faults | never exits; requeued items admitted exactly once; `Retry-After` honoured; provider 429 → step down, later probe up; platform 429 → wait for reset, no step down |
| G-h stop | SIGTERM mid-turn, in backoff, in a paced wait; a wedged loop | ≤ remaining mock call + 1 s; ≤ 1 s from a wait; the watchdog fires |
| G-i restart | 50 random `kill -9` | every stimulus exactly one disposition; rebuilt projections equal the live ones; mode restored; a new epoch with the gap |
| G-j bounded context | 10,000 turns | the pack never exceeds the epoch budget; every turn starts at ≤ 85%; the record grows linearly; the copy guard fires |
| G-l cache discipline | 10,000 turns | within an epoch each request is a byte-prefix of the next; `s_hash` changes only at a swap, `l_hash` only at a rollover; canonical bytes stable across 2 processes; mock cache efficiency ≥ 0.98 outside recorded breaks |
| G-m decisions | scripted answers, every `Undecided`, 3 s delays, cap exhaustion | every family returns a choice on every path with the right `by`; guards hold under adversarial answers; a 3 s delay costs ≤ 2 s per boundary; `Recorded` replay panics on a reworded question |
| G-k cost | the whole slice | $0: loopback only, no live model, no live Jev |

## Gates (slice 2)

Frozen in the same file, below slice 1's, before the first run of what each
gates. Every slice-2 run is loopback-only: a scripted provider on 127.0.0.1
(`rung-host/src/sim/http.rs`) answers in the OpenAI-compatible wire shape a
router documents. No live model, no live Jev, no key.

| id | measure | pass when |
|---|---|---|
| G-n engine adapter | the host on `rung-agent-core`'s engine through the adapter, 24 turns against the loopback provider | every request loopback and every `llm.call` served by it; each request asks for its turn's model with its epoch as `session_id`; exactly two cache breakpoints, at the stable and slow layers' ends; inside a session each request extends the previous (same tools, previous messages a prefix; a last step without its closing instruction); a long tool result seen again unchanged; every `llm.call` carries the served usage, cache and cost; the model's tool calls ran through the host (a note written, a disabled tool refused); a provider 429 recorded as the provider's and stepping down, later probing up; a platform 429 recorded as the platform's, waiting for its reset, no step down; $0 |
| G-o ladder listing filter | 13 simulated hours on a seven-rung ladder against recorded-shape listing fixtures: one rung expires, one endpoint is down, one takes no tools, one is unlisted, one is paid; one refresh fails | listed before the first turn; each listing's verdict per rung is the oracle's (listed, not expired, prompt and completion free, takes `tools`, an endpoint at status ≥ 0); the expiry seen; a failed listing keeps the verdicts and is retried within 15 min; refreshed within 6 h; every turn on an available rung; a rung taken away is switched off before the next turn; a provider 429 steps down to the next available rung (skipping one), a probe up to the nearest available; listing GETs keyless and each on record; loopback, $0. **Amended 2026-10-04 (owner's ruling):** one rung is refused by the router for the account; it is unavailable until the next listing — no turn, step or probe lands on it, a failed listing keeps it unavailable, and a refused current rung is switched off before the next turn; at least one refusal seen |
| G-p ACP outward | the binary on stdio (`sim --acp`, real clock, mock engine) and one JSON-RPC client: owner, peer and observer channels; a no-reply stimulus; prompts from two channels; a cancel; owner-only extensions from a peer; the owner's calendar entry; status, list, load; the owner's stop with a prompt open | every prompt exactly one response; the observer's refused; an answered prompt names the item and turn the record disposed it in, its streamed text ends with the agent's sends to that channel in that turn (or, for the highest-role channel in the turn, the final text), its turn's tool calls streamed; the final text and tool calls only to the highest-role channel admitted in the turn, no other channel's output or item ids to any other channel; a cancelled prompt answered `cancelled`, its item withdrawn or already in a turn; an agent-initiated message delivered as `_rung/outbox`, matching the record; `_rung/stimulus` on disk before its ack; the peer's stop, calendar and release refused; the owner's calendar entry added and fired; status reports the now set; list shows every channel, load reopens one; the owner's stop halts the host, answers open prompts, exit 0 within 5 s; mock engine, $0 |
| G-q ACP over HTTP | the binary on Streamable HTTP (`sim --acp-http 127.0.0.1:0`, owner and peer tokens from env vars) and a raw HTTP client; refused starts without usable tokens | no or an unknown token refused 401; a token's role caps its channels (the peer token cannot open an owner channel); another valid token on an initialized connection refused 403 for POST, GET and DELETE; a peer prompt answered `end_turn` naming an item the record disposed in that turn; the peer's stop refused, the owner's halts and exits 0 within 5 s; no tokens or an unset token env var refuse to start (exit 2); no token in the record; bound to loopback |
| G-r startup-handoff ladder | `rung-host run --config FILE` against the loopback router: a first run, a restart on the same state, an unknown config field, an unset key env var, an unreachable router; and compile-fail cases | each process lists the models at start, before its record exists, and records that listing (`at_start`) before its first turn; the restart recovers the record and lists again; turns run on the real adapter, $0; a bad field or unset key refused (exit 2) naming it, the state untouched; an unreachable router does not stop the start (listing recorded failed, turns fail and back off, ends by its limit); no stage can be skipped (recovering takes a `Listed`, not a `Configured`) and no mid-ladder rung, plan or handoff can be built from outside |
