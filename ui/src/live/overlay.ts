/** The running turn as the owner should see it: the record has its start and nothing more, so the streamed text and the tool
 *  in flight are laid over it. Pure; the page hands it the folded turns and the store's delta. */
import type { Turn } from "../record/folds.ts";
import type { LiveDelta } from "./store.ts";

export function overlayRunning(turns: Turn[], d: LiveDelta | null): Turn[] {
  if (!d) return turns;
  const i = turns.findIndex((t) => t.n === d.turn && t.status === "running");
  if (i < 0) return turns;
  const t = turns[i];
  const calls = d.tool ? [...t.calls, { name: d.tool, group: null, ok: null, refused: null, input: undefined, result: null }] : t.calls;
  const out = turns.slice();
  out[i] = { ...t, text: d.text || t.text, calls };
  return out;
}
