/** Synthetic records: made-up instances in the record's real shape, so the observer shows every state word and
 *  every decision family without a real run for each. `base` is the time of the last line, so a caller picks
 *  "just now" (a held lock) or any fixed time (a test). Nothing here is read from, or claims to be, a real run. */
import type { Line } from "./record/types.ts";

export interface SyntheticInstance {
  id: string;
  name: string;
  /** Stands for the lock on the state directory: true when a process would hold it. */
  lockHeld: boolean;
  lines: Line[];
}

const MIN = 60_000;

class Rec {
  lines: Line[] = [];
  private seq = 0;
  private turn = 0;
  t: number;
  private rpd: number | null;
  constructor(t: number, rpd: number | null) {
    this.t = t;
    this.rpd = rpd;
  }
  add(kind: string, fields: Record<string, unknown> = {}, dt = 1000): Line {
    this.t += dt;
    const l: Line = { at: this.t, kind, seq: ++this.seq, ...fields };
    this.lines.push(l);
    return l;
  }
  start() {
    this.add("host.start", { pid: 4242, config: { engine: "agent", epoch_budget_tokens: 128000, ladder: ["model-a", "model-b"],
      governor: { quota: this.rpd ? { rpd: this.rpd, rpm: 20 } : null } } });
    this.add("epoch.rollover", { cause: "start", from: 0, to: 1, by: { rule: "nothing_to_ask" } });
  }
  /** One turn: boundary, three decisions, started, calls, an llm call, ended. */
  doTurn(o: { kind?: "free" | "committed" | "responding"; text?: string; calls?: [string, boolean][]; failed?: string; asked?: string; byModel?: boolean; cost?: number }) {
    const n = ++this.turn;
    const kind = o.kind ?? "free";
    const boundary = n;
    this.add("boundary", { n: boundary, mode: kind === "free" ? "free" : "committed", pending: o.asked ? 1 : 0 }, 4000);
    let ids: string[] = [];
    if (o.asked) {
      const id = `m${n}`;
      this.add("stimulus.accepted", { item: { id, role: "owner", kind: "peer", channel: "owner", firm: false, at: this.t, text: o.asked } });
      ids = [id];
    }
    const by = o.byModel ? { jev: { backend: "desk", model: "desk-1", cost_usd: 0.00004 } } : { rule: ids.length ? "shadow" : "nothing_to_ask" };
    this.add("decision.admit", { boundary, turn: n, by, choice: { forms: Object.fromEntries(ids.map((i) => [i, "now"])), interrupts: 0 }, answers: {}, wall_us: 3000 }, 5);
    this.add("decision.inject", { boundary, turn: n, by: o.byModel ? by : { rule: "shadow" }, choice: { calendar: true, cue: "none", expectations: false, recall: false },
      jev_choice: { calendar: true, cue: "none", expectations: false, recall: false }, agree: true, answers: {}, wall_us: 3000 }, 1);
    this.add("decision.tools", { boundary, turn: n, by: o.byModel ? by : { rule: "shadow" }, choice: { enabled: ["core", "memory", "read"] },
      jev_choice: { enabled: ["core"] }, agree: false, answers: {}, wall_us: 3000 }, 1);
    if (ids.length) this.add("stimulus.admitted", { turn: n, boundary, ids, digests: [] }, 1);
    this.add("turn.started", { turn: n, boundary, turn_kind: kind, mode: kind === "free" ? "free" : "committed:p1", model: "model-a", rung: 0, epoch: 1,
      pack_tokens: 20000 + n * 1500, header_tokens: 140, enabled: ["core", "memory", "read"], project: kind === "free" ? null : "p1" }, 5);
    const messages: unknown[] = [];
    (o.calls ?? []).forEach(([name, ok], i) => {
      this.add("tool.call", { turn: n, name, group: "read", ok }, 2000);
      messages.push({ role: "assistant", content: [{ type: "tool_use", id: `t${n}_${i}`, name, input: { path: `notes/${name}.md` } }] });
      messages.push({ role: "user", content: [{ type: "tool_result", tool_use_id: `t${n}_${i}`, content: ok ? "done" : "error: not found", is_error: !ok }] });
    });
    if (o.text) messages.push({ role: "assistant", content: o.text });
    this.add("llm.call", { turn: n, call: 1, epoch: 1, rung: 0, model_requested: "model-a", model_served: "model-a", provider: "router.example",
      prompt_tokens: 22000, cached_tokens: 20500, cache_write_tokens: 0, completion_tokens: 300, cost_usd: o.cost ?? 0, latency_ms: 2000 }, 3000);
    this.add("turn.log", { turn: n, header: `[turn ${n}]`, messages, recall: false }, 1);
    this.add("turn.ended", { turn: n, turn_kind: kind, status: o.failed ? "failed" : "completed", calls: (o.calls ?? []).length, elapsed_ms: 9000, model: "model-a", rung: 0,
      failure: o.failed ? { class: o.failed, origin: "provider" } : undefined, final_text: o.text ?? "", cost: { cached: 20500, calls: 1, cost_usd: o.cost ?? 0, prompt: 22000 } }, 1);
    return n;
  }
}

/** Build every synthetic instance so that the last line of each lands at `base`. Deterministic for a given `base`. */
export function synthetic(base: number): SyntheticInstance[] {
  const out: SyntheticInstance[] = [];
  const make = (id: string, lockHeld: boolean, rpd: number | null, build: (r: Rec) => void) => {
    const r = new Rec(0, rpd);
    // Build against a zero clock, then shift so the final line sits at `base`.
    build(r);
    const shift = base - r.t;
    for (const l of r.lines) { l.at += shift; shiftTimes(l, shift); }
    out.push({ id, name: id, lockHeld, lines: r.lines });
  };

  make("atlas", true, 1000, (r) => {
    r.start();
    r.doTurn({ text: "Read the notes folder and counted what the index covers.", calls: [["ws_list", true]] });
    r.add("kernel.commit", { turn: 1, by: "agent", project: "p1", title: "Tidy the notes index", done_when: "every note has an index entry", via: "tool:commit" });
    r.doTurn({ kind: "committed", text: "Wrote three index entries and checked the links. Two notes still lack a summary line.", calls: [["note_write", true], ["ws_list", true]] });
    r.add("kernel.progress", { turn: 3, project: "p1", next_step: "Write the two missing summary lines" });
    r.doTurn({ kind: "committed", text: "The index has 41 entries and the folder has 44 notes. Three are missing. I will list them, then write the entries in order.\n\nFirst the two from this week.", calls: [["ws_list", true], ["note_write", false]] });
    r.add("calendar.added", { entry: { id: "stand-up", origin: "owner", firm: true, text: "Stand-up: say what you are on", when: { at: base + 25 * MIN } } });
    r.add("decision.pack", { boundary: 3, turn: 3, by: { rule: "shadow" }, choice: { action: "append", cause: "pack", keep: [] }, jev_choice: { action: "rollover", cause: "pack", keep: [] }, answers: {}, wall_us: 4000 });
    r.add("decision.consolidate", { boundary: 3, turn: 3, by: { rule: "shadow" }, choice: { considered: ["c1", "c2", "c3"], retain: ["c1", "c3"], note_line: true }, answers: {}, wall_us: 4000 });
    r.add("boundary", { n: 4, mode: "committed", pending: 0 }, 2000);
  });

  make("bramble", true, 1000, (r) => {
    r.start();
    for (let i = 0; i < 4; i++) r.doTurn({ text: "Nothing to do." });
    r.add("boundary", { n: 5, mode: "free", pending: 0 }, 2000);
  });

  make("cedar", true, 1000, (r) => {
    r.start();
    r.doTurn({ text: "Read the inbox." });
    r.doTurn({ failed: "auth" });
    r.add("degraded", { class: "blocked", failure: { class: "auth", origin: "provider" }, why: "The model key was refused. Retrying every 15 min.", until: base + 15 * MIN, owner_wakes: true });
  });

  make("dune", true, 1000, (r) => {
    r.start();
    r.doTurn({ text: "Drafted the week's open questions." });
    r.add("degraded", { class: "paced", why: "Paced to the minute limit", until: base + 2 * MIN });
  });

  make("elm", true, 1000, (r) => {
    r.start();
    r.doTurn({ text: "Idle." });
    r.doTurn({ kind: "responding", asked: "Which notes still lack a summary line?", text: "Two: backoff and the digest. I will write both now.", calls: [["ws_list", true]], byModel: true, cost: 0.00031 });
    r.add("boundary", { n: 3, mode: "free", pending: 0 }, 2000);
  });

  make("fir", false, 1000, (r) => {
    // Died: no halt line, the lock is free.
    r.start();
    r.doTurn({ text: "Started the index." });
    r.doTurn({ kind: "free", text: "Wrote entry one.", calls: [["note_write", true]] });
  });

  make("gorse", false, 1000, (r) => {
    r.start();
    r.doTurn({ text: "Finished the almanac." });
    r.add("halted", { why: { by: "limit", why: "stopped" } });
  });

  return out;
}

function shiftTimes(l: Line, shift: number) {
  if (l.kind === "stimulus.accepted") l.item.at += shift;
}
