/** What the record itself holds of one model call's request, for a host that does not serve the context door or a turn
 *  that predates it. The record keeps the turns' own messages whole, the provider's counts per call, the epoch's title line,
 *  and only fingerprints of the tools and the system text; this fold draws what is there, and says for each band what
 *  is not and why. Nothing is guessed: a band the record cannot place stays unrecorded. */
import type { Line } from "../record/types.ts";
import { WIRE_ORDER, emptyBand, missingBand, type Band, type BandId, type ContextView } from "./types.ts";
import { item, say } from "./messages.ts";

const NO_LOG = "The turn's log is written when the turn ends, so the request is not in the record yet.";

export function foldContext(lines: readonly Line[], turn: number, call: number | null): ContextView | null {
  const started = lines.find((l) => l.kind === "turn.started" && l.turn === turn);
  if (!started) return null;
  const llm = lines.filter((l) => l.kind === "llm.call" && l.turn === turn);
  const calls = llm.length;
  const k = calls === 0 ? 0 : call !== null && call >= 1 && call <= calls ? call : calls;
  const one = k ? llm[k - 1] : null;
  const epoch: number | null = one?.epoch ?? started.epoch ?? null;
  const log = lines.find((l) => l.kind === "turn.log" && l.turn === turn) ?? null;
  const msgs = (log?.messages ?? []) as Line[];

  const bands = new Map<BandId, Band>();
  const stable = "The record keeps only a fingerprint of the stable layer (the tool definitions and the system prompt), not their text.";
  bands.set("tools", missingBand("tools", stable));
  bands.set("system", missingBand("system", stable));

  const roll = epoch === null ? undefined : lines.findLast((l) => l.kind === "epoch.rollover" && l.to === epoch && l.seq < started.seq);
  bands.set("epoch", roll && typeof roll.l1 === "string"
    ? emptyBand("epoch", {
        partial: "Only the epoch's title line was recorded; the rest of the epoch summary was not.",
        estimate: typeof roll.slow_tokens === "number" ? roll.slow_tokens : null,
        items: [{ label: "Epoch title", text: roll.l1, note: null }],
      })
    : missingBand("epoch", "The record has no start line for this epoch."));

  // Earlier turns of this epoch, from their own logs.
  const earlier = lines.filter((l) => l.kind === "turn.started" && l.epoch === epoch && l.turn < turn);
  const logs = new Map<number, Line>();
  for (const l of lines) if (l.kind === "turn.log" && l.turn < turn) logs.set(l.turn, l);
  bands.set("history", emptyBand("history", {
    partial: "Rebuilt from the earlier turns' own logs, one row per turn of this epoch. The record did not keep the request itself.",
    items: earlier.map((s) => {
      const lg = logs.get(s.turn);
      return { label: `Turn ${s.turn}`, note: null, text: lg ? [String(lg.header ?? ""), ...((lg.messages ?? []) as Line[]).map(say)].join("\n") : "No log was written for this turn." };
    }),
  }));

  const recallLine = lines.find((l) => l.kind === "memory.recall" && l.turn === turn);
  if (!log) bands.set("recall", missingBand("recall", NO_LOG));
  else if (log.recall === false && !recallLine) bands.set("recall", emptyBand("recall", { estimate: 0 }));
  else {
    const r = recallLine?.report;
    bands.set("recall", missingBand("recall", `The record keeps only a report of what was recalled${r ? ` (${r.records ?? 0} ${r.records === 1 ? "record" : "records"}, ${String(r.status ?? "unknown")})` : ""}, not the text.`));
  }

  bands.set("message", log
    ? emptyBand("message", {
        text: String(log.header ?? ""),
        partial: log.recall === false ? null : "The turn's header as logged. The recalled memory that went with it is not in the record.",
      })
    : missingBand("message", NO_LOG));

  // The request for call k holds every message before the k-th model reply; the last call, whose reply may be kept only as the turn's final text, holds all of them.
  const replies = msgs.map((m, i) => (m.role === "assistant" ? i : -1)).filter((i) => i >= 0);
  if (!log) bands.set("this_turn", missingBand("this_turn", NO_LOG));
  else if (k === 0 || k > replies.length + 1) bands.set("this_turn", missingBand("this_turn", "The log cannot place this call among the turn's messages."));
  else {
    const upto = k <= replies.length ? replies[k - 1] : msgs.length;
    const items = msgs.slice(0, upto).map(item);
    bands.set("this_turn", emptyBand("this_turn", { items, estimate: items.length === 0 ? 0 : null }));
  }

  const brk = lines.find((l) => l.kind === "cache.break" && l.turn === turn && l.call === k);
  return {
    source: "record", turn, call: k, calls, epoch,
    model: one?.model_served ?? one?.model_requested ?? started.model ?? null,
    usage: one ? { prompt: one.prompt_tokens ?? 0, cached: one.cached_tokens ?? 0, written: one.cache_write_tokens ?? 0 } : null,
    cacheEvent: brk ? { kind: "cache.break", cause: String(brk.cause ?? "unknown") } : null,
    rewritten: false, problems: [], note: null, boundaries: [], damaged: null,
    bands: WIRE_ORDER.map((id) => bands.get(id)!),
    doorNote: null,
  };
}
