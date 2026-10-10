import type { Line } from "../record/types.ts";
import { makeRedactor } from "../record/redact.ts";

// A line read from a host passes the same redactor as a recorded file does: a page never shows a key shape whatever served it.
export const redact = makeRedactor([]);

/** The lines of a `/v1/record` answer, in order, after one check the contract promises: numbers without gaps. */
export function parseRecordLines(answer: { lines: Line[] }): Line[] {
  const lines = answer.lines.map((l) => redact(l) as Line);
  for (let i = 1; i < lines.length; i++) {
    if (lines[i].seq !== lines[i - 1].seq + 1) throw new Error(`the record has a gap between line ${lines[i - 1].seq} and ${lines[i].seq}`);
  }
  return lines;
}
