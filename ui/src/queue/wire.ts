/** The queue as the page reads it: the host's `GET /v1/queue` answer (ui/contract/README.md), read whole and redacted, the words
 *  the page uses for an item, the order it shows, and the fold of a long text. Nothing here writes. */
import { redact } from "../live/wire.ts";

export type Intent = "next" | "free" | "steer";

export interface QueueItem {
  id: string;
  text: string;
  /** Who sent it: owner, peer or host. */
  role: string;
  /** A peer's or a host source's name; null for the owner. */
  name: string | null;
  /** The channel it came by (`owner`, `peer:atlas`, `calendar`). */
  channel: string;
  /** What the sender asked for. Null when the host sent a word the page does not know. */
  intent: Intent | null;
  /** When the host accepted it (the time sent); null when the record has none. */
  at: number | null;
  /** For an item being answered: the turn answering it. */
  turn: number | null;
  /** Why the desk holds it back, in the desk's words; null when nothing holds it. */
  held: string | null;
}

export interface QueueAnswer {
  /** Being answered now (a turn takes a batch). */
  now: QueueItem[];
  /** Waiting for a boundary, in the host's order. */
  waiting: QueueItem[];
  /** The last record line the host read. */
  lastSeq: number;
  /** Where the next page of the list starts, or null on the last. */
  next: number | null;
}

const bad = (why: string): never => { throw new Error(`that is not a queue answer: ${why}`); };
const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const STATES = ["waiting", "answering", "answered", "withdrawn"];

function item(v: unknown): { item: QueueItem; state: string } {
  if (!isObj(v)) return bad("an item is not an object");
  for (const k of ["id", "text", "role", "channel", "state"]) if (typeof v[k] !== "string") bad(`an item has no ${k}`);
  if (!STATES.includes(v.state as string)) bad("an item has a state the page does not know");
  return {
    state: v.state as string,
    item: {
      id: v.id as string, text: v.text as string, role: v.role as string, name: typeof v.name === "string" ? v.name : null, channel: v.channel as string,
      intent: v.intent === "next" || v.intent === "free" || v.intent === "steer" ? v.intent : null,
      at: typeof v.accepted_at === "number" ? v.accepted_at : null, turn: typeof v.turn === "number" ? v.turn : null,
      held: typeof v.held === "string" && v.held ? v.held : null,
    },
  };
}

/** Read one page of the host's list (`GET /v1/queue`): items with a state. What a host or a gateway wrote into it passes the page's
 *  redactor first, a second line behind the host's. Items that are over (answered, withdrawn) are not part of the queue and are left out. */
export function parseQueue(raw: unknown): QueueAnswer {
  const r = redact(raw);
  if (!isObj(r) || !Array.isArray(r.items)) return bad("it has no items");
  const items = r.items.map(item);
  return {
    now: items.filter((i) => i.state === "answering").map((i) => i.item),
    waiting: items.filter((i) => i.state === "waiting").map((i) => i.item),
    lastSeq: typeof r.record_lines === "number" ? r.record_lines : 0,
    next: typeof r.next === "number" ? r.next : null,
  };
}

export interface Row { pos: number; item: QueueItem }

/** The owner's items in a group of their own on top, then the rest, numbered on in one run. The host's order is kept inside each group. */
export function groupWaiting(waiting: readonly QueueItem[]): { owner: Row[]; others: Row[] } {
  const owner = waiting.filter((i) => i.role === "owner");
  const others = waiting.filter((i) => i.role !== "owner");
  return { owner: owner.map((item, i) => ({ pos: i + 1, item })), others: others.map((item, i) => ({ pos: owner.length + i + 1, item })) };
}

export const intentWord = (i: Intent | null): string => (i === "next" ? "Do next" : i === "free" ? "When free" : i === "steer" ? "Steer" : "");

export function roleWord(i: { role: string; name: string | null; channel: string }): string {
  const cap = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);
  if (i.role === "peer") {
    const name = i.name ?? (i.channel.startsWith("peer:") ? i.channel.slice(5) : "");
    return name ? `Peer ${name}` : "Peer";
  }
  return cap(i.role);
}

/** Nothing to send: only white space, or characters that draw nothing (zero-width spaces and joiners, direction marks, soft hyphens, byte-order
 *  marks, the blank fillers and the blank braille cell, control characters). The host refuses only a text that is all white space, so a
 *  message of invisible characters would be taken as a message; the page does not offer to send one. */
const INVISIBLE = /[\p{White_Space}\p{Cf}\p{Cc}\u034f\u115f\u1160\u17b4\u17b5\u180e\u2800\u3164\uffa0]/gu;
export const isBlank = (text: string): boolean => text.replace(INVISIBLE, "") === "";

/** What the box holds after a send ends: cleared only when the host wrote it down and nothing was typed meanwhile; otherwise the text stays
 *  (a failure keeps what was sent, even if the box was emptied meanwhile). */
export function afterSend(current: string, sent: string, wrote: boolean): string {
  if (wrote) return current === sent ? "" : current;
  return current === "" ? sent : current;
}

/** A long text in two parts that join to exactly the text: the page shows the head and offers the rest under "Show more". The fold
 *  falls at a space, never inside a word (and never inside a character); nothing is cut and nothing is added. */
export function foldText(text: string, limit = 280): { head: string; rest: string } {
  if (text.length <= limit) return { head: text, rest: "" };
  // Prefer the end of a line when there is one in the back half of the head; otherwise the last space before the limit.
  let c = text.lastIndexOf("\n", limit);
  if (c < limit / 2) { c = limit; while (c > 0 && !/\s/.test(text[c])) c--; }
  if (c === 0) { c = limit; if (/[\ud800-\udbff]/.test(text[c - 1])) c--; }
  while (c > 0 && /\s/.test(text[c - 1])) c--;
  return { head: text.slice(0, c), rest: text.slice(c) };
}
