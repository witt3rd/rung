import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { href } from "../route.ts";
import { num } from "../record/words.ts";
import { useMode } from "../data/hooks.ts";
import { QueueUnavailable, draftFor, keepDraft, senderFor, useQueue } from "../queue/hooks.ts";
import { afterSend, foldText, groupWaiting, intentWord, isBlank, roleWord, type Intent, type QueueItem, type Row } from "../queue/wire.ts";
import { refusalWords } from "../queue/send.ts";
import { when } from "./format.ts";

/** Only "Do next" is served by the host today; the other two are shown, not hidden, and say why. */
const INTENTS: { v: Intent; label: string; works: boolean }[] = [
  { v: "next", label: "Do next", works: true },
  { v: "free", label: "When free", works: false },
  { v: "steer", label: "Steer", works: false },
];

/** A text whole: a long one shows its first part and keeps the rest under "Show more". Nothing is cut and nothing is added. */
function Folded({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const f = foldText(text);
  return (
    <>
      <p className="content" data-content>{open ? text : f.head}</p>
      {f.rest && <button type="button" className="quiet qmore" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Show less" : "Show more"}</button>}
    </>
  );
}

function ItemRow({ id, row, now, answering }: { id: string; row: Row | { pos: null; item: QueueItem }; now: number; answering?: boolean }) {
  const it = row.item;
  const side = [answering ? "Answering" : intentWord(it.intent), roleWord(it), it.at === null ? "" : when(it.at, now)].filter(Boolean).join(" · ");
  return (
    <div className="row q">
      <span className="pos meta" data-content>{row.pos ?? ""}</span>
      <div>
        <Folded text={it.text} />
        {it.held && <p className="meta qheld" data-content>{it.held}</p>}
      </div>
      <span className="meta qside" data-content>{side}{answering && it.turn !== null && <>{" · "}<a href={href.turns(id, it.turn)}>Turn {num(it.turn)}</a></>}</span>
    </div>
  );
}

/** The page's write controls, and the host's queue under them. Everything else on the page only reads. */
export function QueuePanel({ id, running, state, now, lastSeq }: { id: string; running: boolean; state: string | null; now: number; lastSeq: number | null }) {
  const mode = useMode().data;
  const live = mode === "live";
  const qc = useQueryClient();
  const q = useQueue(id, live, lastSeq);
  const sender = senderFor(id);
  const [text, setText] = useState(() => draftFor(id));
  const intent: Intent = "next";
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<{ words: string; bad: boolean } | null>(null);
  useEffect(() => { setStatus(null); setText(draftFor(id)); }, [id]);

  if (mode === undefined) return <section className="sec"><h2>Queue</h2><p className="none meta">&nbsp;</p></section>;
  if (!live) {
    return (
      <section className="sec">
        <h2>Queue</h2>
        <p className="meta none" data-content>This is a recorded run. It has no queue to add to.</p>
      </section>
    );
  }

  const stopped = !running && state !== null; // the host says it is not running: the door would refuse
  async function submit(e: { preventDefault(): void }) {
    e.preventDefault();
    if (busy || !running || noDoor || isBlank(text)) return;
    const sent = text;
    setBusy(true);
    setStatus({ words: "Sending.", bad: false });
    const r = await sender.send(sent, intent);
    setBusy(false);
    // The box is cleared only when the host wrote it down, and then only if nothing was typed meanwhile; on any refusal the text stays.
    setText((cur) => { const n = afterSend(cur, sent, r.ok); keepDraft(id, n); return n; });
    if (r.ok) {
      // The host has written it down; its list is asked again, and the item shows when the host's list has it.
      await qc.invalidateQueries({ queryKey: ["queue", id] });
      setStatus({ words: r.repeat ? "It was already there; it is added once." : "Sent. The host has written it down.", bad: false });
    } else setStatus({ words: r.message, bad: r.kind !== "empty" && r.kind !== "key_reused" });
  }

  const ans = q.data;
  const noDoor = q.error instanceof QueueUnavailable && q.error.status === 404; // the host has no queue door: a press could only be refused
  const g = ans ? groupWaiting(ans.waiting) : null;
  const unavailable = q.isError;
  return (
    <section className="sec">
      <h2>Queue</h2>
      <form className="qbox" onSubmit={submit}>
        <textarea
          aria-label="Message for the agent" placeholder="What should it do, or know?" rows={3} value={text} readOnly={busy}
          onChange={(e) => { if (busy) return; setText(e.target.value); keepDraft(id, e.target.value); if (status) setStatus(null); }}
        />
        <div className="qctl">
          <div className="seg" role="radiogroup" aria-label="When it reads this">
            {INTENTS.map((i) => (
              <label key={i.v} className={intent === i.v ? "on" : i.works ? undefined : "off"}>
                <input type="radio" name={`intent-${id}`} value={i.v} checked={intent === i.v} disabled={!i.works} onChange={() => {}} />
                {i.label}
              </label>
            ))}
          </div>
          <button type="submit" className="btn primary" disabled={busy || !running || noDoor || isBlank(text)}>Send</button>
        </div>
        <p className="meta qnote">Steer and When free come later.</p>
        <p className={`meta qstatus${status?.bad ? " fail-text" : ""}`} role="status" aria-live="polite">
          {stopped ? refusalWords("not_running", "") : noDoor ? refusalWords("not_there", "") : status ? status.words : "\u00a0"}
        </p>
      </form>

      <h3>Answering</h3>
      {ans ? (ans.now.length ? ans.now.map((i) => <ItemRow key={i.id} id={id} row={{ pos: null, item: i }} now={now} answering />) : <p className="none meta">None.</p>) : <p className="none meta">&nbsp;</p>}

      <h3>Waiting</h3>
      {g && (g.owner.length > 0 || g.others.length > 0)
        ? <p className="meta qrule">Peer items can never be moved above the owner's.</p>
        : <p className="meta qrule">{g ? "None." : "\u00a0"}</p>}
      {unavailable && <p className="meta none" data-content>{noDoor ? "No queue door yet." : "The queue could not be read."}</p>}
      {!unavailable && !ans && <p className="none meta">&nbsp;</p>}
      {g && (g.owner.length > 0 || g.others.length > 0) && (
        <>
          <p className="sub meta">Owner items</p>
          {g.owner.length ? g.owner.map((r) => <ItemRow key={r.item.id} id={id} row={r} now={now} />) : <p className="none meta">None.</p>}
          <p className="sub meta">From others</p>
          {g.others.length ? g.others.map((r) => <ItemRow key={r.item.id} id={id} row={r} now={now} />) : <p className="none meta">None.</p>}
        </>
      )}
    </section>
  );
}
