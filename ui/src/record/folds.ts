/** Folds over a record: pure functions from lines to what a screen shows. The same results are what a host's
 *  read doors are to return (docs/rung-host-api.md); until they exist the observer folds the recorded lines itself. */
import type { Line } from "./types.ts";
import { familyQuestion, firstLine, sayChoice, sayFailure, sayHalt, sayWho, type StateWord, num } from "./words.ts";

/** Thresholds are settings, tuned on live runs (design note, "Several instances"). */
export const THRESHOLDS = { stuckTurnsWithoutProgress: 10, idleFreeTurns: 3 };

export interface Decision {
  seq: number;
  at: number;
  family: string;
  boundary: number | null;
  turn: number | null;
  question: string;
  answer: string;
  /** For a shadowed model: what it would have answered, when that differs. */
  modelWould: string | null;
  who: "rule" | "model" | "unknown";
  label: string;
  /** Why this decider answered; null only when the line records no `by`, which the page test refuses. */
  reason: string | null;
  wallMs: number | null;
  /** Nothing to show for the owner: no item to take, nothing extra shown. */
  trivial: boolean;
}

export interface ToolCall {
  name: string;
  group: string | null;
  ok: boolean | null;
  refused: string | null;
  input: unknown;
  result: string | null;
}

export interface Turn {
  n: number;
  kind: string;
  mode: string | null;
  project: string | null;
  model: string | null;
  epoch: number | null;
  startedAt: number;
  endedAt: number | null;
  status: "running" | "completed" | "failed" | "bounded" | "interrupted";
  failure: string | null;
  text: string;
  asked: { id: string; from: string; text: string }[];
  calls: ToolCall[];
  decisions: Decision[];
  llmCalls: number;
  promptTokens: number;
  cachedTokens: number;
  completionTokens: number;
  costUsd: number;
  elapsedMs: number | null;
  firstSeq: number;
  lastSeq: number;
}

const blocks = (content: unknown): Line[] => (Array.isArray(content) ? (content as Line[]) : []);

export function foldDecisions(lines: readonly Line[]): Decision[] {
  const out: Decision[] = [];
  for (const l of lines) {
    if (!l.kind.startsWith("decision.")) continue;
    const family = l.kind.slice("decision.".length);
    const who = sayWho(l.by);
    const answer = sayChoice(family, l.choice);
    const would = l.jev_choice !== undefined ? sayChoice(family, l.jev_choice) : null;
    out.push({
      seq: l.seq, at: l.at, family, boundary: l.boundary ?? null, turn: l.turn ?? null,
      question: familyQuestion(family), answer,
      modelWould: would !== null && would !== answer ? would : null,
      who: who.who, label: who.label, reason: who.reason,
      wallMs: typeof l.wall_us === "number" ? l.wall_us / 1000 : null,
      trivial: (family === "admit" && Object.keys(l.choice?.forms ?? {}).length === 0) || (family === "inject" && !l.choice?.calendar && !l.choice?.expectations && !l.choice?.recall),
    });
  }
  return out;
}

/** `live`: a turn with no end line is running. For a record with no process it was cut off. */
export function foldTurns(lines: readonly Line[], opts: { live: boolean } = { live: false }): Turn[] {
  const accepted = new Map<string, Line>();
  for (const l of lines) if (l.kind === "stimulus.accepted") accepted.set(l.item.id, l.item);
  const decisions = foldDecisions(lines);
  const byTurn = new Map<number, Turn>();
  const get = (n: number, at: number, seq: number): Turn => {
    let t = byTurn.get(n);
    if (!t) {
      t = { n, kind: "free", mode: null, project: null, model: null, epoch: null, startedAt: at, endedAt: null, status: opts.live ? "running" : "interrupted", failure: null,
        text: "", asked: [], calls: [], decisions: [], llmCalls: 0, promptTokens: 0, cachedTokens: 0, completionTokens: 0, costUsd: 0,
        elapsedMs: null, firstSeq: seq, lastSeq: seq };
      byTurn.set(n, t);
    }
    t.lastSeq = Math.max(t.lastSeq, seq);
    return t;
  };
  const logged = new Set<number>();
  const started = new Set<number>();
  for (const l of lines) {
    if (typeof l.turn !== "number" || l.turn < 1) continue;
    const t = get(l.turn, l.at, l.seq);
    switch (l.kind) {
      case "turn.started":
        started.add(l.turn);
        t.startedAt = l.at; t.firstSeq = Math.min(t.firstSeq, l.seq);
        t.kind = l.turn_kind ?? t.kind; t.mode = l.mode ?? null; t.model = l.model ?? null; t.epoch = l.epoch ?? null;
        t.project = typeof l.project === "string" ? l.project : null;
        break;
      case "stimulus.admitted":
        for (const id of (l.ids ?? []) as string[]) {
          const it = accepted.get(id);
          if (it) t.asked.push({ id, from: it.role === "owner" ? "owner" : String(it.kind ?? it.role), text: String(it.text ?? "") });
        }
        break;
      case "tool.call":
        if (!logged.has(t.n)) t.calls.push({ name: l.name, group: l.group ?? null, ok: l.ok ?? null, refused: null, input: undefined, result: null });
        break;
      case "tool.refused":
        t.calls.push({ name: l.name, group: l.group ?? null, ok: false, refused: String(l.why ?? "refused"), input: undefined, result: l.message ?? null });
        break;
      case "llm.call":
        t.llmCalls += 1; t.promptTokens += l.prompt_tokens ?? 0; t.cachedTokens += l.cached_tokens ?? 0;
        t.completionTokens += l.completion_tokens ?? 0; t.costUsd += l.cost_usd ?? 0;
        break;
      case "turn.log": {
        // The messages as sent give each call its input and result and the text as the model wrote it.
        logged.add(t.n);
        t.calls = t.calls.filter((c) => c.refused !== null);
        const results = new Map<string, Line>();
        const texts: string[] = [];
        const msgs = (l.messages ?? []) as Line[];
        for (const m of msgs) if (m.role === "user") for (const b of blocks(m.content)) if (b.type === "tool_result") results.set(b.tool_use_id, b);
        for (const m of msgs) {
          if (m.role !== "assistant") continue;
          if (typeof m.content === "string") { if (m.content.trim()) texts.push(m.content.trim()); continue; }
          for (const b of blocks(m.content)) {
            if (b.type === "text" && String(b.text ?? "").trim()) texts.push(String(b.text).trim());
            if (b.type === "tool_use") {
              const r = results.get(b.id);
              const res = r ? (typeof r.content === "string" ? r.content : JSON.stringify(r.content)) : null;
              t.calls.push({ name: b.name, group: null, ok: r ? !r.is_error : null, refused: null, input: b.input, result: res });
            }
          }
        }
        t.text = texts.join("\n\n");
        break;
      }
      case "turn.ended": {
        t.endedAt = l.at;
        t.status = l.status === "completed" || l.status === "bounded" ? l.status : "failed";
        t.failure = l.failure && l.failure.class ? sayFailure(l.failure) : null;
        t.elapsedMs = l.elapsed_ms ?? null;
        t.kind = l.turn_kind ?? t.kind;
        t.model = l.model ?? t.model;
        const ft = String(l.final_text ?? "");
        if (ft.trim() && !t.text.includes(ft.trim())) t.text = t.text ? `${t.text}\n\n${ft.trim()}` : ft.trim();
        break;
      }
    }
  }
  for (const d of decisions) if (d.turn !== null && byTurn.has(d.turn)) byTurn.get(d.turn)!.decisions.push(d);
  // A boundary decides for the turn that follows it; a turn that never started (the host halted at that boundary) is not a turn.
  return [...byTurn.values()].filter((t) => started.has(t.n)).sort((a, b) => a.n - b.n);
}

/** One line for a turn in a list: what it said, or what went wrong. A summary; the turn page holds the text whole. */
export function turnLine(t: Turn): string {
  const s = firstLine(t.text);
  if (s) return s;
  if (t.status === "running") return "In progress";
  if (t.status === "interrupted") return "Cut off before it finished";
  if (t.failure) return `No reply (${t.failure})`;
  return "No reply";
}

export interface DaySpend { requests: number; promptTokens: number; cachedTokens: number; completionTokens: number; costUsd: number; deskUsd: number }
const emptyDay = (): DaySpend => ({ requests: 0, promptTokens: 0, cachedTokens: 0, completionTokens: 0, costUsd: 0, deskUsd: 0 });
export const utcDay = (ms: number): string => new Date(ms).toISOString().slice(0, 10);

export function foldSpend(lines: readonly Line[]): { byDay: Record<string, DaySpend>; byModel: Record<string, DaySpend> } {
  const byDay: Record<string, DaySpend> = {};
  const byModel: Record<string, DaySpend> = {};
  const add = (m: DaySpend, l: Line) => {
    if (l.kind === "llm.call") {
      m.requests += 1; m.promptTokens += l.prompt_tokens ?? 0; m.cachedTokens += l.cached_tokens ?? 0;
      m.completionTokens += l.completion_tokens ?? 0; m.costUsd += l.cost_usd ?? 0;
    } else m.deskUsd += l.cost_usd ?? 0;
  };
  for (const l of lines) {
    if (l.kind !== "llm.call" && l.kind !== "desk.ask") continue;
    add((byDay[utcDay(l.at)] ??= emptyDay()), l);
    if (l.kind === "llm.call") add((byModel[l.model_served ?? l.model_requested ?? "unknown"] ??= emptyDay()), l);
  }
  return { byDay, byModel };
}

export interface Pack {
  epoch: number | null;
  contextTokens: number | null;
  budgetTokens: number | null;
  contextPct: number | null;
  cachePct: number | null;
  rollovers: { at: number; cause: string; from: number; to: number }[];
  cacheBreaks: { turn: number; call: number; cause: string }[];
}

// biome-ignore lint: open config shape
const hostConfig = (lines: readonly Line[]): Record<string, any> => {
  for (let i = lines.length - 1; i >= 0; i--) if (lines[i].kind === "host.start") return lines[i].config ?? {};
  return {};
};

export function foldPack(lines: readonly Line[]): Pack {
  const budget = Number(hostConfig(lines).epoch_budget_tokens) || null;
  let started: Line | null = null;
  for (let i = lines.length - 1; i >= 0 && !started; i--) if (lines[i].kind === "turn.started") started = lines[i];
  const epoch = started?.epoch ?? null;
  let prompt = 0, cached = 0;
  const rollovers: Pack["rollovers"] = [];
  const cacheBreaks: Pack["cacheBreaks"] = [];
  for (const l of lines) {
    if (l.kind === "llm.call" && l.epoch === epoch) { prompt += l.prompt_tokens ?? 0; cached += l.cached_tokens ?? 0; }
    if (l.kind === "epoch.rollover") rollovers.push({ at: l.at, cause: String(l.cause ?? ""), from: l.from, to: l.to });
    if (l.kind === "cache.break") cacheBreaks.push({ turn: l.turn, call: l.call, cause: String(l.cause ?? "") });
  }
  const ctx = started ? Number(started.pack_tokens) : null;
  return {
    epoch, contextTokens: ctx, budgetTokens: budget,
    contextPct: ctx !== null && budget ? Math.round((ctx / budget) * 100) : null,
    cachePct: prompt > 0 ? Math.round((cached / prompt) * 100) : null,
    rollovers, cacheBreaks,
  };
}

export interface Summary {
  state: StateWord;
  /** One line on what it is doing (or did last). */
  doing: string;
  needsYou: boolean;
  turns: number;
  lastTurn: number | null;
  lastAt: number;
  requests: number;
  quota: number | null;
  requestsDay: string;
  spendDayUsd: number;
  project: { title: string; doneWhen: string | null; sinceTurn: number } | null;
  idle: boolean;
  next: { text: string; at: number } | null;
  waiting: number;
  lastDecision: Decision | null;
  context: number | null;
  cache: number | null;
}

/** The state word comes from the record and from whether the lock on the state directory is held, never from a guess.
 *  `lockHeld` is that fact. A recorded run has no process, so it is false: the run either halted (Stopped) or did not (Down). */
export function summarize(lines: readonly Line[], opts: { lockHeld: boolean; now: number }): Summary {
  const { lockHeld, now } = opts;
  const turns = foldTurns(lines, { live: lockHeld });
  const decisions = foldDecisions(lines);
  const last = lines[lines.length - 1];
  const lastAt = last?.at ?? now;
  const lastTurn = turns.length ? turns[turns.length - 1] : null;

  // The open commitment: the latest commit with no later release.
  let project: Summary["project"] = null;
  let lastProgressTurn = 0;
  for (const l of lines) {
    if (l.kind === "kernel.commit") { project = { title: String(l.title ?? l.project ?? ""), doneWhen: l.done_when ?? null, sinceTurn: l.turn ?? 0 }; lastProgressTurn = l.turn ?? 0; }
    else if (l.kind === "kernel.release") project = null;
    else if (l.kind === "kernel.progress" && project) lastProgressTurn = l.turn ?? lastProgressTurn;
  }

  // An open degraded period: a `degraded` with no later `degraded.ended`.
  let open: Line | null = null;
  for (const l of lines) { if (l.kind === "degraded") open = l; else if (l.kind === "degraded.ended") open = null; }

  const spend = foldSpend(lines);
  const pack = foldPack(lines);
  const quota = Number(hostConfig(lines).governor?.quota?.rpd) || null;
  const refDay = utcDay(lockHeld ? now : lastAt);
  const day = spend.byDay[refDay] ?? emptyDay();

  const calendar = new Map<string, Record<string, any>>();
  for (const l of lines) {
    if (l.kind === "calendar.added") calendar.set(l.entry.id, l.entry);
    else if (l.kind === "calendar.removed" || l.kind === "calendar.skipped") calendar.delete(l.id);
    else if (l.kind === "calendar.fired") calendar.delete(l.id);
  }
  const upcoming = [...calendar.values()].filter((e) => typeof e.when?.at === "number").sort((a, b) => a.when.at - b.when.at)[0];

  const accepted = new Set<string>();
  for (const l of lines) {
    if (l.kind === "stimulus.accepted") accepted.add(l.item.id);
    else if (l.kind === "stimulus.disposed") accepted.delete(l.id);
  }

  const freeTail = turns.filter((t) => t.kind === "free").slice(-THRESHOLDS.idleFreeTurns);
  const idle = freeTail.length === THRESHOLDS.idleFreeTurns && freeTail.every((t) => t.calls.length === 0);

  const sinceProgress = lastTurn && project ? lastTurn.n - lastProgressTurn : 0;
  let state: StateWord;
  let doing: string;
  if (!lockHeld) {
    if (last?.kind === "halted") { state = "Stopped"; doing = sayHalt(last.why); }
    else { state = "Down"; doing = "It ended without a stop line."; }
    if (lastTurn) doing += ` Last turn was ${num(lastTurn.n)}.`;
  } else if (open && open.class === "blocked") {
    state = "Stuck"; doing = String(open.why ?? "The model refused its key.");
  } else if (lastTurn && lastTurn.status === "bounded") {
    state = "Stuck"; doing = `Turn ${num(lastTurn.n)} ran past its time bound.`;
  } else if (project && sinceProgress >= THRESHOLDS.stuckTurnsWithoutProgress) {
    state = "Stuck"; doing = `No progress on “${project.title}” for ${num(sinceProgress)} turns.`;
  } else if (open) {
    state = "Waiting";
    const until = typeof open.until === "number" ? new Date(open.until) : null;
    doing = `${String(open.why ?? "Waiting")}${until ? `, until ${until.toISOString().slice(11, 16)} UTC` : ""}.`;
  } else if (lastTurn?.kind === "responding") {
    state = "Answering";
    const a = lastTurn.asked[0];
    doing = a ? `Answering: ${firstLine(a.text)}` : "Answering a waiting item.";
  } else if (project) {
    state = "Working"; doing = project.title;
  } else {
    state = "Free time";
    doing = idle ? "Idle: its last three free turns made no tool call." : lastTurn ? `Nothing pulls it. ${turnLine(lastTurn)}` : "Nothing pulls it.";
  }

  return {
    state, doing, needsYou: state === "Stuck" || state === "Down",
    turns: turns.length, lastTurn: lastTurn?.n ?? null, lastAt,
    requests: day.requests, quota, requestsDay: refDay, spendDayUsd: day.costUsd + day.deskUsd,
    project, idle,
    next: lockHeld && upcoming ? { text: String(upcoming.text ?? upcoming.id), at: upcoming.when.at } : null,
    waiting: accepted.size,
    lastDecision: decisions.findLast((d) => !d.trivial) ?? decisions.at(-1) ?? null,
    context: pack.contextPct, cache: pack.cachePct,
  };
}

