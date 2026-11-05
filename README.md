# rung

A type ladder for Rust: you declare the rungs of a state machine and the transitions between them once, and the compiler refuses any path that skips a rung. On top of that kernel sit product crates that run agents as long-lived, steerable, stoppable processes: an agent CLI, a host, an HTTP gateway and an early fleet supervisor.

> **Status: pre-release.** rung is not published to crates.io or any other registry, and it stays that way; build from source. APIs may still change.

```
 ladder! / theory!  ──►  rung-std blocks  ──►  rung-agent-core  ──►  rung-agent  (task CLI, ACP)
 (the kernel: typed      (llm, agent, tools,    (catalog, sessions,   rung-host   (one long-lived agent)
  lifecycle, no          questions, driver)      turn engine, MCP)    rung-gateway (one door for many hosts)
  skipped steps)                                                      rung-fleet  (chief, stewards, crew)
```

## Why it exists

An agent is not a function call. It runs for hours, takes owner messages while it works, gets paused, steered and stopped, crashes and comes back. Most agent code models this with flags, status strings and convention: a work item is "claimed" because a field says so, and nothing stops code from acting on an item that was never claimed. The state machine lives in comments and review.

rung puts it in the types. A work item moves `Pending → Running → Done`, and the only way to hold a `Running` value is to go through the transition that produces it. A skipped step, a token dropped on an error path, or a match that forgets a verdict is a compile error, not a bug found in production. The products built on it keep the same discipline at run time: what an agent did is an append-only record you can replay, and a stop is a stop.

## Core ideas

- **The ladder.** `ladder!` declares rungs, transitions and recovery edges. Tokens move by value, are `#[must_use]`, and mid-ladder constructors are sealed, so a value is evidence that the path to it was taken. `theory!` declares sentences that a machine settles, or that an outside party settles when no machine can. The kernel is the crate `rung`.
- **The agent.** `rung-agent` is a task CLI over `rung-agent-core`: a catalog of endpoints, resumable sessions, a turn engine, MCP tools, and ACP (agent client protocol) over stdio or HTTP. It talks to any OpenAI-compatible route.
- **The host.** `rung-host` is one long-lived agent and everything around its turns: an inbox for owner, peer and observer messages, idle and continuous modes, a governor for pacing and spend, and a stop authority. Everything it does is written to a replayable record; restarting it replays the record and continues.
- **The gateway.** `rung-gateway` is one listener in front of any number of hosts. It serves the UI, lists instances and forwards requests, adding each instance's key itself, so no page or response carries a key. It has a read-only token role.
- **Receipts and authority.** Delivering a message to a host returns a typed receipt, idempotent by correlation id. Only the principal's grant authorises a spawn, an assignment or a stop; a one-time grant is spent once.
- **The fleet.** `rung-fleet` supervises many hosts in three tiers: a chief, stewards, and crew that work on one task and are retired. The delegation lifecycle (spawned, assigned, active, done, retired, with held and stalled as recoverable states) is itself a ladder. This crate is the youngest part.

## Quickstart

You need a Rust toolchain (1.88 or newer). From a clone:

```sh
cargo build -p rung-host -p rung-agent
```

**The ladder.** A complete program, compiled and run as a doctest, is in the crate docs of `rung/src/lib.rs`, and there are four graded examples:

```sh
cargo test -p rung --doc
cargo run -p rung --example 01_enforced_graph
```

**The agent.** `rung-agent` needs a model endpoint and a key (see [the agent guide](docs/guides/rung-agent.md)); without credentials it says so and exits:

```sh
target/debug/rung-agent --help
target/debug/rung-agent --json --tools none "Reply ok."
```

**The host, against a mock.** No network and no key. Run it from a scratch directory; `RUNG_HOME` keeps its instance registry out of your home:

```sh
mkdir -p scratch/demo/inbox scratch/demo/sandbox
cp examples/rung-host-mock.yaml scratch/
echo '{"role":"owner","text":"Hello, what are you working on?"}' > scratch/demo/inbox/hello.msg
cd scratch
RUNG_HOME=$PWD/home ../target/debug/rung-host run --config rung-host-mock.yaml --turns 3
jq -c '{seq, kind}' demo/state/record/*.ndjson | head
```

The mock engine answers deterministically, so this shows the plumbing (inbox, record, stop) rather than model quality. Run it again and the host replays the record and continues. The [host quickstart](docs/guides/rung-host-quickstart.md) walks through the record and stopping.

## Status

Unpublished, and pre-1.0 (the workspace is at 0.3). Nothing here is on crates.io.

- **Solid:** the kernel (`ladder!`, the sealed-token guarantees, compile-fail tests pinning the refusals), the standard blocks, the agent library and CLI, and the host with its record.
- **Early:** the gateway and the UI, and the fleet, which is growing phase by phase against written propositions ([fleet props](docs/rung-fleet-props.md)). Expect change there.
- **Not yet true:** the repository measures how many defects its audit-and-rectify loop has found in rung itself; that number is still zero because no real judgment has been dispatched through it. Do not read the absence of findings as proof of correctness.

Normative documents are the `docs/*-props.md` files; the notes beside them are derivation. Where they disagree, props win.

## How it is developed

rung is built by coding agents under mechanical gates rather than line-by-line human review. Work lands directly on `master`, with no pull requests: a lander rebases onto the latest `master`, runs the fast gate (formatting, document guards, strict clippy and unit tests for the changed crates and what depends on them) and pushes a fast-forward. A slower tier runs integration tests, doctests, rustdoc and more every few hours. The rules are in the [landing section of AGENTS.md](AGENTS.md#landing-direct-to-master-no-prs-authoritative).

Outside contributions are welcome as pull requests against `master`. Keep them small, and do not rewrite history.

## Licence

Dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
