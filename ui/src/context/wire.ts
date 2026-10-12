/** The host's context door, `GET /v1/turns/{n}/context?call=k` (docs/rung-host-api.md, S4), read into the page's one shape.
 *  A body that does not fit is refused in words, never half drawn: a band that is wrong would read as what the model was sent.
 *  Every text passes the same redactor as everything a host sends (a second line: the host redacts first) and is never shortened. */
import { redact } from "../live/wire.ts";
import type { Line } from "../record/types.ts";
import { item as messageItem, say } from "./messages.ts";
import { WIRE_ORDER, emptyBand, missingBand, type Band, type BandId, type BandItem, type ContextView, type Usage } from "./types.ts";

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const count = (v: unknown, what: string): number => {
  if (typeof v !== "number" || !Number.isFinite(v) || v < 0) throw new Error(`the context answer has no valid ${what}`);
  return v;
};
const optCount = (v: unknown, what: string): number | null => (v === undefined || v === null ? null : count(v, what));
const need = (v: unknown, band: string, what: string): string => {
  if (typeof v !== "string") throw new Error(`the ${band} band has no ${what}`);
  return v;
};
const messages = (v: unknown, band: string): Line[] => {
  if (!Array.isArray(v) || v.some((m) => !isObj(m) || typeof m.role !== "string")) throw new Error(`the ${band} band's messages are not a list of messages`);
  return v as Line[];
};
const MARKS = ["cached", "partly", "new", "unknown"] as const;

const NOT_THERE = "The host did not record this band for this call.";
/** Why a band is not in the answer, in words. The host names what it cannot show and says why in `note`. */
const sentence = (t: string): string => `${t.charAt(0).toUpperCase()}${t.slice(1).replace(/\.$/, "")}.`;
const whyMissing = (id: BandId, note: string | null): string =>
  note ? `${sentence(note)}${id === "recall" ? " The turn may have been shown recalled memory that the record does not hold." : ""}` : NOT_THERE;

function band(id: BandId, b: Record<string, unknown>): Band {
  const out = emptyBand(id, {
    estimate: optCount(b.est_tokens, `${id} size`),
    cache: typeof b.cache === "string" && (MARKS as readonly string[]).includes(b.cache) ? (b.cache as Band["cache"]) : null,
  });
  switch (id) {
    case "tools": {
      if (!Array.isArray(b.items)) throw new Error("the tools band has no definitions");
      out.items = b.items.map((t) => {
        if (!isObj(t)) throw new Error("the tools band has a definition that is not an object");
        return { label: need(t.name, "tools", "name"), text: need(t.text, "tools", "text"), note: null };
      });
      break;
    }
    case "history": {
      if (!Array.isArray(b.rows)) throw new Error("the history band has no rows");
      let thin = 0;
      out.items = b.rows.map((r): BandItem => {
        if (!isObj(r) || typeof r.turn !== "number") throw new Error("the history band has a row without a turn");
        const complete = r.header_complete !== false;
        if (!complete) thin++;
        const header = need(r.header, "history", "header");
        return {
          label: `Turn ${r.turn}`,
          text: [header, ...messages(r.messages ?? [], "history").map(say)].join("\n"),
          note: complete ? null : "This turn was shown recalled memory that the record does not hold, so its header is as far as the record goes.",
        };
      });
      if (thin) out.partial = `${thin} of the earlier turns ${thin === 1 ? "was" : "were"} shown recalled memory that the record does not hold.`;
      break;
    }
    case "this_turn":
      out.items = messages(b.messages ?? [], "this turn").map(messageItem);
      break;
    case "recall":
      out.text = need(b.text, "recall", "text");
      if (b.intact === false) out.partial = "The recorded text does not match what the host stored for it. It is shown as recorded.";
      break;
    default:
      out.text = need(b.text, id, "text");
  }
  return out;
}

export function parseContext(raw: unknown): ContextView {
  if (!isObj(raw)) throw new Error("the answer is not a context answer");
  const body = redact(raw) as Record<string, unknown>;
  if (!Array.isArray(body.bands)) throw new Error("the context answer has no bands");
  const turn = count(body.turn, "turn"), call = count(body.call, "call"), calls = count(body.calls, "number of calls");

  const given = new Map<BandId, Record<string, unknown>>();
  for (const b of body.bands as unknown[]) {
    if (!isObj(b) || typeof b.band !== "string") throw new Error("the context answer has a band with no name");
    if (!(WIRE_ORDER as readonly string[]).includes(b.band)) continue; // a band this page does not know is not drawn in the wrong place
    if (given.has(b.band as BandId)) throw new Error(`the context answer gives the ${b.band} band twice`);
    given.set(b.band as BandId, b);
  }
  const note = typeof body.note === "string" ? body.note : null;
  const listed = new Set(Array.isArray(body.missing) ? body.missing.filter((m): m is string => typeof m === "string") : []);

  let usage: Usage | null = null;
  if (isObj(body.provider)) usage = { prompt: count(body.provider.prompt_tokens, "prompt count"), cached: count(body.provider.cached_tokens, "cached count"), written: optCount(body.provider.cache_write_tokens, "cache write count") ?? 0 };
  const ev = isObj(body.cache_event) && typeof body.cache_event.cause === "string" ? { kind: typeof body.cache_event.kind === "string" ? body.cache_event.kind : "cache.break", cause: body.cache_event.cause } : null;
  const model = isObj(body.model) ? (typeof body.model.served === "string" ? body.model.served : typeof body.model.requested === "string" ? body.model.requested : null) : null;
  const boundaries = (Array.isArray(body.boundaries) ? body.boundaries : [])
    .filter((b): b is Record<string, unknown> => isObj(b) && (b.after === "system" || b.after === "epoch"))
    .map((b) => ({ after: b.after as BandId, tokens: optCount(b.tokens, "boundary position") }));

  return {
    source: "door", turn, call, calls,
    epoch: optCount(body.epoch, "epoch"), model, usage,
    cacheEvent: ev,
    rewritten: body.rewritten === true,
    problems: Array.isArray(body.problems) ? body.problems.filter((p): p is string => typeof p === "string") : [],
    note, boundaries,
    damaged: typeof body.damaged === "string" ? body.damaged : null,
    bands: WIRE_ORDER.map((id) => {
      const g = given.get(id);
      if (g) return band(id, g);
      // The door leaves the recall band out of a turn that had no recall: that is an empty band, not a gap. A band it names as missing is a gap, with its note.
      if (id === "recall" && !listed.has("recall")) return emptyBand("recall", { estimate: 0 });
      return missingBand(id, listed.has(id) ? whyMissing(id, note) : NOT_THERE);
    }),
    doorNote: null,
  };
}
