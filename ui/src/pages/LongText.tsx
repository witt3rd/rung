import { useMemo, useState } from "react";
import { hasLongLine, isLong } from "../context/words.ts";

/** A text, whole. A long one is folded on the page: every character is in the page, the fold only keeps it to a screenful until the
 *  owner opens it, and "Show less" folds it again. A very long line scrolls sideways instead of wrapping. Nothing is cut and nothing is added. */
export function LongText({ text, boxed = false, mono = false, style }: { text: string; boxed?: boolean; mono?: boolean; style?: React.CSSProperties }) {
  const [open, setOpen] = useState(false);
  const long = isLong(text);
  const wide = useMemo(() => hasLongLine(text), [text]);
  const cls = `content${mono || boxed ? " mono" : ""}${boxed ? " text" : ""}${long && !open ? " folded" : ""}${wide ? " nowrap" : ""}`;
  return (
    <div>
      {wide && <p className="meta" data-content>This text has a very long line, so it scrolls sideways instead of wrapping. It is whole.</p>}
      {boxed ? <pre className={cls} data-content>{text}</pre> : <p className={cls} data-content style={style}>{text}</p>}
      {long && <button className="quiet" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Show less" : "Show more"}</button>}
    </div>
  );
}

/** A summary line stands for a longer text: a button opens the whole of it under the line, and "Show less" closes it. Nothing is shown if the
 *  line is the whole text, or if there is no text at all. */
export function WholeToggle({ line, whole }: { line: string; whole: string }) {
  const [open, setOpen] = useState(false);
  // No dead control: with nothing to show, or the line the whole text, there is nothing to open.
  if (whole.trim() === "" || whole.trim() === line.trim()) return null;
  return (
    <div>
      <button className="quiet" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Show less" : "Show whole"}</button>
      {open && <LongText text={whole} />}
    </div>
  );
}
