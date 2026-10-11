/** Fetch one call's strata: the host's context door first; if it is not there, or fails, or answers what cannot be read, the
 *  record's own strata with the reason in plain words. The page is never left without an answer, and never shows a guess as a fact. */
import type { Line } from "../record/types.ts";
import { foldContext } from "./fold.ts";
import type { ContextView } from "./types.ts";
import { parseContext } from "./wire.ts";

export type Mode = "live" | "static";

/** Live: the host's door through the gateway. Recorded: a door answer saved beside the record (the fixture shape). */
export function contextUrl(mode: Mode, id: string, turn: number, call: number): string {
  return mode === "live" ? `api/i/${encodeURIComponent(id)}/v1/turns/${turn}/context?call=${call}` : `data/${encodeURIComponent(id)}/context/${turn}-${call}.json`;
}

/** What a past turn's context depends on: the turn, the call, the mode, and whether the turn's log is written yet (until it is, the door
 *  answers 409). Not the length of the record: a running host writes a line every moment, and a key that moved with them would ask the door again
 *  for the same answer, and draw it again, which closes the band the reader has open. */
export const contextQueryKey = (id: string, turn: number, call: number, mode: Mode, lines: readonly Line[]) =>
  ["context", id, turn, call, mode, lines.some((l) => l.kind === "turn.log" && l.turn === turn)] as const;

export interface LoadArgs { mode: Mode; id: string; turn: number; call: number; lines: readonly Line[]; fetchImpl?: typeof fetch }

export async function loadContext(a: LoadArgs): Promise<ContextView | null> {
  const f = a.fetchImpl ?? ((...x: Parameters<typeof fetch>) => fetch(...x));
  const fromRecord = (note: string): ContextView | null => {
    const v = foldContext(a.lines, a.turn, a.call);
    return v ? { ...v, doorNote: note } : null;
  };
  let note: string;
  try {
    const r = await f(contextUrl(a.mode, a.id, a.turn, a.call));
    const text = await r.text();
    let body: unknown;
    try { body = JSON.parse(text); } catch { body = undefined; }
    // The door refuses in the error shape ({error, message}); its own words say what is wrong with the turn or the call.
    const e = body as { error?: unknown; message?: unknown } | undefined;
    const said = typeof e?.error === "string" && typeof e?.message === "string" ? e.message : null;
    const NOT_THE_ROUTE = new Set(["not_found", "no_such_route"]); // a gateway without the door says so in its own way
    if (r.status === 404 && (!said || NOT_THE_ROUTE.has(e!.error as string))) {
      note = a.mode === "live" ? "The host does not serve its context door, so this is what its record holds." : "No saved context for this call, so this is what the record holds.";
    } else if (!r.ok) {
      note = said ? `The host's context door answered ${r.status}: ${said}, so this is what its record holds.` : `The host's context door answered ${r.status}, so this is what its record holds.`;
    } else {
      try {
        if (body === undefined) throw new Error("it is not JSON");
        const v = parseContext(body);
        if (v.turn !== a.turn || v.call !== a.call) note = `The host's context door answered for another call (turn ${v.turn}, call ${v.call}), so this is what its record holds.`;
        else return v;
      } catch (err) {
        note = `The host's context answer could not be read (${(err as Error).message}), so this is what its record holds.`;
      }
    }
  } catch {
    note = "The host's context door did not answer, so this is what its record holds.";
  }
  return fromRecord(note);
}
