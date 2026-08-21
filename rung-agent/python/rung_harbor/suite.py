"""Capability ladder for rung-agent, using Harbor *agent* tasks.

Harbor's examples/ tree mixes two things:

- Tasks that exercise an agent (write a file, run a command, follow a skill).
- Tasks that exercise Harbor (network policy, verifier isolation, CUA, CUDA,
  trajectory replay). Those tell us nothing about rung-agent.

This module is only the first kind, ordered cheap-to-dear. Terminal-Bench 2.0
is phase 2 — the real-world set — after this ladder is green.

Does not import Harbor.
"""

from __future__ import annotations

from dataclasses import dataclass


# OpenRouter alias. Harbor `--model` still wants the `openrouter/` prefix.
SUITE_MODEL = "openrouter/~deepseek/deepseek-v4-flash-latest"


@dataclass(frozen=True)
class Case:
    """One capability, one Harbor task path."""

    id: str
    capability: str
    tools: tuple[str, ...]
    path: str
    task_name: str
    skip: str | None = None


CASES: tuple[Case, ...] = (
    Case(
        id="write-file",
        capability="Create a file with exact contents",
        tools=("write_file",),
        path="examples/tasks/hello-world",
        task_name="harbor/hello-world",
    ),
    Case(
        id="shell-user",
        capability="Shell as a non-root user; two artifacts",
        tools=("shell", "write_file"),
        path="examples/tasks/hello-user",
        task_name="harbor/hello-user",
    ),
    Case(
        id="cwd-capture",
        capability="Capture command output into a file in cwd",
        tools=("shell", "write_file"),
        path="examples/tasks/hello-workdir",
        task_name="harbor/hello-workdir",
    ),
    Case(
        id="alpine-write",
        capability="Same write on a minimal (Alpine) image",
        tools=("write_file",),
        path="examples/tasks/hello-alpine",
        task_name="harbor/hello-alpine",
    ),
    Case(
        id="skills",
        capability="Discover and follow a SKILL.md",
        tools=("skill", "write_file"),
        path="examples/tasks/hello-skills",
        task_name="harbor/hello-skills",
    ),
    Case(
        id="multi-step",
        capability="Persist work across Harbor steps in one environment",
        tools=("write_file", "edit"),
        path="examples/tasks/hello-multi-step-simple",
        task_name="harbor/hello-multi-step-simple",
    ),
    Case(
        id="sidecar-http",
        capability="HTTP to a compose sidecar from the sandbox",
        tools=("shell",),
        path="examples/tasks/sidecar-artifacts",
        task_name="harbor/sidecar-artifacts",
    ),
    Case(
        id="llm-judge",
        capability="Unconstrained generation scored by an LLM judge",
        tools=("write_file",),
        path="examples/tasks/llm-judge-example",
        task_name="harbor/llm-judge-example",
    ),
    Case(
        id="mcp",
        capability="Call a tool on an MCP server",
        tools=(),
        path="examples/tasks/hello-mcp",
        task_name="harbor/hello-mcp",
        skip="rung-agent catalog has no MCP client",
    ),
)

SKIPPED_HARNESS = (
    "hello-cuda / computer-use / cua: GPU or desktop, not this agent",
    "hello-world-bat: Windows batch, not this agent",
    "hello-load-*: trajectory replay, not a live agent",
    "network-policy-* / verifier-mode-*: Harbor harness, not the agent",
    "environment-env-*: Harbor env injection, not the agent",
    "describe-image: vision; implement catalog has no image tool",
)

PHASE_2 = "terminal-bench@2.0"


def live_cases() -> tuple[Case, ...]:
    return tuple(c for c in CASES if c.skip is None)


def case_by_id(cid: str) -> Case:
    for c in CASES:
        if c.id == cid:
            return c
    known = ", ".join(c.id for c in CASES)
    raise KeyError(f"unknown case '{cid}' (want one of: {known})")
