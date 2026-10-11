/** A simulated host: a numbered, gap-free record that grows, and subscribers that follow it. It speaks the doors in
 *  ui/contract/README.md and nothing else. It stands where H3 (read doors) and H5 (events) will stand; the contract tests
 *  are what both must pass. Not a model of the host's thinking: the lines have the real record's shapes, the content is invented. */
import type { ServerResponse } from "node:http";
import type { Line } from "../src/record/types.ts";
import { summarize } from "../src/record/folds.ts";
import { synthetic } from "../src/synthetic.ts";
import { makeRedactor } from "../src/record/redact.ts";

/** A reader's partial frames wait in a buffer this big; past it the reader is told how many it missed (H5, flow control). */
const PARTIAL_BUFFER = 4096;

interface Sub {
  res: ServerResponse;
  partials: boolean;
  records: boolean;
  /** The next line this reader has not been sent. Record frames are pulled: a reader that does not read stays where it is. */
  next: number;
  queue: string[];
  skipped: number;
  wake: (() => void) | null;
  closed: boolean;
}

/** One server-sent event: only a record frame has an id. */
export const frame = (event: string, data: unknown, id?: number): string =>
  `${id !== undefined ? `id: ${id}\n` : ""}event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const PARAGRAPHS = [
  "The index has 41 entries and the folder has 44 notes. Three are missing: the two from this week and the one about backoff.",
  "I will list them first, then write the entries in order, checking each link as I go.",
  "Two notes still lack a summary line. I will draft those after the entries, so the index is whole before I stop.",
  "That settles the folder. Next the open questions: which of them can wait for the weekly note, and which the owner should see today.",
  "The backoff note is the longest; its summary needs the three cases in one sentence each, so I will read it again before writing.",
];

export class SimHost {
  readonly lines: Line[] = [];
  private subs = new Set<Sub>();
  private turn = 0;
  private stopped = false;
  keepaliveMs = 15_000;
  /** A test puts a line of its own in the summary, as a host that did not redact would. */
  doingOverride: string | null = null;
  /** The host redacts at every door and on the stream before it writes (contract: "the host redacts first"). A test turns it off to
   *  model a host that has not been given its redactor yet, so the page's own second line can be shown to hold. */
  redactDoors = true;
  private scrub = makeRedactor([]);
  out<T>(v: T): T { return this.redactDoors ? (this.scrub(v) as T) : v; }

  constructor(seed: string | false = "atlas") {
    if (seed) {
      const from = synthetic(Date.now()).find((i) => i.id === seed)!;
      for (const l of from.lines) this.lines.push({ ...l });
      this.turn = Math.max(0, ...this.lines.map((l) => (typeof l.turn === "number" ? l.turn : 0)));
    } else this.start();
  }

  private start() {
    this.append("host.start", { pid: 1, config: { engine: "agent", epoch_budget_tokens: 128000, governor: { quota: { rpd: 1000, rpm: 20 } } } });
  }

  get lastSeq(): number { return this.lines.length; }

  append(kind: string, fields: Record<string, unknown> = {}): Line {
    const line: Line = { at: Date.now(), kind, seq: this.lines.length + 1, ...fields };
    this.lines.push(line);
    this.wakeAll();
    return line;
  }

  /** A partial frame: given to whoever is connected now, never numbered, never kept. */
  delta(d: Record<string, unknown>) {
    const f = frame("delta", this.out(d));
    for (const s of this.subs) {
      if (!s.partials) continue;
      if (s.queue.length >= PARTIAL_BUFFER) { s.skipped++; continue; }
      s.queue.push(f);
      s.wake?.();
    }
  }

  private wakeAll() { for (const s of this.subs) s.wake?.(); }

  /** Follow the record from `after` (H5): an `open` frame, every line after the cursor once and in order, partial frames while
   *  connected, and an `end` frame when the host stops. Lines are pulled as the reader takes them. */
  async follow(res: ServerResponse, after: number, mode: "on" | "off" | "only" = "on") {
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache, no-transform", "x-accel-buffering": "no" });
    const sub: Sub = { res, partials: mode !== "off" && !this.stopped, records: mode !== "only", next: after, queue: [], skipped: 0, wake: null, closed: false };
    this.subs.add(sub);
    const ka = setInterval(() => { if (!res.destroyed) res.write(": keepalive\n\n"); }, this.keepaliveMs);
    const done = () => { sub.closed = true; clearInterval(ka); this.subs.delete(sub); sub.wake?.(); };
    res.on("close", done);
    const send = async (f: string) => {
      if (res.destroyed) return;
      if (!res.write(f)) await new Promise<void>((r) => { res.once("drain", r); res.once("close", r); });
    };
    await send(frame("open", { contract: 1, after, live: !this.stopped, partials: sub.partials, redaction: { env_unset: [] } }));
    while (!sub.closed) {
      let moved = false;
      if (sub.skipped) { const n = sub.skipped; sub.skipped = 0; await send(frame("delta", { turn: 0, call: 0, kind: "behind", skipped: n })); moved = true; }
      while (sub.queue.length && !sub.closed) { await send(sub.queue.shift()!); moved = true; }
      while (sub.records && sub.next < this.lines.length && !sub.closed) {
        const l = this.lines[sub.next++];
        await send(frame("record", this.out(l), l.seq));
        moved = true;
      }
      if (sub.closed) break;
      // A reader that is sent no record frames (partials=only) has nothing to replay: it is through once the host has stopped.
      if (this.stopped && (!sub.records || sub.next >= this.lines.length) && !sub.queue.length) {
        const last = this.lines.at(-1);
        await send(frame("end", { reason: last?.kind === "halted" ? "stopped" : "down", last_seq: this.lines.length, problem: null }));
        res.end();
        break;
      }
      if (!moved) await new Promise<void>((r) => { sub.wake = r; });
      sub.wake = null;
    }
    done();
  }

  summary() {
    const s = summarize(this.lines, { lockHeld: !this.stopped, now: Date.now() });
    const last = this.lines.at(-1)!;
    return this.out({
      contract: 1, state: s.state, doing: this.doingOverride ?? s.doing, needs_you: s.needsYou, turn: s.lastTurn,
      last_seq: last.seq, last_at: last.at, requests: s.requests, quota: s.quota, spend_usd_day: s.spendDayUsd,
    });
  }

  // -- writing -------------------------------------------------------------------------------------------------

  /** `n` quick lines: what the tests poke the host with. */
  async poke(n: number) {
    for (let i = 0; i < n; i++) {
      this.append("note.written", { turn: this.turn + 1, text: `a note, number ${i + 1}` });
      await sleep(2);
    }
  }

  /** `n` lines of about `bytes` each, written as fast as a reader that keeps up can take them. */
  async flood(n: number, bytes: number) {
    const text = "x".repeat(bytes);
    for (let i = 0; i < n; i++) {
      this.append("note.written", { turn: this.turn + 1, text });
      if (i % 4 === 3) await new Promise((r) => setImmediate(r));
    }
  }

  /** One whole turn, as a live host writes it: the deltas stream while the model writes and a tool runs; the record gets
   *  the finished turn at its end. `pace` is the pause between pieces (0 for none). */
  async runTurn(opts: { pace?: number; kind?: "free" | "committed" | "responding"; asked?: string; tools?: string[]; text?: string } = {}) {
    const pace = opts.pace ?? 150;
    const n = ++this.turn;
    const kind = opts.kind ?? "committed";
    const boundary = n + 1;
    this.append("boundary", { n: boundary, mode: kind === "free" ? "free" : "committed", pending: opts.asked ? 1 : 0 });
    let ids: string[] = [];
    if (opts.asked) {
      const id = `m${n}`;
      this.append("stimulus.accepted", { item: { id, role: "owner", kind: "peer", channel: "owner", firm: false, at: Date.now(), text: opts.asked } });
      ids = [id];
    }
    const by = { rule: ids.length ? "shadow" : "nothing_to_ask" };
    this.append("decision.admit", { boundary, turn: n, by, choice: { forms: Object.fromEntries(ids.map((i) => [i, "now"])), interrupts: 0 }, answers: {}, wall_us: 2000 });
    this.append("decision.inject", { boundary, turn: n, by: { rule: "shadow" }, choice: { calendar: true, cue: "none", expectations: false, recall: false }, jev_choice: { calendar: true, cue: "none", expectations: false, recall: false }, agree: true, answers: {}, wall_us: 2000 });
    this.append("decision.tools", { boundary, turn: n, by: { rule: "shadow" }, choice: { enabled: ["core", "memory", "read"] }, jev_choice: { enabled: ["core"] }, agree: false, answers: {}, wall_us: 2000 });
    if (ids.length) this.append("stimulus.admitted", { turn: n, boundary, ids, digests: [] });
    const started = Date.now();
    this.append("turn.started", { turn: n, boundary, turn_kind: kind, mode: kind === "free" ? "free" : "committed:p1", model: "model-a", rung: 0, epoch: 1, pack_tokens: 20000 + n * 900, header_tokens: 140, enabled: ["core", "memory", "read"], project: kind === "free" ? null : "p1" });
    const text = opts.text ?? [PARAGRAPHS[n % PARAGRAPHS.length], PARAGRAPHS[(n + 2) % PARAGRAPHS.length]].join("\n\n");
    const messages: unknown[] = [];
    const tools = opts.tools ?? ["ws_list", "note_write"];
    // The model writes a little, runs a tool, writes the rest.
    const words = text.split(" ");
    const half = Math.floor(words.length / 2);
    const f = (call: number, rest: Record<string, unknown>) => ({ turn: n, call, ...rest });
    // The model's answer text is a block: it begins, arrives in pieces, ends. A tool call is a block too (its arguments arrive in pieces).
    const say = async (call: number, ws: string[]) => {
      this.delta(f(call, { kind: "block_start", id: null }));
      for (let i = 0; i < ws.length; i += 4) { this.delta(f(call, { kind: "text", text: ws.slice(i, i + 4).join(" ") + " " })); if (pace) await sleep(pace); }
      this.delta(f(call, { kind: "block_end", id: null }));
    };
    await say(0, words.slice(0, half));
    for (let i = 0; i < tools.length; i++) {
      const id = `t${n}_${i}`;
      const input = { path: `notes/${tools[i]}.md` };
      this.delta(f(0, { kind: "tool_call", id, name: tools[i] }));
      const args = JSON.stringify(input);
      this.delta(f(0, { kind: "tool_input", id, delta: args.slice(0, 8) }));
      this.delta(f(0, { kind: "tool_input", id, delta: args.slice(8) }));
      this.delta(f(0, { kind: "block_end", id }));
      this.delta(f(0, { kind: "tool_run", name: tools[i], input }));
      if (pace) await sleep(pace * 3);
      this.delta(f(0, { kind: "tool_done", name: tools[i], ok: true, output: "done" }));
      this.append("tool.call", { turn: n, name: tools[i], group: "read", ok: true });
      messages.push({ role: "assistant", content: [{ type: "tool_use", id, name: tools[i], input }] });
      messages.push({ role: "user", content: [{ type: "tool_result", tool_use_id: id, content: "done", is_error: false }] });
    }
    await say(1, words.slice(half));
    this.delta(f(1, { kind: "turn_end" }));
    if (kind === "committed") this.append("kernel.progress", { turn: n, project: "p1", next_step: "Write the next index entry" });
    messages.push({ role: "assistant", content: text });
    this.append("llm.call", { turn: n, call: 1, epoch: 1, rung: 0, model_requested: "model-a", model_served: "model-a", provider: "router.example", prompt_tokens: 22000, cached_tokens: 20500, cache_write_tokens: 0, completion_tokens: 300, cost_usd: 0, latency_ms: Date.now() - started });
    this.append("turn.log", { turn: n, header: `[turn ${n}]`, messages, recall: false });
    this.append("turn.ended", { turn: n, turn_kind: kind, status: "completed", calls: tools.length, elapsed_ms: Date.now() - started, model: "model-a", rung: 0, final_text: text, cost: { cached: 20500, calls: 1, cost_usd: 0, prompt: 22000 } });
    return n;
  }

  /** Keep running turns, a pause between, until stopped: what the viewable mock does. */
  async live(gapMs = 9000) {
    while (!this.stopped) {
      await this.runTurn({ pace: 160 });
      await sleep(gapMs);
    }
  }

  /** Close every reader, as a restart or a dropped link would. They resume from their last number. */
  cutAll() { for (const s of [...this.subs]) s.res.destroy(); this.subs.clear(); }

  /** The host stops itself: a halted line, then every stream ends with an `end` frame (the lock is released). */
  async halt() { this.append("halted", { why: { by: "limit", why: "stopped" } }); this.stopped = true; this.wakeAll(); await sleep(50); }

  stop() { this.stopped = true; this.wakeAll(); for (const s of [...this.subs]) s.res.destroy(); }
}
