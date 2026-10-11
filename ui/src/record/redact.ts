/** The page's redactor: a second line behind the host's. The host redacts first, with the workspace's one redactor
 *  (`rung-agent-core/src/redact.rs`); a page cannot undo what a host has put on the wire, and it knows no secret that has no shape and
 *  was not named. What it holds here is a port of that redactor's **token-shape part**, from the same table, not a list of its own:
 *  `SHAPE_TABLE` is the `SHAPES` table of `Scan::token_shape` entry for entry (a test reads the Rust file and fails if they differ), with the
 *  same boundary and length rules, plus its AWS ids, JWTs, bare `Bearer` tokens and private key bodies. It is NOT ported: auth header
 *  values, URL credentials, webhook URLs and secret-named assignments (`NAME=value`): those are the host's to remove. It replaces and
 *  never shortens or drops, and has no length cap. It also removes the exact value of every secret it is given. */
export const REDACTED = "[redacted]";

/** (prefix, minimum characters after it, may follow `_`): the Rust table, in its order. */
export const SHAPE_TABLE: readonly (readonly [string, number, boolean])[] = [
  ["sk-", 16, false],
  ["sk_live_", 16, true],
  ["sk_test_", 16, true],
  ["rk_live_", 16, true],
  ["ghp_", 20, true],
  ["gho_", 20, true],
  ["ghu_", 20, true],
  ["ghs_", 20, true],
  ["ghr_", 20, true],
  ["github_pat_", 20, true],
  ["glpat-", 16, true],
  ["xoxb-", 10, true],
  ["xoxp-", 10, true],
  ["xoxa-", 10, true],
  ["xoxr-", 10, true],
  ["xoxs-", 10, true],
  ["xapp-", 20, true],
  ["xai-", 20, true],
  ["gsk_", 20, true],
  ["tskey-", 16, true],
  ["AIza", 30, true],
  ["hf_", 30, false],
  ["npm_", 30, false],
  ["pypi-", 30, false],
  ["dp.st.", 20, true],
  ["dp.pt.", 20, true],
  ["dp.ct.", 20, true],
  ["dp.sa.", 20, true],
];

const isWord = (c: string) => /[A-Za-z0-9_]/.test(c);
const isTok = (c: string | undefined) => c !== undefined && /[A-Za-z0-9_.-]/.test(c);
const isBearer = (c: string | undefined) => c !== undefined && /[A-Za-z0-9\-._~+/=]/.test(c);
/** An escape sequence that ends exactly at `i`: `\\n`, `\\r`, `\\t`, `\\b`, `\\f` (with an escaped backslash before it too) or `\\uXXXX`. Its last character is a
 *  letter or a digit, so a plain word boundary misses it; in JSON-escaped text it is a break, and a word starts after it. */
function escapeEndsAt(t: string, i: number): boolean {
  return (i >= 2 && t[i - 2] === "\\" && "nrtbf".includes(t[i - 1])) || (i >= 6 && t[i - 6] === "\\" && t[i - 5] === "u" && /^[0-9a-fA-F]{4}$/.test(t.slice(i - 4, i)));
}
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\-]/g, "\\$&");
const PREFIX_RE = new RegExp(SHAPE_TABLE.map(([p]) => esc(p)).join("|") + "|AKIA|ASIA|eyJ", "g");

/** The end of a token that starts at `at`: the run of token characters, without the dots that end it. */
function tokRun(t: string, at: number): number {
  let e = at;
  while (isTok(t[e])) e++;
  while (e > at && t[e - 1] === ".") e--;
  return e;
}

/** The ranges of known key shapes in `t`: the port of `Scan::token_shape`, `bearer` and `private_keys`. */
function shapeRanges(t: string): [number, number][] {
  const out: [number, number][] = [];
  let glue = 0;
  PREFIX_RE.lastIndex = 0;
  for (let m = PREFIX_RE.exec(t); m; m = PREFIX_RE.exec(t)) {
    const i = m.index;
    if (i < glue) { PREFIX_RE.lastIndex = glue; continue; }
    const afterWord = i > 0 && i !== glue && !escapeEndsAt(t, i) && isWord(t[i - 1]);
    const entry = SHAPE_TABLE.find(([p]) => t.startsWith(p, i));
    if (entry) {
      const [p, min, afterUnderscore] = entry;
      if (afterWord && !(afterUnderscore && t[i - 1] === "_")) continue;
      const tail = i + p.length;
      const end = tokRun(t, tail);
      const len = end - tail;
      const ok = len >= min && (p === "sk-" || p === "hf_" ? len >= 32 || /[0-9]/.test(t.slice(tail, end)) : true);
      if (ok) { out.push([i, end]); glue = end; PREFIX_RE.lastIndex = end; }
      continue;
    }
    if (afterWord) continue;
    if ((t.startsWith("AKIA", i) || t.startsWith("ASIA", i)) && /^[A-Z0-9]{16}/.test(t.slice(i + 4, i + 20)) && t.length >= i + 20) {
      out.push([i, i + 20]); glue = i + 20; PREFIX_RE.lastIndex = glue;
    } else if (t.startsWith("eyJ", i)) {
      const end = tokRun(t, i);
      if (end - i >= 30) {
        const d1 = t.indexOf(".", i);
        const d2 = d1 >= 0 && d1 < end ? t.indexOf(".", d1 + 1) : -1;
        if (d2 >= 0 && d2 < end) { out.push([i, end]); glue = end; PREFIX_RE.lastIndex = end; }
      }
    }
  }
  // A bare `Bearer <token>`: the token goes, the word stays. Prose ("bearer authentication") has no digit or punctuation.
  for (const m of t.matchAll(/bearer([ \t]+)/gi)) {
    if (m.index > 0 && isWord(t[m.index - 1]) && !escapeEndsAt(t, m.index)) continue; // inside a word: prose
    const v = m.index + m[0].length;
    let e = v;
    while (isBearer(t[e])) e++;
    const tokenish = /[0-9\-._~+/=]/.test(t.slice(v, e));
    if (e - v >= 8 && (tokenish || e - v >= 20)) out.push([v, e]);
  }
  // Private key blocks: the body goes, the BEGIN and END frame stays.
  for (const m of t.matchAll(/-----BEGIN ([^\n]*?PRIVATE KEY)-----/g)) {
    const from = m.index + m[0].length;
    const endAt = t.indexOf("-----END ", from);
    const to = endAt < 0 ? t.length : endAt;
    const body = t.slice(from, to);
    const lead = body.length - body.trimStart().length;
    const trimmed = body.trim();
    if (trimmed) out.push([from + lead, from + lead + trimmed.length]);
  }
  return out;
}

/** `t` with every known key shape replaced by the marker; nothing else changes. */
export function redactShapes(t: string): string {
  const ranges = shapeRanges(t).sort((a, b) => a[0] - b[0] || b[1] - a[1]);
  let out = "";
  let at = 0;
  for (const [s, e] of ranges) {
    if (e <= at) continue;
    out += t.slice(at, Math.max(s, at)) + REDACTED;
    at = e;
  }
  return out + t.slice(at);
}

// What may still become a key: a prefix of the same table, at the very end of text that is still arriving, followed by token
// characters only. Built from SHAPE_TABLE, so a shape added there is protected in a stream with no second edit.
const GENERIC = SHAPE_TABLE.filter(([, , u]) => !u).map(([p]) => esc(p));
const DISTINCT = SHAPE_TABLE.filter(([, , u]) => u).map(([p]) => esc(p));
// Before a prefix: not a word character, or the end of a JSON escape (see escapeEndsAt).
const ESC_BEFORE = String.raw`(?<=\\[nrtbf])|(?<=\\u[0-9a-fA-F]{4})`;
const UNFINISHED = new RegExp(
  `(?:(?:(?<![A-Za-z0-9_])|${ESC_BEFORE})(?:${[...GENERIC, "AKIA", "ASIA", "eyJ"].join("|")})|(?:(?<![A-Za-z0-9])|${ESC_BEFORE})(?:${DISTINCT.join("|")}))[A-Za-z0-9_.-]*$`,
);
const UNFINISHED_BEARER = new RegExp(String.raw`((?:(?<![A-Za-z0-9_])|${ESC_BEFORE})bearer[ \t]+)[A-Za-z0-9\-._~+/=]+$`, "i");

/** Hide what may be the start of a key at the end of text that is still being written, until the rest arrives and the whole shape is
 *  replaced. Call it on the whole text so far, after `redactShapes`. */
export function hideUnfinished(t: string): string {
  return t.replace(UNFINISHED, REDACTED).replace(UNFINISHED_BEARER, `$1${REDACTED}`);
}

/** A secret shorter than this is more likely a word than a key: matching it would damage text. (The host's own is six.) */
const MIN_SECRET = 8;

export type Redactor = (value: unknown) => unknown;

/** Applied to every string value (and key) of a decoded JSON value, so a JSON escape cannot hide a value from it. */
export function makeRedactor(secrets: readonly string[]): Redactor {
  const exact = [...new Set(secrets.filter((s) => s.length >= MIN_SECRET))].sort((a, b) => b.length - a.length);
  const text = (s: string): string => {
    let t = s;
    for (const v of exact) if (t.includes(v)) t = t.split(v).join(REDACTED);
    return redactShapes(t);
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
