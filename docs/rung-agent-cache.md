# rung-agent and the prompt cache

A provider caches the prefix of a request. A later request is billed at the
cached rate only for the bytes it shares with an earlier one, from the
first byte on. So within one session each request must extend the one
before it byte for byte, and on providers that cache only where asked
(Claude), the request must also ask. This page records why rung-agent's ACP
turns got no cache, the evidence for each cause, and what the caller must
keep stable.

**Class.** These are rung's defects in the request rung-agent builds, not
provider faults. One cause (A) is a missing request feature. The rest (B, C)
are places where rung changed bytes that an earlier request had already
sent.

## Evidence

- Mock: `rung-agent/tests/acp_prefix.rs` runs one ACP session against
  `rung_testkit::llm::mock_llm`. It records every request body and checks
  that each one extends the one before.
  `rung-std/tests/llm_wire.rs` pins how the markers are placed, and that
  with markers removed each request is a byte-prefix of the next.
- Live: `scripts/acp_cache_probe.py` runs the same two-turn ACP session through
  a recording proxy to OpenRouter. The first turn reads a 6 KB file; the
  second turn asks a follow-up. The system text is about 2.8k tokens. The
  probe prints, for each call, where its request stops extending the one
  before and the usage the route reported. Model
  `qwen/qwen3.8-27b:free`, $0.

| call | before: where it diverges | before: prompt / cached | after: where it diverges | after: prompt / cached |
|---|---|---|---|---|
| turn 1, call 1 | first call | 2820 / 0 | first call | 2820 / 0 |
| turn 1, call 2 | extends | 5536 / 4096 | extends | 5536 / 2816 |
| turn 1, call 3 | extends | 5832 / 5632 | extends | 5832 / 5376 |
| turn 2, call 1 | message 1 (the ask) | 4530 / 4352, and 0 in another run | extends | 5865 / 5632 (author-reported; not independently reproduced) |

Before the fix, turn 2 first diverged at the previous ask and then at the
shortened tool result. The "before" prompt is smaller only because the
history had been shortened. The free route's cache is best-effort: in one
"before" run, turn 2 got 0 cached tokens.

## Causes

| # | cause | rung's? | evidence | fix |
|---|---|---|---|---|
| A1 | `CachePolicy::Auto` placed `cache_control` only on the Anthropic wire. On an OpenAI-compatible route, such as OpenRouter to Claude, rung sent no marker, so Claude cached nothing. With a Claude model, every turn showed `cached_tokens: 0`. | yes | before the fix, `llm_wire.rs` gave a body with no markers | Auto marks the end of the system text and the latest user message when the route takes markers (OpenRouter, or a Claude model id) and the caller placed none. System and user text then always go as text parts, so moving the marker changes no earlier byte. |
| A2 | No `session_id`, so a router had no key to keep a session on one provider and its warm cache. | yes | no `session_id` in any request | On an OpenRouter route, rung-agent sends the ACP session id as `session_id`. Other routes get none: a plain server may refuse the field. One check (`rung_std::llm::is_openrouter_url`) decides this for rung-agent and for the rung-host adapter's default. |
| B1 | An ACP all-text ask was sent as content parts and replayed as a string. | yes | live turn 2 diverged at message 1. The tokens were equal (4352 cached), but the bytes were not. | An all-text prompt is sent as its job text, the form in which it is replayed. |
| B2 | Session history cut a tool result to 4000 chars. A later turn diverged at the cut, and every step of that turn after the cut lost the cache. | yes | live turn 2 diverged at the tool result | Results are kept verbatim. The loop's own 8192-byte cap (`DEFAULT_TOOL_OUTPUT_LIMIT`) already bounds each result, so history grows at most twice as fast as with the old cut. |
| B3 | The recall block went in front of the ask for that call only. The next turn replayed the ask without the block, so it diverged before the ask. | yes | mock: divergence at the start of the previous ask | The block follows the ask. Every byte through the ask now matches. The previous turn's steps still follow the block, so they are read once uncached on the next turn. The block holds volatile text (`observed_at`), so it stays out of the session. |
| C1 | A session's `_meta.systemPrompt` lived only in the process that served `session/new`. After a restart, `session/load` sent no system text, and the whole prefix was lost. | yes | mock: load diverged at message 0 | The session file keeps the session's system text. A fork copies it. |
| C2 | `session/load`, `session/resume` and `session/fork` ignored their `mcpServers`, so the tool list changed after a restart. | yes | mock: tool list differed after a load | Load, resume and fork connect the MCP servers the request names. |

Ruled out (each stable across turns and across processes): tool order and
definitions (a fixed roster, and the MCP order the server lists),
timestamps or ids in system or tool text (there are none), JSON key order
(`serde_json` `preserve_order`, so a session's stored tool input
round-trips), and `reasoning_effort` and `max_tokens` (read once per
process).

Tail-only by design: the last step's closing instruction, and a recall
block. Each is sent with one call and is not stored, so it costs only the
messages after it, once.

Not rung's to fix:

- A tool image becomes a note in history, because the session file holds no
  image data. Only with `llm.images: true`.
- A context-marked block that an earlier turn already sent is stored once,
  so the replay leaves it out (`docs/rung-memory.md`).
- Providers do not cache short prompts. Claude needs at least 1024 to 4096
  tokens before a marker, depending on the model, and OpenAI-style automatic
  caching starts at 1024 tokens.

`rung-host` keeps its own two fixed breakpoints, pinned by gates G-l and
G-n (`docs/rung-host.md`). Its adapter now sends `session_id` by default
only on an OpenRouter route. `scripts/cache_probe.py` is a separate
offline tool. It reads a recorded request log and usage log, and prints the
first byte at which each request differs from the one before.

## What the caller must keep stable

For the cache to hold across turns, a host that fronts rung-agent over ACP
must:

- **Use one ACP session per conversation.** Each `session/prompt` replays
  the session's history. A new session for each message shares only the
  system text and tools with the one before.
- **Send the same system text, byte for byte.** Set it once in
  `_meta.systemPrompt` on `session/new`. Keep the file behind
  `--system-prompt @file` or `RUNG_SYSTEM_PROMPT_FILE` unchanged, because it
  is reread every turn. Never put the time, a counter, a request id or
  retrieved state in it.
- **Put volatile text in the prompt, at its end.** The prompt is the tail of
  the request, so the time, presence and retrieved notes cost only that
  turn there.
- **Keep the same tools.** Name the same `mcpServers` on new, load,
  resume and fork, and keep each server's `tools/list` stable in order, names,
  descriptions and schemas.
- **Keep the same model and settings.** Keep `RUNG_MODEL`, the reasoning
  level and `RUNG_MAX_TOKENS` fixed for the session. Each provider has its
  own cache, so a different model or route starts cold.
- **Send unchanged context once.** If a context block is resent every turn,
  send it unmarked, or the replay will not match.
