/** What a tool call's argument is on its line, and what is kept for a fold. A short, single-line argument (a path, a query) rides on the line;
 *  a long or many-line one (a note, a whole file) is not on the line and is offered whole: nothing of it is dropped. */
const INLINE_AT = 80;

const text = (v: unknown): string => {
  if (v === undefined || v === null) return "";
  if (typeof v === "string") return v;
  const o = v as Record<string, unknown>;
  for (const k of ["path", "query", "text", "cmd", "url", "note", "next_step"]) if (typeof o[k] === "string") return o[k] as string;
  const first = Object.values(o).find((x) => typeof x === "string");
  return typeof first === "string" ? first : JSON.stringify(v);
};

export function argFold(input: unknown): { inline: string; whole: string | null } {
  const s = text(input);
  if (s === "") return { inline: "", whole: null };
  if (!s.includes("\n") && s.length <= INLINE_AT) return { inline: ` ${s}`, whole: null };
  return { inline: "", whole: s };
}
