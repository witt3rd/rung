/** One followed instance: its whole record (loaded once through the read door), kept current by the events client, plus what the
 *  running turn is doing right now (deltas: short-lived, never part of the record). Plain subscribe/snapshot so React reads it
 *  with useSyncExternalStore and a test reads it directly. Changes are batched so a burst of lines is one render. */
import type { Line } from "../record/types.ts";
import { parseRecordLines, redact } from "./wire.ts";
import { EventsClient, type Delta, type Status } from "./client.ts";

/** What `GET /v1/summary` answers (ui/contract/README.md). */
export interface HostSummary {
  contract: number;
  state: string;
  doing: string;
  needs_you: boolean;
  turn: number | null;
  last_seq: number;
  last_at: number;
  requests: number;
  quota: number | null;
  spend_usd_day: number;
}

export interface LiveState {
  loaded: boolean;
  error: string | null;
  status: Status;
  lines: Line[];
  host: HostSummary | null;
}

/** What the running turn is doing now. Gone when the turn's end is on the record, and on a cut (pieces were lost). */
export interface LiveDelta {
  turn: number;
  text: string;
  tool: string | null;
}

export class LiveStore {
  private state: LiveState = { loaded: false, error: null, status: "connecting", lines: [], host: null };
  private delta: LiveDelta | null = null;
  private subs = new Set<() => void>();
  private deltaSubs = new Set<() => void>();
  private pending: Line[] = [];
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private summaryTimer: ReturnType<typeof setTimeout> | null = null;
  private client: EventsClient | null = null;
  private stopped = false;
  private base: string;
  private fetchImpl: typeof fetch;

  constructor(base: string, fetchImpl: typeof fetch = (...a) => fetch(...a)) {
    this.base = base.replace(/\/$/, "");
    this.fetchImpl = fetchImpl;
  }

  subscribe = (cb: () => void) => { this.subs.add(cb); return () => { this.subs.delete(cb); }; };
  subscribeDelta = (cb: () => void) => { this.deltaSubs.add(cb); return () => { this.deltaSubs.delete(cb); }; };
  getState = (): LiveState => this.state;
  getDelta = (): LiveDelta | null => this.delta;

  private set(patch: Partial<LiveState>) { this.state = { ...this.state, ...patch }; for (const s of this.subs) s(); }
  private setDelta(d: LiveDelta | null) { this.delta = d; for (const s of this.deltaSubs) s(); }

  async start(): Promise<void> {
    this.stopped = false;
    try {
      const lines = await this.loadRecord();
      if (this.stopped) return;
      this.set({ loaded: true, error: null, lines });
      void this.refreshSummary();
      this.follow(lines.at(-1)?.seq ?? 0);
    } catch (e) {
      if (!this.stopped) this.set({ loaded: true, error: (e as Error).message });
    }
  }

  stop(): void {
    this.stopped = true;
    this.client?.stop();
    if (this.flushTimer) clearTimeout(this.flushTimer);
    if (this.summaryTimer) clearTimeout(this.summaryTimer);
  }

  /** The whole record in one answer: the read door has no cap on `limit`. */
  private async loadRecord(): Promise<Line[]> {
    const r = await this.fetchImpl(`${this.base}/v1/record?offset=0&limit=100000000`);
    if (!r.ok) throw new Error(`the record could not be read (${r.status})`);
    return parseRecordLines((await r.json()) as { lines: Line[] });
  }

  private follow(after: number) {
    this.client?.stop();
    this.client = new EventsClient({
      url: (a) => `${this.base}/v1/events?after=${a}`,
      after,
      fetchImpl: this.fetchImpl,
      onRecord: (l) => this.onRecord(l),
      onDelta: (d) => this.onDelta(d),
      onStatus: (s) => {
        if (s === "reconnecting") this.setDelta(null); // pieces may have been lost in the cut: show none rather than a torn text
        this.set({ status: s });
      },
      onReset: () => {
        this.client = null;
        this.pending = [];
        if (this.flushTimer) { clearTimeout(this.flushTimer); this.flushTimer = null; }
        this.set({ loaded: false });
        void this.start();
      },
    });
    this.client.start();
  }

  private onRecord(l: Line) {
    l = redact(l) as Line;
    this.pending.push(l);
    if (l.kind === "turn.ended" && this.delta?.turn === l.turn) this.setDelta(null);
    if (!this.flushTimer) this.flushTimer = setTimeout(() => this.flush(), 20);
  }

  private flush() {
    this.flushTimer = null;
    if (!this.pending.length) return;
    const lines = this.state.lines.concat(this.pending);
    this.pending = [];
    this.set({ lines });
    // The host's own summary is the authority for the state word; ask again soon after the record moves.
    if (!this.summaryTimer) this.summaryTimer = setTimeout(() => { this.summaryTimer = null; void this.refreshSummary(); }, 250);
  }

  private async refreshSummary() {
    try {
      const r = await this.fetchImpl(`${this.base}/v1/summary`);
      if (r.ok && !this.stopped) this.set({ host: (await r.json()) as HostSummary });
    } catch { /* the stream shows the trouble; the summary will be asked for again with the next line */ }
  }

  private onDelta(d: Delta) {
    const cur = this.delta && this.delta.turn === d.turn ? this.delta : { turn: d.turn, text: "", tool: null };
    if (d.kind === "text") this.setDelta({ ...cur, text: redact(cur.text + (d.text ?? "")) as string });
    else this.setDelta({ ...cur, tool: d.phase === "start" && d.name ? (redact(d.name) as string) : null });
  }
}
