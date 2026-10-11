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

/** Streamed text is read while it is still arriving, so a key may be half written at its end. Shapes that are not yet whole are
 *  hidden too, until the rest arrives and the whole shape is replaced; nothing of a key's head or tail is ever shown. */
const UNFINISHED = /(?:\bsk-|\bgh[pousr]_|\bgithub_pat_|\bxox[abprs]-|\bAKIA|\bAIza|\bBearer\s+)[A-Za-z0-9._~+/=-]*$/;

/** Redact text that is still being written. Always called on the whole text received so far, never on a piece. */
export function redactStreaming(text: string): string {
  return (redact(text) as string).replace(UNFINISHED, "[redacted]");
}
