# Live cached-token proof with the memory block on

Evidence note for `docs/rung-agent-cache.md`. Measured 2026-10-04 on roger, rung `3a94bc8`
(`v0.2.1-29`, `rung-agent` built from that commit), live through OpenRouter, memory provider
`rung-memory-jevmem` serving a real Jev store live (read-only; recall on, so a recalled block
is in turn 1).

```bash
# provider: eval store with the payments note, live Jev, read-only
doppler run -p fleet -c dev_work -- python3 scripts/acp_cache_probe.py \
    --model deepseek/deepseek-chat-v3.1 \
    --memory mcp:http://127.0.0.1:9187/mcp --scope w4-native:payments \
    --asks "Read notes.txt with read_file and tell me how many lines it has." \
           "Thanks. Which branch does the payments service deploy from?" \
    --out /tmp/cache-probe
```

`scripts/acp_cache_probe.py` gained `--memory`, `--scope`, `--asks` for this. Spend: USD 0.0071 for the three DeepSeek runs below (sum of `cost` in the committed usage files);
the other trials (Haiku about 0.015, two Gemini runs about 0.004 each, one earlier DeepSeek run about 0.003) are
author-observed from the probe output, not committed. All together under USD 0.05. Model choice: `openai/gpt-4o-mini` is
refused by the workspace ZDR guardrail; `anthropic/claude-haiku-4.5` and `google/gemini-2.5-flash` ran but
reported `cached_tokens: 0` on every call (two Gemini runs, one Haiku run); `deepseek/deepseek-chat-v3.1`
reports cached tokens.

## Result

`prompt / cached_tokens`, one ACP session, two turns (call 1 is turn 1's tool step, call 2 is turn 2):

| run | call 0 | call 1 | **turn 2** (call 2) | where call 2 stops extending call 1 |
|---|---|---|---|---|
| memory **on**, run 1 | 2830 / 2494 | 5081 / 2826 | **5055 / 2508** | message 1 (the ask) |
| memory **on**, run 2 | 2830 / 2826 | 5071 / 2829 | **5043 / 2512** | message 1 (the ask) |
| memory **off** (control) | 2364 / 2176 | 4596 / 4 | 4621 / 4596 | extends |

**cached_tokens > 0 on turn two with the memory block on: yes (2508 and 2512 of ~5050, about 50%).**
It is not the full-prefix hit (4621 / 4596) that the memory-off control gets, so one prefix
difference remains. (Raw usage: `docs/evidence/cache-live-memory/usage-*.jsonl`; message sizes quoted below:
`sizes-on-1.json`; the requests themselves are held back, they quote the notes.)

## The remaining difference

Turn 1 sends the ask with the recall block appended inside the same user message
(`"<ask>\n\n---\n## Recalled memory ..."`, 1175 bytes). Turn 2 replays that message as the bare ask
(95 bytes), because the session keeps the ask as the user wrote it (`rung-agent-core/src/run.rs`,
"Recall: shown to this call only"; `memory::inject`). The requests agree through the system text
and the first bytes of the ask, then diverge at the end of message 1. The prefix that is cached is
the system text and the tools (~2.5k tokens); everything after the ask in turn 1's history
(the assistant tool call and the 8.8 KB tool result) is billed uncached on turn 2 (prompt minus cached, about 2.5k tokens).

This is the cost `docs/rung-agent-cache.md` already records for B3 ("the previous turn's steps
still follow the block, so they are read once uncached on the next turn"); the live numbers size it
here at about half of the turn-2 prompt, because the block sits inside the ask message and the
step that follows is a large tool result. It scales with the size of the steps that follow the recall in that turn. A fix would put the block
in a place the next turn can replay byte for byte (store it with the ask, or send it as a separate
trailing message after the ask's tool-free turn), at the price of the block living in the session.
Not built here; built since, see below.

## After the fix

The session user line now keeps the block beside the ask (`recalled`, never the
user's `text`), and a later turn replays the ask and its block byte for byte
(`docs/rung-memory.md`, B3 in `docs/rung-agent-cache.md`). Measured 2026-10-05 on
roger, same route and model, memory `baseline` seeded by a `--seed` turn in its own
session (its calls are not reported), so turn 1 carries a recalled block:

```bash
doppler run -p fleet -c dev_work -- python3 scripts/acp_cache_probe.py \
    --model deepseek/deepseek-chat-v3.1 --memory baseline \
    --seed "Remember this: notes.txt is the payments service notes file, and the payments service deploys from the release/payments branch." \
    --asks "Read notes.txt with read_file and tell me how many lines it has." \
           "Thanks. Which branch does the payments service deploy from?"
```

| run | call 0 | call 1 | **turn 2** (call 2) | where call 2 stops extending call 1 |
|---|---|---|---|---|
| memory **on**, the fix | 2739 / 2176 | 4981 / 2688 | **5236 / 4864** | extends |
| memory **on**, before (`e519a4f`) | 2709 / 4 | 4962 / 2705 | **4999 / 2432** | message 1 (the ask) |
| memory **off** (control, no `--memory`) | 2355 / 2253 | 4608 / 2351 | **4633 / 4608** | extends |

Call 2 is turn 2's first call. In the control run, with no recall to answer from,
turn 2 took one more step: a `grep` call 3 (4957 / 2304). It extends call 2, but the
route moved from DeepInfra to SiliconFlow and started cold. It is in
`usage-fix-off.jsonl` and is not part of the turn-2 comparison.

Turn 2 with memory on now caches 4864 of 5236 (93%), against 2432 of 4999 (49%)
before; the uncached rest is turn 2's own ask and its recall block. Raw usage:
`docs/evidence/cache-live-memory/usage-fix-*.jsonl`. Spend: USD 0.0098 for the
calls reported (sum of `cost`), plus three short seed calls.

## Caveats

- The probe's recording proxy is `http://127.0.0.1`, so `rung_std::llm::is_openrouter_url` is false and no
  `session_id` was sent (`session_id=False` in the output); A2 is not exercised here.
- DeepSeek's provider varied between calls (DeepInfra, SiliconFlow), and a route change starts cold
  (memory-off call 1 shows `cached 4`), so the absolute cached counts are best-effort; the memory-on
  vs memory-off gap held in both memory-on runs.
- Gemini 2.5 Flash and Haiku 4.5 report no cached tokens through this route, so no proof on them.
