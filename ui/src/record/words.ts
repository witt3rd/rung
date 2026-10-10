/** Plain words for what the record says. Every sentence the page shows about the host comes from here or
 *  from a fold, so the wording can be tested in one place. No ids or hashes reach the main view. */
import type { Line } from "./types.ts";

export type StateWord = "Working" | "Answering" | "Free time" | "Waiting" | "Stuck" | "Down" | "Stopped";

/** The question each decision family settles, as the owner would ask it. */
export const FAMILY_QUESTION: Record<string, string> = {
  admit: "Which waiting items to take now, and whether to interrupt",
  inject: "What to put in front of the agent this turn",
  tools: "Which groups of tools to turn on",
  pack: "Whether to keep the context or start a new one",
  consolidate: "What to keep in memory and whether to write a note",
};

export function familyQuestion(family: string): string {
  return FAMILY_QUESTION[family] ?? `A decision of kind ${family}`;
}

const list = (xs: unknown): string[] => (Array.isArray(xs) ? xs.map(String) : []);
const sayList = (xs: string[], none: string) => (xs.length ? xs.join(", ") : none);

/** The answer of a decision, in words. */
export function sayChoice(family: string, choice: Record<string, unknown> | undefined): string {
  const c = choice ?? {};
  switch (family) {
    case "admit": {
      const forms = Object.keys((c.forms as object) ?? {});
      const n = Number(c.interrupts ?? 0);
      if (!forms.length) return "Nothing waiting to take.";
      return `Took ${forms.length} waiting ${forms.length === 1 ? "item" : "items"}${n ? `, interrupting ${n}` : ""}.`;
    }
    case "inject": {
      const parts = [c.calendar ? "the calendar" : null, c.expectations ? "its expectations" : null, c.recall ? "a memory recall" : null].filter(Boolean);
      return parts.length ? `Showed ${parts.join(", ")}.` : "Showed nothing extra.";
    }
    case "tools":
      return `Turned on ${sayList(list(c.enabled), "no tool groups")}.`;
    case "pack": {
      const action = String(c.action ?? "");
      const cause = String(c.cause ?? "").replace(/_/g, " ");
      if (action === "append") return "Kept the same context.";
      if (action.startsWith("rollover")) return `Started a new context${cause ? ` (${cause})` : ""}.`;
      return `Chose ${action || "nothing"}.`;
    }
    case "consolidate": {
      const considered = list(c.considered).length;
      const kept = list(c.retain).length;
      return `Looked at ${considered} ${considered === 1 ? "memory" : "memories"}, kept ${kept}${c.note_line ? ", wrote a note line" : ""}.`;
    }
    default:
      return JSON.stringify(c);
  }
}

/** Who decided, and why a rule stepped in. `by` is `{rule: why}` or `{jev: {…}}` in the record. */
export function sayWho(by: Line["by"]): { who: "rule" | "model" | "unknown"; label: string; reason: string | null } {
  if (by && typeof by === "object" && "rule" in by) {
    const why = String(by.rule);
    const reasons: Record<string, string> = {
      nothing_to_ask: "There was nothing to ask.",
      shadow: "The model's answer was recorded for comparison only; the rule decided.",
      timeout: "The model did not answer in time, so the rule decided.",
      cap: "The day's spend cap was reached, so the rule decided.",
      kill: "The model was switched off, so the rule decided.",
    };
    return { who: "rule", label: "By rule", reason: reasons[why] ?? `The rule decided: ${why.replace(/_/g, " ")}.` };
  }
  if (by && typeof by === "object") {
    const [name, info] = Object.entries(by)[0] ?? ["model", {}];
    const model = (info as { model?: string })?.model;
    return { who: "model", label: `By model ${model ?? name}`, reason: "The model decided." };
  }
  return { who: "unknown", label: "Unknown", reason: null };
}

/** A failure class, in words. */
export function sayFailure(f: { class?: string; origin?: string } | null | undefined): string {
  if (!f || !f.class) return "failed";
  const cls = f.class.replace(/_/g, " ");
  return f.origin ? `failed: ${f.origin} ${cls}` : `failed: ${cls}`;
}

export function sayHalt(why: { by?: string; why?: string } | undefined): string {
  const by = why?.by ?? "";
  if (by === "limit") return "Reached its run limit.";
  if (by === "owner") return "Its owner told it to stop.";
  if (by === "signal") return "It received a stop signal.";
  return by ? `It halted (${by}).` : "It halted.";
}

/** "3 min ago", "2 d ago": the largest whole unit. */
export function ago(thenMs: number, nowMs: number): string {
  const s = Math.max(0, Math.round((nowMs - thenMs) / 1000));
  if (s < 5) return "just now";
  if (s < 60) return `${s} s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}

/** "1,000": thousands separators fixed to one locale so tests and screens agree. */
export const num = (n: number): string => n.toLocaleString("en-US");

export const usd = (n: number): string => (n === 0 ? "$0.00" : n < 0.01 ? `$${n.toFixed(4)}` : `$${n.toFixed(2)}`);

export function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${s % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

/** The first non-empty line, with leading markdown marks removed: a summary, never a cut of the text. */
export function firstLine(text: string): string {
  for (const raw of text.split("\n")) {
    const t = raw.replace(/^[\s#>*\-•]+/, "").replace(/\*\*/g, "").trim();
    if (t) return t;
  }
  return "";
}
