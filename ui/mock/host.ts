/** A simulated host: a numbered, gap-free record that grows, and subscribers that follow it. It speaks the doors in
 *  ui/contract/README.md and nothing else. It stands where H3 (read doors) and H5 (events) will stand; the contract tests
 *  are what both must pass. Not a model of the host's thinking: the lines have the real record's shapes, the content is invented. */
import type { ServerResponse } from "node:http";
import type { Line } from "../src/record/types.ts";
import { summarize } from "../src/record/folds.ts";
import { synthetic } from "../src/synthetic.ts";
import { makeRedactor } from "../src/record/redact.ts";

/** A reader whose unsent output passes this is dropped (contract: "a slow reader is dropped"). */
export const HIGH_WATER = 1024 * 1024;

interface Sub {
  res: ServerResponse;
  /** While the replay runs, live frames wait here so the order is replay, caught_up, live. */
  held: string[] | null;
}

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
  /** How many readers were dropped for being slow; the tests read it. */
  dropped = 0;

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
    this.broadcast(frame("record", this.out(line), line.seq));
    return line;
  }

  /** A short-lived delta: sent to whoever is following now, never numbered, never kept. */
  delta(d: Record<string, unknown>) { this.broadcast(frame("delta", this.out(d))); }

  private broadcast(f: string) {
    for (const s of this.subs) {
      if (s.held) { s.held.push(f); continue; }
      this.write(s, f);
    }
  }

  private write(s: Sub, f: string) {
    if (s.res.destroyed) { this.subs.delete(s); return; }
    s.res.write(f);
    if (s.res.writableLength > HIGH_WATER) { this.drop(s); }
  }

  private drop(s: Sub) {
    this.subs.delete(s);
    this.dropped++;
    s.res.destroy();
  }

  /** Follow the record from `after`: replay, then `caught_up`, then live lines and deltas. */
  async follow(res: ServerResponse, after: number) {
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache", "x-accel-buffering": "no" });
    const sub: Sub = { res, held: [] };
    this.subs.add(sub);
    const upTo = this.lines.length;
    const ka = setInterval(() => { if (!res.destroyed) res.write(": keepalive\n\n"); }, this.keepaliveMs);
    res.on("close", () => { clearInterval(ka); this.subs.delete(sub); });
    for (let i = after; i < upTo; i++) {
      if (res.destroyed) return;
      const l = this.lines[i];
      const ok = res.write(frame("record", this.out(l), l.seq));
      if (res.writableLength > HIGH_WATER) { this.drop(sub); return; }
      if (!ok) await new Promise<void>((r) => { res.once("drain", r); res.once("close", r); });
    }
    if (res.destroyed) return;
    res.write(frame("caught_up", { last_seq: upTo }));
    const held = sub.held!;
    sub.held = null;
    for (const f of held) this.write(sub, f); // lines written during the replay, after the snapshot
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

  /** `n` quick lines, with a delta before each: what the tests poke the host with. */
  async poke(n: number) {
    for (let i = 0; i < n; i++) {
      this.delta({ turn: this.turn + 1, kind: "text", text: `thinking ${i + 1}… ` });
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
    const say = async (ws: string[]) => {
      for (let i = 0; i < ws.length; i += 4) { this.delta({ turn: n, kind: "text", text: ws.slice(i, i + 4).join(" ") + " " }); if (pace) await sleep(pace); }
    };
    await say(words.slice(0, half));
    for (let i = 0; i < tools.length; i++) {
      this.delta({ turn: n, kind: "tool", name: tools[i], phase: "start" });
      if (pace) await sleep(pace * 3);
      this.delta({ turn: n, kind: "tool", name: tools[i], phase: "end" });
      this.append("tool.call", { turn: n, name: tools[i], group: "read", ok: true });
      messages.push({ role: "assistant", content: [{ type: "tool_use", id: `t${n}_${i}`, name: tools[i], input: { path: `notes/${tools[i]}.md` } }] });
      messages.push({ role: "user", content: [{ type: "tool_result", tool_use_id: `t${n}_${i}`, content: "done", is_error: false }] });
    }
    await say(words.slice(half));
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

  stop() { this.stopped = true; for (const s of [...this.subs]) s.res.end(); }
}
