# rung-fleet R1: two hosts behind one gateway (result)

Record of the R1 run (witt3rd/rung#347), 2026-11-01. Design and claims are in
[`../../rung-fleet-notes.md`](../../rung-fleet-notes.md) and
[`../../rung-fleet-props.md`](../../rung-fleet-props.md); this page is the
measured evidence only. Host: x86-64, 20 cores, release build of rung at
f34bedb plus the R1 branch. Provider: a fake OpenAI-compatible server on
loopback (one fixed reply, configurable latency); no key, no login.

## The scenario

Four small scripts were used and are not kept in the repo (the repo's scripts
folder holds probes and audits, not run harnesses); each is described here.

- **setup**: writes a host file per member (`name`, `state`, `workspace`,
  `inbox`, `stop_file`, mock engine, memory off) and a gateway file with
  `registry_dir` and one `read_only_token_envs` entry.
- **scenario**: starts a chief and a steward as two ordinary
  `rung-host run --config` processes (agent engine against the stub, `quota`
  rpd 5000 / rpm 12), one `rung-gateway` with `registry_dir` and one read-only
  token, and a client that holds only that token.
- **measure**: starts N hosts against the stub for a 60 s window and samples RSS,
  CPU and record growth per host (and the gateway's RSS), with the stub's
  latency and each host's `quota` as the variables.
- **stub**: a one-reply HTTP server on loopback taking a port and a latency.

## What was shown

A chief and a steward are two ordinary host processes, each with its own state
directory (record), inbox and stop file, behind one gateway.

- `rung-host ls` lists `chief` and `steward-alpha` as `running`; the gateway
  `GET /api/instances` lists the same two (`source: registry`).
- A client holding only the read-only token: lists them, reads
  `/api/i/steward-alpha/v1/record?limit=3` (`host.start`, `pack.layers`,
  `epoch.rollover`), opens `/api/i/chief/v1/events?after=N` and, while it is
  open, an owner message dropped in the chief's inbox arrives as
  `stimulus.accepted`, `stimulus.admitted`, `stimulus.disposed(answered)`.
  The steward's record has 0 copies of that message, the chief's has 4.
- The same token POSTing to the queue is `403 read_only`; a wrong token `401`.
  `?token=` works for the event stream (a phone browser cannot set headers).
- `kill -9` of the steward (a pid started in that run): `rung-host ls` says
  `down`; restart: `running`, record shows `host.start`, `recovered`, new epoch.
  Stop file on both: `stopped`, both stay listed (also through the gateway).
- In CI form: `rung-gateway/tests/fleet_r1.rs` runs two chiefs and a steward as
  real hosts (mock engine) and checks the same list/read/stream/refuse/isolation
  facts. Mutation checked: writing the marker into another member's inbox turns
  it red.

The registry convention is **a name that begins with the role** (`chief-main`,
`steward-alpha`); the id is the slug, so the role is the first segment. No entry
field was added: it would be a second copy of what the name states. Props:
`a-member-is-named-by-its-role` (new), `a-read-only-role-watches-the-fleet`
(new), `chiefs-do-not-share-context` (owed to decidable). `an-observer-cannot-prompt`
stays owed (the ACP observer refusal is already tested in
`rung-host/tests/gate_acp.rs`; the owner takeover and reload halves are not cited).

## Unknown 2: resident cost of a host

Release build, memory on, `engine.kind: agent` against the stub. 60 s windows
(one core = 100%).

| hosts | provider / pacing | RSS per host | CPU per host | record growth per host |
|---|---|---|---|---|
| 1 | 0.3 s/call, no quota | 19.6 MB | 5.7% | 1.0 MB/min |
| 5 | same | 20.7 MB (103 MB total) | 5.4% | 1.0 MB/min |
| 10 | same | 20.6 MB (206 MB total) | 4.8% | 1.0 MB/min |
| 20 | same | 20.4 MB (408 MB total) | 5.0% (99% of a core in all) | 1.0 MB/min |
| 10 | 0.3 s/call, quota rpm 60 | 20.5 MB | 1.9% | 0.38 MB/min |
| 10 | 0.3 s/call, quota rpm 6 | 19.8 MB | 0.5% | 0.08 MB/min |
| 10 | 5 s/call, no quota | 20.1 MB | 0.5% | 0.07 MB/min |
| 3 (600 s) | 2 s/call, quota rpm 30 | 19.8 MB start, 20.2-21.4 end | 1.1% | 0.17 MB/min |

- A host with the mock engine and no HTTP engine: 12.9 MB RSS, 2 threads.
  Agent engine: 3 threads, about 10 fds. A host with `acp.http`: 15.5 MB, 26
  threads (the listener's pool).
- Gateway: 5.5 MB RSS and no measurable CPU in front of 10 instances.
- Memory is flat enough (+0.4 to +1.6 MB in 10 minutes, one soak) that N, not
  age, sets it. **N hosts cost about 20 MB x N.** 20 hosts is about 0.4 GB.
- What sets CPU, disk and (with a real provider) money is the **free-time pace**.
  A host with nothing to do keeps taking turns, so turns/min is the provider's
  speed unless `quota` (or a slow provider) holds it. The default ceiling is 600
  turns/min and is not configurable in the host file; `quota.rpm` is the knob.
  At a fast provider that is about 1.5 GB of record per host per day. A fleet
  config should give every member a `quota` from day one (R4).
- Stub replies are identical, so the copy-loop guard fires and rolls the epoch
  now and then: a stub artifact, not a cost finding.

## Gaps found in rung

1. No pace but the 600/min ceiling without a `quota` (above). Not changed:
   pace is policy; flagged for R4 (per-role defaults).
2. `clippy --workspace --all-targets -D warnings` failed on current stable
   (rustc 1.99): `drop_non_drop` in `rung-gateway/tests/context_memory.rs`.
   Fixed on a separate branch (one line).
3. The gateway start line printed `(0 instances)` for a registry-only gateway,
   and a gateway with no `registry_dir` lists nothing without a word. Fixed on
   a separate branch (`Settings::sources`, 3 tests).
4. The host's own HTTP door has no `/v1/summary` (404); the summary is the
   gateway's fold of the record. Documented in the notes, not changed.
5. The read-only token skips the Host check by design (a request with it from
   any Host is 200), so `allowed_hosts` protects the owner role only. Documented
   behaviour; a phone path over the tailnet relies on it.

## Not done

- A live login to a real provider (needs the owner); nothing here used one.
- Remote (a phone over an overlay network) was simulated by
  `Authorization: Bearer` over loopback; the Host rule for an overlay name is
  `allowed_hosts`, not exercised with a real one.
