/** Sending one item to the host's queue door. One press is one write: a second press of the same message while the first is in
 *  flight is the same request, and a press repeated after an answer was lost carries the same key, so the host adds it once.
 *  The page shows an item in the list only when the host's list has it; nothing here touches the list. */
import { redact } from "../live/wire.ts";
import { isBlank, type Intent } from "./wire.ts";

export type RefusalKind = "not_running" | "key_reused" | "read_only" | "not_there" | "refused" | "unreachable" | "empty";
export type SendResult = { ok: true; id: string; repeat: boolean } | { ok: false; kind: RefusalKind; message: string };

/** The page's own words for each way a send ends, and the host's reason where it gave one. */
export function refusalWords(kind: RefusalKind, hostSaid: string): string {
  switch (kind) {
    case "not_running": return "Start the instance first.";
    case "key_reused": return "That press was already added. Nothing new was written.";
    case "read_only": return "This view can only read, so it cannot send.";
    case "not_there": return "This instance cannot take messages from the page yet.";
    case "unreachable": return hostSaid || "It did not answer. Press Send again: the message is added once.";
    case "empty": return "Write something first.";
    case "refused": return hostSaid || "The host refused it.";
  }
}

interface Press { text: string; intent: Intent; key: string; inflight: Promise<SendResult> | null }

export interface SenderOptions {
  /** The queue door's URL (through the gateway). */
  url: string;
  fetchImpl?: typeof fetch;
  newKey?: () => string;
}

export class Sender {
  private press: Press | null = null;
  private o: SenderOptions;
  constructor(o: SenderOptions) { this.o = o; }

  /** No press waits for an answer or may still be repeated: nothing here needs to outlive the page. */
  get idle(): boolean { return this.press === null; }

  send(text: string, intent: Intent): Promise<SendResult> {
    if (isBlank(text)) return Promise.resolve({ ok: false, kind: "empty", message: refusalWords("empty", "") });
    let p = this.press;
    if (p && p.text === text && p.intent === intent) {
      if (p.inflight) return p.inflight; // the same press again: the same request
    } else {
      p = this.press = { text, intent, key: (this.o.newKey ?? (() => crypto.randomUUID()))(), inflight: null };
    }
    const press = p;
    press.inflight = this.run(press).then((r) => {
      press.inflight = null;
      // Written, or definitely not: the next press is a new one. After an answer that was lost the press stays, so its key is reused.
      if (r.ok || r.kind !== "unreachable") { if (this.press === press) this.press = null; }
      return r;
    });
    return press.inflight;
  }

  private async run(p: Press): Promise<SendResult> {
    const f = this.o.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
    let r: Response;
    try {
      r = await f(this.o.url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ key: p.key, text: p.text, intent: p.intent }) });
    } catch {
      return { ok: false, kind: "unreachable", message: refusalWords("unreachable", "") };
    }
    let body: { id?: unknown; repeat?: unknown; error?: unknown; message?: unknown } = {};
    try { body = (await r.json()) ?? {}; } catch { /* not JSON: the status decides */ }
    if (r.ok) return { ok: true, id: typeof body.id === "string" ? body.id : p.key, repeat: body.repeat === true };
    const said = typeof body.message === "string" ? (redact(body.message) as string) : "";
    // 502 and 503 from the instance's side leave the message possibly queued: the press stays, and a repeat carries its key. A gateway that has
    // no key or no address for the instance sent nothing: that is a refusal.
    const kind: RefusalKind = r.status === 409 && body.error === "not_running" ? "not_running"
      : r.status === 409 && body.error === "key_reused" ? "key_reused"
      : r.status === 403 ? "read_only"
      : r.status === 404 ? "not_there"
      : r.status >= 500 && body.error !== "no_owner_key" && body.error !== "no_address" ? "unreachable"
      : "refused";
    return { ok: false, kind, message: refusalWords(kind, said) };
  }
}
