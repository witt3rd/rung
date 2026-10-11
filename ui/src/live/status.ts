/** The one mapping from what the stream can be doing to the words every page uses for it and what it means. `Record<Status, …>` makes it
 *  exhaustive: a status added to `STATUSES` does not compile until it has an entry here, so two pages cannot read the same status two ways. */
import { STATUSES, type End, type Status } from "./client.ts";
import type { Summary } from "../record/folds.ts";
import { isLockHeld } from "../record/words.ts";

/** Does this status mean the instance is running? `unknown` when the stream cannot say (it is not connected, or there is no stream). */
export type Running = "yes" | "no" | "unknown";

/** What the page is told about the running turn, for the words of the Now page. */
export interface TurnContext {
  /** The record's last turn has started and not ended. */
  turnRunning?: boolean;
  /** The tool in flight, if the stream said one. */
  tool?: string | null;
  /** How long the turn has run, in words. */
  elapsed?: string;
}

const ROW: Record<Status, { turnsLine: string; now: (c: TurnContext) => string; running: Running }> = {
  connecting: { turnsLine: "Reconnecting", now: () => "Reconnecting.", running: "unknown" },
  reconnecting: { turnsLine: "Reconnecting", now: () => "Reconnecting.", running: "unknown" },
  "catching up": { turnsLine: "Catching up", now: () => "Catching up.", running: "yes" },
  following: {
    turnsLine: "Following",
    now: (c) => (c.turnRunning ? `${c.tool ? `Running ${c.tool}` : "Writing"}, ${c.elapsed ?? "0 s"} in.` : "Between turns."),
    running: "yes",
  },
  // The host stopped, whatever the record's last turn says: a stale record may still hold a turn that never ended.
  ended: { turnsLine: "Not running", now: () => "Not running.", running: "no" },
  unavailable: { turnsLine: "Recorded, not followed", now: () => "Not followed.", running: "unknown" },
};

export const TURNS_LINE = Object.fromEntries(STATUSES.map((s) => [s, ROW[s].turnsLine])) as Record<Status, string>;

export function describeStatus(status: Status, ctx: TurnContext = {}): { turnsLine: string; nowWords: string; running: Running } {
  const r = ROW[status];
  return { turnsLine: r.turnsLine, nowWords: r.now(ctx), running: r.running };
}

/** The stream's last frame says the host stopped (or died): the instance is not running, whatever a summary read before it said. */
export const stoppedByStream = (ended: End | null): boolean => ended !== null && (ended.reason === "stopped" || ended.reason === "down");

/** The summary as the pages show it once the stream has ended: a summary that still holds a lock-held word is stale, and the stream's
 *  word replaces it. A summary that already says Stopped, Down or Unreadable is left as it is. */
export function withStreamEnd(summary: Summary, ended: End | null): Summary {
  if (!stoppedByStream(ended) || !isLockHeld(summary.state)) return summary;
  const stopped = ended!.reason === "stopped";
  return { ...summary, state: stopped ? "Stopped" : "Down", doing: "Not running.", needsYou: !stopped };
}
