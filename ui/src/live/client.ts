/** The events client: follows a host's record over server-sent events and hands each line over exactly once, in order,
 *  across any number of cuts. It keeps the number of the last line it applied, resumes from it, ignores anything at or below
 *  it, and treats a skipped number as a broken stream (reconnect, resume) rather than a line to apply. Deltas are passed on
 *  as they come and are dropped by the host on a cut: nothing here replays them. */
import type { Line } from "../record/types.ts";
import { SseParser } from "./sse.ts";

/** Every state the stream can be in. `describeStatus` has an entry for each: add one here and it does not compile until it has words. */
export const STATUSES = ["connecting", "catching up", "following", "reconnecting", "unavailable", "ended"] as const;
export type Status = (typeof STATUSES)[number];

/** A partial frame (docs/rung-host-api.md, H5): short-lived, never numbered, never replayed. `turn` and `call` are 0 on `behind`. */
export type Delta =
  | { kind: "text"; turn: number; call: number; text: string }
  | { kind: "tool_call"; turn: number; call: number; id: string; name: string }
  | { kind: "tool_input"; turn: number; call: number; id: string; delta: string }
  | { kind: "tool_run"; turn: number; call: number; name: string; input: unknown }
  | { kind: "tool_done"; turn: number; call: number; name: string; ok: boolean; output: string }
  | { kind: "block_start"; turn: number; call: number; id: string | null }
  | { kind: "block_end"; turn: number; call: number; id: string | null }
  | { kind: "turn_end"; turn: number; call: number }
  | { kind: "behind"; turn: number; call: number; skipped: number };

/** The first frame of a stream. `live`: the host holds its lock. `partials`: this stream will carry partial frames. */
export interface Open { contract: number; after: number; live: boolean; partials: boolean; redaction: { env_unset: string[] } }
/** The last frame of a stream whose host stopped (or whose record broke): the body ends after it. */
export interface End { reason: "stopped" | "down" | "damaged" | "error"; last_seq: number; problem: string | null }

export interface EventsOptions {
  /** The stream's URL for lines after `after`. */
  url: (after: number) => string;
  /** The last line number already held. */
  after: number;
  onRecord: (line: Line) => void;
  onDelta?: (d: Delta) => void;
  onOpen?: (o: Open) => void;
  onEnd?: (e: End) => void;
  onStatus?: (s: Status) => void;
  /** The host does not hold the cursor we have (409 cursor_ahead: it is from another instance): the record must be loaded again. */
  onReset?: () => void;
  /** The gateway has no event stream for this instance (404): the record is shown as it was read, and not followed. */
  onUnavailable?: () => void;
  fetchImpl?: typeof fetch;
  /** First wait before reconnecting; it doubles to `maxBackoffMs` and resets once a stream has opened. */
  backoffMs?: number;
  maxBackoffMs?: number;
  /** After an `end` frame (the host stopped), how long before asking again whether it is back. */
  endedRetryMs?: number;
}

export class EventsClient {
  private last: number;
  private ac: AbortController | null = null;
  private stopped = false;
  private o: EventsOptions;

  constructor(o: EventsOptions) { this.o = o; this.last = o.after; }

  get lastSeq(): number { return this.last; }

  start(): void { void this.loop(); }

  stop(): void { this.stopped = true; this.ac?.abort(); }

  /** Cut the connection from this side (a test, or a page going to the background); the loop resumes by itself. */
  cut(): void { this.ac?.abort(); }

  private async loop() {
    const f = this.o.fetchImpl ?? fetch;
    let wait = this.o.backoffMs ?? 500;
    const max = this.o.maxBackoffMs ?? 8000;
    let first = true;
    let afterEnd = false;
    while (!this.stopped) {
      if (!afterEnd) this.o.onStatus?.(first ? "connecting" : "reconnecting");
      first = false;
      const ac = (this.ac = new AbortController());
      let opened = false;
      let ended = false;
      try {
        const r = await f(this.o.url(this.last), { headers: { accept: "text/event-stream", "last-event-id": String(this.last) }, signal: ac.signal });
        if (r.status === 409) { this.o.onReset?.(); return; }
        if (r.status === 404) { this.o.onStatus?.("unavailable"); this.o.onUnavailable?.(); return; }
        if (r.status !== 200 || !r.body) throw new Error(`events: status ${r.status}`);
        const p = new SseParser();
        const dec = new TextDecoder();
        const reader = r.body.getReader();
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          for (const e of p.push(dec.decode(value, { stream: true }))) {
            if (e.event === "open") {
              opened = true; wait = this.o.backoffMs ?? 500;
              const o = JSON.parse(e.data) as Open;
              if (o.live || !afterEnd) this.o.onStatus?.("catching up");
              this.o.onOpen?.(o);
            } else if (e.event === "record") {
              const seq = Number(e.id);
              if (!Number.isInteger(seq) || seq <= this.last) continue; // seen: a resume overlaps, never repeats
              if (seq !== this.last + 1) throw new Error(`events: line ${seq} after ${this.last}`); // a gap: resume
              this.last = seq;
              this.o.onRecord(JSON.parse(e.data) as Line);
            } else if (e.event === "delta") {
              this.o.onDelta?.(JSON.parse(e.data) as Delta);
            } else if (e.event === "end") {
              ended = true;
              this.o.onStatus?.("ended");
              this.o.onEnd?.(JSON.parse(e.data) as End);
            }
          }
        }
      } catch {
        // a cut, a refusal, a gap: fall through to the wait and resume
      }
      if (this.stopped) return;
      if (ended) {
        // The host stopped: nothing to follow. Ask again now and then whether it is back; the status stays as it is.
        afterEnd = true;
        await new Promise((r) => setTimeout(r, this.o.endedRetryMs ?? 5000));
        continue;
      }
      afterEnd = false;
      this.o.onStatus?.("reconnecting");
      await new Promise((r) => setTimeout(r, wait));
      if (!opened) wait = Math.min(max, wait * 2);
    }
  }
}
