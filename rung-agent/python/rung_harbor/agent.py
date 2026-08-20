"""Harbor `BaseInstalledAgent` wrapping the `rung-agent` binary.

The Python class runs on the host; the binary is uploaded into the sandbox.
Auth is OpenRouter via Harbor's model connection (`OPENROUTER_API_KEY`).

Default model: `openrouter/anthropic/claude-sonnet-4.5` (TB-class coding,
cheaper than opus; billed through the work OpenRouter key).

Usage (from the Harbor checkout, work key in the environment)::

    cargo build -p rung-agent --release
    PYTHONPATH=<rung>/rung-agent/python \\
      doppler run -p fleet -c dev_work -- \\
      uv run harbor run -p examples/tasks/hello-world \\
        --agent rung_harbor.agent:RungAgent \\
        --model openrouter/anthropic/claude-sonnet-4.5
"""

from __future__ import annotations

import os
import shlex
from pathlib import Path
from typing import override

from harbor.agents.installed.base import (
    BaseInstalledAgent,
    CliFlag,
    with_prompt_template,
)
from harbor.agents.model_connection import ModelConnectionSpec
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

from rung_harbor.model import rung_model_slug

OPENROUTER_BASE = "https://openrouter.ai/api/v1"
REMOTE_BIN = "/usr/local/bin/rung-agent"


def host_binary() -> Path:
    env = os.environ.get("RUNG_AGENT_BIN")
    if env:
        p = Path(env).expanduser()
        if p.is_file():
            return p
        raise FileNotFoundError(f"RUNG_AGENT_BIN is not a file: {p}")
    root = Path(__file__).resolve().parents[3]
    for candidate in (
        root / "target" / "release" / "rung-agent",
        root / "target" / "debug" / "rung-agent",
        Path.home() / ".cargo" / "bin" / "rung-agent",
    ):
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(
        "rung-agent binary not found. Build with "
        "`cargo build -p rung-agent --release` or set RUNG_AGENT_BIN."
    )


class RungAgent(BaseInstalledAgent):
    """Install our `rung-agent` CLI in the sandbox and run it headless."""

    MODEL_CONNECTION = ModelConnectionSpec(passthrough=True)

    CLI_FLAGS = [
        CliFlag(
            "kind",
            cli="--type",
            type="enum",
            choices=["explore", "implement", "review"],
            default="implement",
        ),
        CliFlag(
            "max_iterations",
            cli="--max-iterations",
            type="int",
        ),
    ]

    @staticmethod
    @override
    def name() -> str:
        return "rung-agent"

    @override
    def version(self) -> str | None:
        return "0.1.0"

    @override
    def get_version_command(self) -> str | None:
        return "rung-agent --help >/dev/null && echo 0.1.0"

    @override
    async def install(self, environment: BaseEnvironment) -> None:
        await self.ensure_system_dependencies(environment, ("ca_certificates",))
        src = host_binary()
        await environment.upload_file(src, REMOTE_BIN)
        await self.exec_as_root(
            environment,
            command=f"chmod 755 {shlex.quote(REMOTE_BIN)}",
        )

    @override
    def populate_context_post_run(self, context: AgentContext) -> None:
        pass

    @override
    @with_prompt_template
    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        access = self.model_connection
        api_key = access.api_key or os.environ.get("OPENROUTER_API_KEY")
        if not api_key:
            raise ValueError(
                "OPENROUTER_API_KEY missing. Run under "
                "`doppler run -p fleet -c dev_work`."
            )
        model = rung_model_slug(self.model_name)
        base_url = access.base_url or OPENROUTER_BASE
        env = {
            **dict(access.env),
            "OPENROUTER_API_KEY": api_key,
            "RUNG_API_KEY": api_key,
            "RUNG_BASE_URL": base_url,
            "RUNG_MODEL": model,
            "RUNG_PROTOCOL": "openai-chat",
            "RUNG_CONFIG": "/nonexistent-rung-config.yaml",
            "SSL_CERT_FILE": "/etc/ssl/certs/ca-certificates.crt",
        }
        flags = self.build_cli_flags()
        extra = (flags + " ") if flags else ""
        prompt = shlex.quote(instruction)
        await self.exec_as_agent(
            environment,
            command=(
                f"{shlex.quote(REMOTE_BIN)} {extra}--isolation none -- {prompt} "
                f"2>&1 | tee /logs/agent/rung-agent.txt"
            ),
            env=env,
        )
