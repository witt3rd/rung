/** The events client: follows a host's record over server-sent events and hands each line over exactly once, in order,
 *  across any number of cuts. It keeps the number of the last line it applied, resumes from it, ignores anything at or below
 *  it, and treats a skipped number as a broken stream (reconnect, resume) rather than a line to apply. Deltas are passed on
 *  as they come and are dropped by the host on a cut: nothing here replays them. */
import type { Line } from "../record/types.ts";
import { SseParser } from "./sse.ts";

export type Status = "connecting" | "catching up" | "following" | "reconnecting";

export interface Delta {
  turn: number;
  kind: "text" | "tool";
  text?: string;
  name?: string;
  phase?: "start" | "end";
}

export interface EventsOptions {
  /** The stream's URL for lines after `after`. */
  url: (after: number) => string;
  /** The last line number already held. */
  after: number;
  onRecord: (line: Line) => void;
  onDelta?: (d: Delta) => void;
  onStatus?: (s: Status) => void;
  /** The host does not know the number we hold (400): the record must be loaded again. */
  onReset?: () => void;
  fetchImpl?: typeof fetch;
  /** First wait before reconnecting; it doubles to `maxBackoffMs` and resets once the stream has caught up. */
  backoffMs?: number;
  maxBackoffMs?: number;
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
    while (!this.stopped) {
      this.o.onStatus?.(first ? "connecting" : "reconnecting");
      first = false;
      const ac = (this.ac = new AbortController());
      let caught = false;
      try {
        const r = await f(this.o.url(this.last), { headers: { accept: "text/event-stream", "last-event-id": String(this.last) }, signal: ac.signal });
        if (r.status === 400) { this.o.onReset?.(); return; }
        if (r.status !== 200 || !r.body) throw new Error(`events: status ${r.status}`);
        const p = new SseParser();
        const dec = new TextDecoder();
        this.o.onStatus?.("catching up");
        const reader = r.body.getReader();
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          for (const e of p.push(dec.decode(value, { stream: true }))) {
            if (e.event === "record") {
              const seq = Number(e.id);
              if (!Number.isInteger(seq) || seq <= this.last) continue; // seen: a resume overlaps, never repeats
              if (seq !== this.last + 1) throw new Error(`events: line ${seq} after ${this.last}`); // a gap: resume
              this.last = seq;
              this.o.onRecord(JSON.parse(e.data) as Line);
            } else if (e.event === "delta") {
              this.o.onDelta?.(JSON.parse(e.data) as Delta);
            } else if (e.event === "caught_up") {
              caught = true; wait = this.o.backoffMs ?? 500;
              this.o.onStatus?.("following");
            }
          }
        }
      } catch {
        // a cut, a refusal, a gap: fall through to the wait and resume
      }
      if (this.stopped) return;
      this.o.onStatus?.("reconnecting");
      await new Promise((r) => setTimeout(r, wait));
      if (!caught) wait = Math.min(max, wait * 2);
    }
  }
}
