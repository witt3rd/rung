/** The queue doors of a simulated host, with the real doors' bodies (docs/rung-host-api.md, "S1 — the queue"): `GET /v1/queue` is the
 *  record's items, each with a state, ordered for a page and paged; `POST /v1/queue` writes a `stimulus.accepted` line before it
 *  answers, and the key is the item's id, so a repeat adds once. A stopped instance lists and refuses an add. The same checks in
 *  test/contract-queue.test.ts run against the real gateway and host; this is a scripted stand-in, not a model of the desk: it holds
 *  back only what a test says to (a `decision.admit` line naming the item). */
import type { IncomingMessage, ServerResponse } from "node:http";
import type { SimHost } from "./host.ts";
import type { Line } from "../src/record/types.ts";

const send = (res: ServerResponse, status: number, body: unknown) => { res.writeHead(status, { "content-type": "application/json" }).end(JSON.stringify(body)); };
const refuse = (res: ServerResponse, status: number, error: string, message: string) => send(res, status, { error, message });

type State = "waiting" | "answering" | "answered" | "withdrawn";
const STATES: State[] = ["waiting", "answering", "answered", "withdrawn"];

interface Entry { id: string; order: number; item: Line; state: State; turn: number | null; disposition: string | null; settled: number }

/** The record's queue, as the host's fold reads it: accepted, admitted into a turn (answering until it is disposed), requeued, disposed. */
export function foldQueue(lines: readonly Line[]) {
  const entries = new Map<string, Entry>();
  let forms: Record<string, string> = {};
  for (const l of lines) {
    if (l.kind === "stimulus.accepted") {
      if (l.item.control || entries.has(l.item.id)) continue;
      entries.set(l.item.id, { id: l.item.id, order: l.seq, item: l.item, state: "waiting", turn: null, disposition: null, settled: 0 });
    } else if (l.kind === "stimulus.admitted") {
      for (const id of [...(l.ids ?? []), ...(l.digests ?? [])] as string[]) {
        const e = entries.get(id);
        if (e && e.state === "waiting") { e.state = "answering"; e.turn = l.turn; }
      }
    } else if (l.kind === "stimulus.requeued") {
      for (const id of (l.ids ?? []) as string[]) { const e = entries.get(id); if (e && e.state === "answering") { e.state = "waiting"; e.turn = null; } }
    } else if (l.kind === "stimulus.disposed") {
      const e = entries.get(l.id);
      if (e) { e.state = l.disposition === "withdrawn" ? "withdrawn" : "answered"; e.disposition = l.disposition ?? null; e.turn = typeof l.turn === "number" ? l.turn : e.turn; e.settled = l.seq; }
    } else if (l.kind === "decision.admit") forms = l.choice?.forms ?? {};
  }
  const rank = (e: Entry) => (e.item.role === "owner" ? 0 : e.item.role === "peer" ? 1 : 2);
  const all = [...entries.values()];
  const answering = all.filter((e) => e.state === "answering").sort((a, b) => a.order - b.order);
  const waiting = all.filter((e) => e.state === "waiting").sort((a, b) => rank(a) - rank(b) || a.order - b.order);
  const over = all.filter((e) => e.state === "answered" || e.state === "withdrawn").sort((a, b) => b.settled - a.settled);
  const held = (e: Entry): string | null =>
    e.state !== "waiting" ? null
      : forms[e.id] === "at_break" ? "held by the decision desk until the agent's next break"
      : forms[e.id] === "digest" ? "held by the decision desk: it will be one line in the next turn header, not shown in full" : null;
  const json = (e: Entry, position: number | null) => {
    const role = String(e.item.role ?? "");
    const channel = String(e.item.channel ?? "");
    return {
      id: e.id, text: e.item.text ?? "", role, name: role === "owner" ? null : role === "peer" ? (channel.includes(":") ? channel.split(":").slice(1).join(":") : channel) : channel,
      channel, kind: e.item.kind ?? null, intent: typeof e.item.intent === "string" ? e.item.intent : "next", order: e.order, position,
      accepted_at: typeof e.item.at === "number" ? e.item.at : null, state: e.state, turn: e.turn, disposition: e.disposition, held: held(e),
    };
  };
  const sorted = [...answering.map((e) => json(e, null)), ...waiting.map((e, i) => json(e, i + 1)), ...over.map((e) => json(e, null))];
  const counts = Object.fromEntries(STATES.map((s) => [s, all.filter((e) => e.state === s).length]));
  return { sorted, counts, find: (id: string) => sorted.find((i) => i.id === id) };
}

function list(host: SimHost, url: URL, res: ServerResponse) {
  const q = url.searchParams;
  const names = [...q.keys()];
  const bad = (m: string) => refuse(res, 400, "bad_request", m);
  if (new Set(names).size !== names.length || names.some((n) => !["state", "offset", "limit"].includes(n))) return bad("a query name is repeated or not known");
  let states: State[] = ["waiting", "answering"];
  if (q.has("state")) {
    const parts = q.get("state")!.split(",");
    if (parts.some((p) => ![...STATES, "open", "all"].includes(p))) return bad("state is not a list of waiting, answering, answered, withdrawn, open or all");
    states = [...new Set(parts.flatMap((p) => (p === "all" ? STATES : p === "open" ? (["waiting", "answering"] as State[]) : [p as State])))];
  }
  const num = (name: string, dflt: number, min: number): number | null => {
    const v = q.get(name);
    if (v === null) return dflt;
    return /^\d+$/.test(v) && Number(v) >= min ? Number(v) : null;
  };
  const offset = num("offset", 0, 0), limit = num("limit", 500, 1);
  if (offset === null || limit === null) return bad("offset or limit is not a whole number (limit at least 1)");
  const f = foldQueue(host.lines);
  const asked = f.sorted.filter((i) => states.includes(i.state as State));
  const page = asked.slice(offset, offset + limit);
  send(res, 200, host.out({
    items: page, total: asked.length, offset, limit, next: offset + limit < asked.length ? offset + limit : null,
    counts: f.counts, record_lines: host.lastSeq, damaged: null, redaction: { env_unset: [] },
  }));
}

async function body(req: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const c of req) chunks.push(c as Buffer);
  return Buffer.concat(chunks).toString("utf8");
}

const KEY = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,199}$/;

export async function handleQueue(host: SimHost, req: IncomingMessage, res: ServerResponse): Promise<void> {
  const url = new URL(req.url!, "http://host");
  if (req.method === "GET" || req.method === "HEAD") return list(host, url, res);
  if (req.method !== "POST") return refuse(res, 405, "method_not_allowed", "GET, HEAD or POST");
  if (host.isStopped) return refuse(res, 409, "not_running", "the instance is not running: start it first (the gateway has no start door); nothing was sent");
  if (!String(req.headers["content-type"] ?? "").toLowerCase().startsWith("application/json")) return refuse(res, 415, "unsupported_media_type", "the body must be application/json");
  const bad = (m: string) => refuse(res, 400, "bad_request", m);
  let b: Record<string, unknown>;
  try { b = JSON.parse(await body(req)); } catch { return bad("the body is not JSON"); }
  if (typeof b !== "object" || b === null || Array.isArray(b)) return bad("the body is not an object");
  for (const k of Object.keys(b)) if (!["text", "key", "intent"].includes(k)) return bad(`${k} is not a field of this door`);
  if (typeof b.text !== "string" || b.text.trim() === "") return bad("text is required and must not be blank");
  if (b.key !== undefined && (typeof b.key !== "string" || !KEY.test(b.key))) return bad("key is 1 to 200 letters, digits, . _ : -, starting with a letter or digit");
  if (b.intent !== undefined && b.intent !== "next") return bad(b.intent === "steer" || b.intent === "free" ? `intent ${b.intent} is not served yet: only next` : "intent is not an intent");
  const id = (b.key as string | undefined) ?? `q-${host.lastSeq + 1}`;
  const have = foldQueue(host.lines).find(id);
  if (have) {
    if (have.text !== b.text) return refuse(res, 409, "key_reused", "the key already names a different message; nothing written");
    return send(res, 200, host.out({ id, repeat: true, item: have }));
  }
  host.append("stimulus.accepted", { item: { id, kind: "peer", role: "owner", channel: "owner", firm: false, at: Date.now(), text: b.text } });
  send(res, 201, host.out({ id, repeat: false, item: foldQueue(host.lines).find(id) }));
}
