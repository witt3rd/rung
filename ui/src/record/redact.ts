/** The one redactor. It replaces; it never shortens or drops a line, and has no length cap.
 *  It removes (1) the exact value of every secret it was given, which a host names by environment
 *  variable, and (2) the shapes of well-known keys. Applied to every string value of a record line,
 *  so a JSON escape cannot hide a value from it. */
export const REDACTED = "[redacted]";

const SHAPES: RegExp[] = [
  /\bsk-[A-Za-z0-9_-]{16,}/g, // provider keys
  /\bsk-or-v1-[A-Za-z0-9]{16,}/g,
  /\bgh[pousr]_[A-Za-z0-9]{20,}/g, // code-host tokens
  /\bgithub_pat_[A-Za-z0-9_]{20,}/g,
  /\bxox[abprs]-[A-Za-z0-9-]{10,}/g,
  /\bAKIA[0-9A-Z]{16}\b/g,
  /\bAIza[0-9A-Za-z_-]{30,}/g,
  /\bBearer\s+[A-Za-z0-9._~+/=-]{20,}/g,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)/g,
];

/** A secret shorter than this is more likely a word than a key: matching it would damage text. */
const MIN_SECRET = 8;

export type Redactor = (value: unknown) => unknown;

export function makeRedactor(secrets: readonly string[]): Redactor {
  const exact = [...new Set(secrets.filter((s) => s.length >= MIN_SECRET))].sort((a, b) => b.length - a.length);
  const text = (s: string): string => {
    let t = s;
    for (const v of exact) if (t.includes(v)) t = t.split(v).join(REDACTED);
    for (const re of SHAPES) t = t.replace(re, REDACTED);
    return t;
  };
  const walk = (v: unknown): unknown => {
    if (typeof v === "string") return text(v);
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === "object") {
      const o: Record<string, unknown> = {};
      for (const [k, x] of Object.entries(v)) o[text(k)] = walk(x);
      return o;
    }
    return v;
  };
  return walk;
}
