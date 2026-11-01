---
type: repository guide
title: Rung Repository Guide
description: A task-routing map for the Rung Rust workspace and observer UI. Use it to identify the owning layer, follow the detailed guide for that concern, and select the validation that protects its boundary.
tags: [repository-guide, workspace, routing, rust, ui]
verified:
  - by: openwiki/0.7.2
    at: 2026-11-01T15:22:53.269Z
sources:
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-bf8080eb6a0133dfe91012a2
    resource: repo://rung-agent-core/src/lib.rs
  - id: openwiki-source-75bb82ce061dabeefb69db8e
    resource: repo://rung-gateway/src/lib.rs
  - id: openwiki-source-d32a2f8573bc46003958a6af
    resource: repo://rung-host/src/lib.rs
  - id: openwiki-source-4f9b2aa7f3c649a70bea719b
    resource: repo://rung/src/lib.rs
  - id: openwiki-source-60cfb4a8f7e8499e43bc3c0c
    resource: repo://ui/README.md
generated: { by: "openwiki/0.7.2", at: "2026-11-01T15:22:53.269Z" }
---

# Rung Repository Guide

Start here to decide **where a change belongs**, not to learn every subsystem. Rung has a reusable type-state kernel and product crates that build an agent, persistent host, gateway, and browser observer around it. Put a concern at its owner rather than copying behavior across layers; then use the linked guide for the lifecycle, contract, and focused tests.

## Choose your destination by the change

| If you need to change… | Start here | Owns the decision |
| --- | --- | --- |
| `ladder!`, generated token guarantees, recovery rules, provenance gates, or generic audit dispatch | [Type Ladders, Theory Gates, and Audit Dispatch](concepts/type-ladders-and-heteronomy.md) | The `rung`/`rung-macro` kernel, `rung-het`, and generic supporting crates |
| Crate placement, public entrypoints, or whether a mechanism is kernel or product | [Workspace Architecture and Ownership Boundaries](architecture/workspace-boundaries.md) | The workspace dependency direction and consumer-neutrality rule |
| A task-agent turn, session, tool scope, MCP, memory hook, ACP-facing behavior, or agent status | [Agent Turns, Sessions, Tools, and Memory Hooks](workflows/agent-turns-and-sessions.md) | `rung-agent-core`; keep `rung-agent` as its thin CLI shell |
| A continuous agent’s start/recovery, durable record, queue, model selection, scheduling, stimuli, or stop behavior | [Continuous Host Lifecycle, Record, and Queue](operations/continuous-host.md) | `rung-host` |
| Multi-host routing, host-door access, per-instance credentials, SSE forwarding, native record reads, or the live/offline observer | [Host Gateway, Doors, and Observer UI](integrations/gateway-and-ui.md) | `rung-gateway` at the HTTP edge and `ui/` at the browser edge |
| API-key environment names, OAuth, XDG/local state, configuration precedence, or safe runtime setup | [Runtime Configuration, Credentials, and Local State](operations/configuration-and-secrets.md) | The configuration-owning product crate; never a committed secret |
| Which refusal, integration, mock-process, contract, UI, documentation, or CI check should prove a change | [Verification Strategy and CI Gates](testing/verification-and-ci.md) | The boundary the change can break |

The detailed pages are intentionally separated: an agent change often affects host embedding, and a host-door change can affect gateway and UI contracts, but that does not make the gateway the owner of host state or the UI the owner of the protocol.

## Read the hierarchy before editing

1. **Kernel first when the invariant is generic.** `rung-macro` implements `ladder!`; `rung` supplies its runtime surface and re-exports that macro, so consumers depend on `rung`. The kernel’s job is to make permitted state transitions and their evidence compile-time constraints, not to own sessions, credentials, or deployment behavior.
2. **Use product crates for operational state.** `rung-agent-core` is the embeddable agent library; `rung-agent` is the argv-to-exit-code shell. `rung-host` embeds the core turn engine and owns the long-running lifecycle around it. Do not make the host shell out to `rung-agent` merely to run a turn.
3. **Treat the gateway as an edge, not a second host.** It fronts multiple hosts with one listener, retains instance keys, applies roles, and proxies host `/v1` doors. The observer app is a separate npm/TypeScript deliverable in `ui/`, not a Cargo workspace member.
4. **Keep dependencies flowing outward.** The workspace’s reusable harness must not acquire consumer-specific names, types, configuration, fixtures, or dependencies. Product-specific policy belongs in an adapter or product crate; generalize only when the abstraction is genuinely caller-neutral.

For the complete ownership table and public executable/library boundaries, continue with [Workspace Architecture and Ownership Boundaries](architecture/workspace-boundaries.md).

## A safe change path

1. **Classify the concern.** Pick the row above by the state or external contract that changes. If a change crosses a boundary, identify the owner and every consumer that must be checked.
2. **Trace through the owner’s public seam.** For example, trace an agent behavior from `rung-agent-core`’s engine/job split, a host behavior from its record-backed lifecycle, or a browser behavior from the gateway/host door contract—not from a convenient UI rendering alone.
3. **Preserve boundary invariants.** Type ladders use compile-time refusal; agent sessions and host records preserve durable history; the gateway keeps instance keys out of clients and enforces read-only access before proxying; the UI follows the published read/stream contract.
4. **Prove the affected boundary.** Start with the narrow crate, refusal, integration, or contract test, then run the appropriate workspace gates. [Verification Strategy and CI Gates](testing/verification-and-ci.md) maps representative commands to the failure modes they cover.

## Entrypoints at a glance

These commands select product boundaries; consult the target page before changing their behavior or operational configuration:

```sh
cargo run -p rung-agent -- ...
cargo run -p rung-host -- run --config rung-host.yaml
cargo run -p rung-gateway -- --config gateway.yaml
cargo run -p rung-doctrine --bin render -- --check
```

The repository charter in [AGENTS.md](../AGENTS.md) is the authoritative working agreement: generated proposition documents are not hand-edited, credentials are machine-local and referenced by environment-variable name, and the workspace checks include formatting, strict linting, tests, rendered-document verification, repository guards, and rustdoc. For UI work, enter [`ui/`](../ui/README.md) and run its npm checks as well; the UI is outside Cargo and its Node checks are not implied by a Rust-only test run.

## Quick handoffs

- **“I need a new workflow state or stronger transition guarantee.”** Read [Type Ladders](concepts/type-ladders-and-heteronomy.md), then its refusal tests via [Verification](testing/verification-and-ci.md).
- **“I need to change what an agent does in one turn.”** Read [Agent Turns and Sessions](workflows/agent-turns-and-sessions.md); follow the host guide too if the host supplies the thread, tools, or persistence context.
- **“I need a durable queue or restart behavior.”** Go directly to [Continuous Host](operations/continuous-host.md), whose record is the durable authority.
- **“I need a browser/API change.”** Read [Gateway and UI](integrations/gateway-and-ui.md) first, then run the relevant Rust gateway and TypeScript contract checks described in [Verification](testing/verification-and-ci.md).
- **“I need to add a setting or credential.”** Start with [Configuration and Secrets](operations/configuration-and-secrets.md), keeping secret values out of configuration and source.

When uncertain, favor the lowest layer that can express the concern without learning a consumer’s identity, and validate from the public boundary outward.
