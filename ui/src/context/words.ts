/** The words of the strata page, in one place so they can be tested. */
import type { Band, BandId } from "./types.ts";
import type { Mark } from "./cache.ts";
import { num } from "../record/words.ts";

export const TITLE: Record<BandId, string> = {
  tools: "Tools", system: "System prompt", epoch: "Epoch summary", history: "History",
  recall: "Memory recall", message: "Message", this_turn: "This turn so far",
};

/** What each band is, in plain words. */
export const WHAT: Record<BandId, string> = {
  tools: "The tools the model may call.",
  system: "Who the agent is and the host's rules.",
  epoch: "What earlier epochs left behind.",
  history: "The turns before this one.",
  recall: "What memory returned for this turn.",
  message: "What this turn was asked.",
  this_turn: "The model's own earlier calls in this turn.",
};

export const BOUNDARY: Record<1 | 2, string> = {
  1: "Cache boundary 1: end of the stable prefix",
  2: "Cache boundary 2: end of the epoch layer",
};

export const MARK_WORD: Record<Mark, string> = { cached: "cached", new: "new", partly: "partly cached", empty: "nothing sent", unknown: "unknown" };

const plural = (n: number, one: string, many: string) => `${num(n)} ${n === 1 ? one : many}`;

/** One short fact about what a band holds. */
export function fact(b: Band): string {
  if (!b.recorded) return "not recorded";
  const n = b.items.length;
  switch (b.id) {
    case "tools": return plural(n, "definition", "definitions");
    case "history": return n === 0 ? "no earlier turn" : plural(n, "turn", "turns");
    case "this_turn": return n === 0 ? "nothing yet" : plural(n, "message", "messages");
    case "epoch": return n === 0 && !b.text ? "empty" : "";
    case "recall": return b.estimate === 0 && !b.text ? "nothing recalled" : "";
    default: return "";
  }
}

export const sayCause = (cause: string): string => cause.replace(/_/g, " ");
export const tilde = (n: number): string => `~${num(n)}`;

/** A band with nothing in it to open. */
export const isEmpty = (b: Band): boolean => b.recorded && !b.text && b.items.length === 0;

/** A text long enough to fold under "Show more". The whole text is always in the page; the fold only keeps the band short. */
export const FOLD_LINES = 14;
export const FOLD_CHARS = 1200;
export const isLong = (t: string): boolean => t.length > FOLD_CHARS || t.split("\n").length > FOLD_LINES;

/** The line about the cache at this call: a break the host recorded, a cold cache, or neither. */
export function sayEvent(e: { kind: string; cause: string } | null): string {
  if (!e) return "No cache break before this call.";
  return e.kind === "cache.cold" ? `The cache was cold at this call: ${sayCause(e.cause)}.` : `A cache break came before this call: ${sayCause(e.cause)}.`;
}

/** A line this long is not wrapped: a browser that breaks a one-line megabyte of mixed scripts into lines takes tens of seconds, and the tab
 *  is frozen for all of them. The text is whole either way; such a text scrolls sideways instead of wrapping. */
export const LONG_LINE = 50_000;
export function hasLongLine(t: string): boolean {
  if (t.length <= LONG_LINE) return false;
  let at = 0;
  while (at < t.length) {
    const nl = t.indexOf("\n", at);
    const end = nl < 0 ? t.length : nl;
    if (end - at > LONG_LINE) return true;
    at = end + 1;
  }
  return false;
}
