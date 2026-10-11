/** Logged messages as readable text, whole: the shape a record's `turn.log` and the context door both give them in. */
import type { Line } from "../record/types.ts";
import type { BandItem } from "./types.ts";

const blocks = (content: unknown): Line[] => (Array.isArray(content) ? (content as Line[]) : []);
const asText = (v: unknown): string => (typeof v === "string" ? v : JSON.stringify(v, null, 2));

/** One message as text, whole. */
export function say(m: Line): string {
  if (typeof m.content === "string") return m.content;
  return blocks(m.content).map((b) => {
    if (b.type === "text") return String(b.text ?? "");
    if (b.type === "tool_use") return `call ${String(b.name)} ${asText(b.input)}`;
    if (b.type === "tool_result") return `result${b.is_error ? " (failed)" : ""}: ${asText(b.content)}`;
    return asText(b);
  }).join("\n");
}

export const labelOf = (m: Line): string =>
  m.role === "assistant" ? "Model" : blocks(m.content).some((b) => b.type === "tool_result") ? "Tool result" : "Message";

export const item = (m: Line): BandItem => ({ label: labelOf(m), text: say(m), note: null });
