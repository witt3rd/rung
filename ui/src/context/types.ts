/** What was sent to the model for one model call, as stacked bands in the order the model sees them (wire order).
 *  One shape, whether the host's context door gave it (`source: "door"`) or the page rebuilt what it could from the record
 *  (`source: "record"`). The door's body is proposed in contract/README.md. */

export type BandId = "tools" | "system" | "epoch" | "history" | "recall" | "message" | "this_turn";

/** Wire order: the first band is the first thing the model reads. Two cache breakpoints sit after `system` and after `epoch`.
 *  The turn's header is one message on the wire: its message part first, its recalled memory after it. */
export const WIRE_ORDER: readonly BandId[] = ["tools", "system", "epoch", "history", "message", "recall", "this_turn"];

/** The band after which a cache boundary is drawn, and which boundary it is. */
export const BOUNDARY_AFTER: Partial<Record<BandId, 1 | 2>> = { system: 1, epoch: 2 };

/** A row inside a band: one tool definition, one past turn, one message. Its text is whole. */
export interface BandItem { label: string; text: string; /** Something to say about this row alone (for example, that part of it is not in the record). */ note: string | null }

export interface Band {
  id: BandId;
  /** False: the host did not record this band, so there is no text; `why` says why. */
  recorded: boolean;
  why: string | null;
  /** True but only in part (for example only the epoch's title line): what is missing, in words. */
  partial: string | null;
  /** The host's own estimate of the band's size in tokens (an estimate; the provider's count is the truth). */
  estimate: number | null;
  /** The cache mark the host's door gave for this band, as it gave it; null when the page has to derive it. */
  cache: "cached" | "partly" | "new" | "unknown" | null;
  /** The band as one text, or as rows (a tool definition, a past turn), whole. A recorded band with neither is empty. */
  text: string | null;
  items: BandItem[];
}

/** The provider's own counts for the call. */
export interface Usage { prompt: number; cached: number; written: number }

export interface ContextView {
  source: "door" | "record";
  turn: number;
  /** The call shown (1-based); 0 when the turn made no model call. */
  call: number;
  /** How many model calls the turn made. */
  calls: number;
  epoch: number | null;
  model: string | null;
  /** Null until the provider has answered the call. */
  usage: Usage | null;
  /** A cache break or a cold cache the host recorded at this call: its kind and cause, as the record names them (rollover, model_switch). */
  cacheEvent: { kind: string; cause: string } | null;
  /** The engine rewrote what it was given inside the turn: the bands are what the host gave it, not what the last calls saw. */
  rewritten: boolean;
  /** In words, what the record cannot give back whole for this call. */
  problems: string[];
  /** The host's own note on the answer (for example, that the turn predates context recording). */
  note: string | null;
  /** The two cache breakpoints, where the host placed them (provider-scaled token position). */
  boundaries: { after: BandId; tokens: number | null }[];
  /** The record is damaged, in the host's words. */
  damaged: string | null;
  /** Seven bands, always, in wire order. */
  bands: Band[];
  /** Why the page shows the record's strata and not the host's door, when it does. */
  doorNote: string | null;
}

export const emptyBand = (id: BandId, over: Partial<Band> = {}): Band =>
  ({ id, recorded: true, why: null, partial: null, estimate: null, cache: null, text: null, items: [], ...over });
export const missingBand = (id: BandId, why: string): Band => emptyBand(id, { recorded: false, why });
