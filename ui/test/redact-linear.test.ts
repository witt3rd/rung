// The page's redactor must take time linear in the text, whatever the text is: every layer of a model's input passes through it, and
// a hostile text (a peer's message, a fetched page, a tool result) must not freeze the tab. No cap and no truncation: the output is whole.
import { test } from "node:test";
import assert from "node:assert/strict";
import { redactShapes, makeRedactor, REDACTED } from "../src/record/redact.ts";

const time = (f: () => unknown): number => {
  const t0 = performance.now();
  f();
  return performance.now() - t0;
};
const best = (f: () => unknown, n = 3): number => Math.min(...Array.from({ length: n }, () => time(f)));

// Texts that make a scan restart at every candidate: each shape here is a candidate that is refused, over and over.
const HOSTILE: Record<string, (n: number) => string> = {
  "eyJ- run with no dots (not a JWT)": (n) => "eyJ-".repeat(n / 4),
  "eyJ run, one dot only": (n) => "eyJ".repeat(n / 8) + "." + "eyJ".repeat(n / 8),
  "eyJ short runs separated by spaces": (n) => "eyJab ".repeat(n / 6),
  "private key BEGIN with no END, over and over": (n) => "-----BEGIN PRIVATE KEY-----\nabc\n".repeat(n / 31),
  "BEGIN markers with no label end": (n) => "-----BEGIN ".repeat(n / 11),
  "sk- that is too short, over and over": (n) => "sk-abc ".repeat(n / 7),
  "bearer words": (n) => "bearer ".repeat(n / 7),
  "a long run of token characters": (n) => "a".repeat(n),
};

for (const [name, make] of Object.entries(HOSTILE)) {
  test(`linear: ${name}: twice the text takes at most about twice the time`, { timeout: 30_000 }, () => {
    const small = make(200_000), big = make(400_000);
    redactShapes(make(8_000)); // warm up
    const t1 = best(() => redactShapes(small)), t2 = best(() => redactShapes(big));
    // Quadratic growth would be 4x; a generous 3x absorbs noise. The absolute bound catches a slow constant as well.
    assert.ok(t2 < Math.max(3 * t1, 50), `400 kB took ${t2.toFixed(1)} ms against ${t1.toFixed(1)} ms for 200 kB`);
    assert.ok(t2 < 2000, `400 kB took ${t2.toFixed(0)} ms`);
  });
}

test("a hostile text comes back whole: nothing cut, nothing dropped", () => {
  for (const make of Object.values(HOSTILE)) {
    const t = make(100_000);
    const out = redactShapes(t);
    assert.ok(out.length >= Math.min(t.length, 1) && !out.includes("…"));
    if (!out.includes(REDACTED)) assert.equal(out, t, "text with no secret in it is returned unchanged");
  }
});

test("a real JWT and a real private key are still found after a long hostile run", () => {
  const jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r";
  const out = redactShapes("eyJ-".repeat(50_000) + " then " + jwt + " and more");
  assert.ok(!out.includes(jwt) && out.includes(REDACTED) && out.endsWith(" and more"));
  const key = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END PRIVATE KEY-----";
  const o2 = redactShapes("-----BEGIN ".repeat(20_000) + key);
  assert.ok(!o2.includes("MIIEvQIBADANBgkqhkiG9w0BAQEFAASC"));
});

test("the whole door answer takes linear time too: one hostile band among many texts", () => {
  const body = { bands: [{ text: "eyJ-".repeat(100_000) }, { text: "ordinary text. ".repeat(50_000) }] };
  const r = makeRedactor([]);
  const t = best(() => r(body), 2);
  assert.ok(t < 2000, `${t.toFixed(0)} ms`);
});
