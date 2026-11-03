# Run rung-agent: flags, config, environment, login

Informative. The authority is the code: `rung-agent --help` (`usage()` in
`rung-agent-core/src/args.rs`) for flags, `rung-agent-core/src/config.rs` for
config keys and environment variables. This page says where to look and what
each knob is for; it does not restate the help text. Contracts it points at:
[rung-agent-oauth.md](../reference/rung-agent-oauth.md),
[rung-agent-cache.md](../reference/rung-agent-cache.md),
[rung-memory.md](../reference/rung-memory.md).

## Run it

```bash
cargo build -p rung-agent
target/debug/rung-agent --help
target/debug/rung-agent --json --tools none "Reply ok."     # one prompt, one JSON outcome
target/debug/rung-agent --task-id ID "and then?"             # resume a session
target/debug/rung-agent --acp                                # ACP over stdio (an editor or a host)
```

Sessions are `<cwd>/.rung/sessions/<id>.jsonl`. `--toolset explore|implement|review`
chooses a named toolset; `--tools` composes groups for one call.
`--isolation worktree` runs in `<repo>.wt/rung-task--<id>`; `--background`
spawns a child and prints its `task_id` and pid (the child carries
`RUNG_AGENT_CHILD`, which is internal).

Other flags: `--acp-http [ADDR]` and `--acp-token TOKEN` (ACP over Streamable
HTTP, experimental; the token is required as a bearer), `--mcp-http name=url`
(connect an MCP server; repeatable), `--stream` (NDJSON trace events),
`--max-iterations N` (model calls per prompt; 0 is no cap), `--system-prompt`
and `--user-prompt` (text or `@file`), `--type` (alias of `--toolset`),
`--memory-scope ls|rm ID|drop` (inspect or delete a baseline memory scope).

## Config

`$XDG_CONFIG_HOME/rung/config.yaml` (`RUNG_CONFIG` names another path). A key
never goes in the file: `llm.api_key_env` names the
variable that holds it.

```yaml
llm:
  base_url: https://openrouter.ai/api/v1
  model: deepseek/deepseek-chat-v3.1
  api_key_env: OPENROUTER_API_KEY
```

Every `llm:` key also has an environment variable that wins over the file.

| `llm:` key | environment | notes |
|---|---|---|
| `base_url`, `model` | `RUNG_BASE_URL`, `RUNG_MODEL` | an OpenAI-compatible route |
| `api_key_env` | `RUNG_KEY_FILE` (names the variable that holds the key) | without either: `RUNG_API_KEY`, then `XAI_API_KEY`; an empty key sends no `Authorization` header |
| `timeout_secs` | `RUNG_TIMEOUT_SECS` | per request, default 120 |
| `idle_timeout_secs` | `RUNG_IDLE_TIMEOUT_SECS` | stall limit on a stream |
| `max_tokens`, `temperature` | `RUNG_MAX_TOKENS`, `RUNG_TEMPERATURE` | unset sends none |
| `protocol` | `RUNG_PROTOCOL` | `auto`, `openai-chat`, `anthropic-messages` |
| `reasoning_level` | `RUNG_REASONING_LEVEL` | passed to the route as given |
| `cache` | | `auto` or `none`; see [rung-agent-cache.md](../reference/rung-agent-cache.md) |
| `images` | `RUNG_IMAGES=on` | tool results may carry images (default off) |
| `auth.oauth` | `RUNG_OAUTH_PROVIDER`, `RUNG_OAUTH_PROFILE` | a login instead of a key (below) |

Other sections: `memory:` (`provider`, `scope`, `dir`, `timeout_secs`,
`retain_timeout_secs`; environment `RUNG_MEMORY`, `RUNG_MEMORY_SCOPE`,
`RUNG_MEMORY_DIR`, `RUNG_MEMORY_TIMEOUT_SECS`, `RUNG_MEMORY_RETAIN=distill`,
`RUNG_MEMORY_RETAIN_TIMEOUT_SECS`, and `RUNG_MEMORY_TOKEN`, the provider's bearer token; first steps in
[rung-memory-quickstart.md](rung-memory-quickstart.md)) and `turn_check:`
(`backend` `off` or `jev`, `base_url`, `model`, `api_key_env`, `timeout_secs`;
environment `RUNG_TURN_CHECK`, `RUNG_TURN_CHECK_BASE_URL`; default off).

Other variables: `RUNG_HOME` (machine-local state such as the OAuth logins, default `~/.rung/`),
`RUNG_SYSTEM_PROMPT_FILE` (the host sets it at spawn),
`RUNG_SHELL_TIMEOUT_SECS` (the shell tool), `RUNG_REDACT_ENVS` (names of
further variables whose values are redacted from output),
`RUNG_REASONING` (the reasoning level for a host-spawned engine), `RUNG_MODEL_FACTS=off` (skip the model-listing request), `RUNG_ACP_WORKERS=0` (keep
the single-process ACP agent instead of one worker per session; `RUNG_ACP_WORKER`
marks a worker and is internal), `RUNG_PYTHON_STORE` and
`RUNG_INLINE_PYTHON` (the python tool; see
[the experiment](../experiments/inline-python-vs-tools.md)), and
`RUNG_DECIDE`, `RUNG_DECIDE_BASE_URL`, `RUNG_DECIDE_MODEL`,
`RUNG_DECIDE_BUDGET_USD`, `RUNG_DECIDE_LEDGER` (the decide block's backend).

## Log in with an OAuth provider

For a provider that issues a login to a person rather than a key:

```bash
rung-agent auth login github-copilot [--profile NAME] [--enterprise DOMAIN]
rung-agent auth status        # never prints a token
rung-agent auth models
rung-agent auth logout github-copilot [--profile NAME]
```

`login` shows a device code and a URL and polls until it is approved. The
login lives at `$RUNG_HOME/oauth/<provider>--<profile>.json`, mode 0600, never
in a repo. Select it with `llm.auth.oauth: { provider, profile }` or with
`RUNG_OAUTH_PROVIDER` / `RUNG_OAUTH_PROFILE`. The contract (O1–O19) is
[rung-agent-oauth.md](../reference/rung-agent-oauth.md).
