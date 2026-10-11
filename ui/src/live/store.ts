/** One followed instance: its whole record (loaded once through the read door), kept current by the events client, plus what the
 *  running turn is doing right now (deltas: short-lived, never part of the record). Plain subscribe/snapshot so React reads it
 *  with useSyncExternalStore and a test reads it directly. Changes are batched so a burst of lines is one render. */
import type { Line } from "../record/types.ts";
import { parseRecordLines, redact, redactStreaming } from "./wire.ts";
import { EventsClient, type Delta, type Open, type Status } from "./client.ts";

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
  /** The command that starts it, when it is a native instance with a config file. The page shows it; nothing here starts a host. */
  start_command?: string | null;
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
  /** The streamed text exactly as it came. It is redacted when read, over the whole text so far: a key split across pieces
   *  is whole in here, and redacting the running text would leave the tail of a key the next piece completes. */
  private raw: LiveDelta | null = null;
  private delta: LiveDelta | null = null;
  /** The turn whose deltas are not shown: it was already running when we joined or when the stream came back, so some of its
   *  pieces were never seen and what we would show is a torn tail. Its finished text arrives in the record. */
  private tornTurn: number | null = null;
  /** Has the stream replayed what existed when it opened? Partial frames are shown only after it has: until then the page cannot tell
   *  which turn was already running, and a turn already running was not seen from its start. */
  private caught = false;
  /** The number of the last line the record held when the stream opened (asked of the summary), or null while that is not known. */
  private joinSeq: number | null = null;
  /** The next piece of answer text begins a new text block: it is set apart from what came before. */
  private newBlock = false;
  private followWhenFlushed = false;
  /** Which opening of the stream is the current one: an answer to the summary asked for an older one is let go. */
  private openGen = 0;
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
  private setDelta(d: LiveDelta | null) {
    this.raw = d;
    this.delta = d ? { turn: d.turn, text: redactStreaming(d.text), tool: d.tool === null ? null : (redact(d.tool) as string) } : null;
    for (const s of this.deltaSubs) s();
  }

  /** The turn that has started and not ended, from the lines held and those queued. */
  private runningTurn(): number | null {
    let running: number | null = null;
    for (const l of [...this.state.lines, ...this.pending]) {
      if (l.kind === "turn.started") running = l.turn;
      else if (l.kind === "turn.ended" && l.turn === running) running = null;
    }
    return running;
  }

  async start(): Promise<void> {
    this.stopped = false;
    try {
      const lines = await this.loadRecord();
      if (this.stopped) return;
      this.tornTurn = null;
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
      onOpen: (o) => void this.onOpen(o),
      onEnd: () => void this.refreshSummary(),
      onStatus: (s) => {
        if (s === "reconnecting" || s === "connecting") { this.caught = false; this.setDelta(null); } // pieces may have been lost in the cut: show none rather than a torn text
        // "following" is the store's to say: it is true once the replay has been applied (checkCaught).
        this.set({ status: s });
      },
      onReset: () => {
        this.client?.stop(); // a client that asked for a reset is done; one still looping would follow a record we are about to replace
        this.client = null;
        this.pending = [];
        this.caught = false;
        this.followWhenFlushed = false;
        this.setDelta(null); // a turn the new record may not hold: its pieces are stale
        if (this.flushTimer) { clearTimeout(this.flushTimer); this.flushTimer = null; }
        this.set({ loaded: false });
        void this.start();
      },
    });
    this.client.start();
  }

  /** The stream opened. Ask where the record stood, so that the replay's end is known: until the lines to that number are applied no
   *  partial frame is shown, and the turn running then (started before we were connected) is never shown streaming. */
  private async onOpen(o: Open) {
    const gen = ++this.openGen;
    this.caught = false;
    this.tornTurn = null;
    this.joinSeq = null;
    this.newBlock = false;
    this.setDelta(null);
    if (!o.live) return; // a stopped host: its stream ends right after the replay
    // Without the summary the page cannot say where the replay ends, so it would show no partial frame at all: ask again, with a growing
    // wait, for as long as this stream is the open one. The record is followed meanwhile.
    for (let wait = 250; !this.stopped && gen === this.openGen && this.joinSeq === null; wait = Math.min(wait * 2, 4000)) {
      try {
        const r = await this.fetchImpl(`${this.base}/v1/summary`);
        if (r.ok && !this.stopped && gen === this.openGen) {
          const host = redact(await r.json()) as HostSummary;
          this.set({ host });
          this.joinSeq = host.last_seq;
          this.checkCaught();
          return;
        }
      } catch { /* try again */ }
      await new Promise((res) => setTimeout(res, wait));
    }
  }

  private checkCaught() {
    if (this.caught || this.joinSeq === null || !this.client || this.client.lastSeq < this.joinSeq) return;
    this.caught = true;
    this.tornTurn = this.runningTurn();
    // "Following" is said once the replayed lines are in the state the page shows, not when they have only been received.
    if (this.pending.length) this.followWhenFlushed = true;
    else if (this.state.status === "catching up") this.set({ status: "following" });
  }

  private onRecord(l: Line) {
    l = redact(l) as Line;
    this.pending.push(l);
    if (l.kind === "turn.ended" && this.raw?.turn === l.turn) this.setDelta(null);
    if (!this.flushTimer) this.flushTimer = setTimeout(() => this.flush(), 20);
    this.checkCaught();
  }

  private flush() {
    this.flushTimer = null;
    if (!this.pending.length) return;
    const lines = this.state.lines.concat(this.pending);
    this.pending = [];
    this.set({ lines });
    if (this.followWhenFlushed) { this.followWhenFlushed = false; if (this.state.status === "catching up") this.set({ status: "following" }); }
    // The host's own summary is the authority for the state word; ask again soon after the record moves.
    if (!this.summaryTimer) this.summaryTimer = setTimeout(() => { this.summaryTimer = null; void this.refreshSummary(); }, 250);
  }

  private async refreshSummary() {
    try {
      const r = await this.fetchImpl(`${this.base}/v1/summary`);
      if (r.ok && !this.stopped) this.set({ host: redact(await r.json()) as HostSummary });
    } catch { /* the stream shows the trouble; the summary will be asked for again with the next line */ }
  }

  private onDelta(d: Delta) {
    if (!this.caught) return;
    if (d.kind === "behind") {
      // This reader missed partial frames and the host cut every block that was open: what follows is not the turn's whole text.
      this.tornTurn = this.raw?.turn ?? this.runningTurn();
      this.setDelta(null);
      return;
    }
    if (d.turn === this.tornTurn) return;
    const cur = this.raw && this.raw.turn === d.turn ? this.raw : { turn: d.turn, text: "", tool: null };
    switch (d.kind) {
      case "block_start":
        if (d.id === null && cur.text) this.newBlock = true;
        if (cur !== this.raw) this.setDelta(cur);
        break;
      case "text":
        this.setDelta({ ...cur, text: cur.text + (this.newBlock ? "\n\n" : "") + d.text });
        this.newBlock = false;
        break;
      case "tool_call":
      case "tool_run":
        this.setDelta({ ...cur, tool: d.name });
        break;
      case "tool_done":
        this.setDelta({ ...cur, tool: null });
        break;
      default: // tool_input, block_end, turn_end: nothing to show; the record has the whole of it
    }
  }
}
