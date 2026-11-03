# AGENTS.md — rung-agent

The task CLI: a thin shell over `rung-agent-core` (catalog, sessions, turn
engine, MCP, ACP, OAuth). Charter for the whole repo: [`../AGENTS.md`](../AGENTS.md).

## What it is

Product CLI `rung-agent`: catalog, sessions, isolation worktrees, background
spawn, XDG `config.yaml`, `--acp` on stdio, `--acp-http` Streamable HTTP. Kernel
`task` is nested `Spawn`, depth 1; catalog, resume, worktrees and background are
product, not kernel. Flags and config keys are documented by `rung-agent --help`
(source: `rung-agent-core/src/args.rs`).

OAuth is the second credential kind: `rung-agent auth login <provider>` (GitHub
Copilot first). Contract: [`docs/reference/rung-agent-oauth.md`](../docs/reference/rung-agent-oauth.md).
The prompt cache note: [`docs/reference/rung-agent-cache.md`](../docs/reference/rung-agent-cache.md).

## ACP

- `--acp` is ACP v1 on stdio (`agent-client-protocol` crate), the only stable
  transport. Baseline plus load/list/delete/close/set_mode/resume and unstable
  `session/fork`.
- `--acp-http [ADDR]` is the experimental Streamable HTTP RFD (`POST`/`GET`/
  `DELETE /acp`, `Acp-Connection-Id` / `Acp-Session-Id`, SSE GET streams),
  matching `@agentclientprotocol/sdk` `createHttpStream` / `AcpServer`. HTTP/2 is
  accepted; HTTP/1.1 is served so the TS client works on localhost. WebSocket
  upgrade returns 426 until sacp HTTP lands.
- Prompt emits tool-call `session/update`s; cancel is checked before each LLM
  call and around each tool. Session history keeps each turn's tool calls and
  results and replays them into the next turn. Prompt image, audio and embedded
  context are claimed. MCP HTTP (and ACP-required stdio) tools are admitted for
  the session.

## Configuration

| file | holds |
|---|---|
| `~/.rung/providers.yaml` | endpoint catalog and `default:` (driver) |
| `~/.rung/auth.yaml` | provider to key (machine-local; never commit) |
| `$XDG_CONFIG_HOME/rung/config.yaml` | `rung-agent` LLM settings (`llm.api_key_env` optional, names the variable, not the key); `turn_check` (off by default) |

OAuth logins live under `$RUNG_HOME/oauth/` (mode 0600; never commit).
`$RUNG_HOME` overrides `~/.rung/`; `RUNG_CONFIG` overrides the XDG path. Env
`RUNG_*` / `XAI_API_KEY` wins over the agent file.

## Harbor validation suite

Harbor eval is out of tree (`python/rung_harbor`); do not fork Harbor. The
validation suite (`rung_harbor.suite`) is a capability ladder of Harbor *agent*
tasks. Each `run` / `next` writes timestamped evidence under `harbor-runs/`
(gitignored) and appends `index.jsonl`; redo a case with `validate run <id>`,
which always creates a new folder. Key: `doppler run -p fleet -c dev_work`
(`OPENROUTER_API_KEY`). Suite model:
`openrouter/~deepseek/deepseek-v4-flash-latest`.

```bash
PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate list
PYTHONPATH=<rung>/rung-agent/python \
  doppler run -p fleet -c dev_work -- \
  python3 -m rung_harbor.validate next
python3 -m rung_harbor.validate run cwd-capture
python3 -m rung_harbor.validate show cwd-capture
python3 -m rung_harbor.validate import   # seed from existing Harbor jobs/
```
