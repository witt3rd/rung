// The page's redactor is a port of the workspace's one redactor (rung-agent-core/src/redact.rs), not a list of its own.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { SHAPE_TABLE, hideUnfinished, makeRedactor, redactShapes } from "../src/record/redact.ts";
import { redactStreaming } from "../src/live/wire.ts";

const RUST = join(import.meta.dirname, "../../rung-agent-core/src/redact.rs");

function rustShapes(): [string, number, boolean][] {
  const src = readFileSync(RUST, "utf8");
  const table = /const SHAPES: &\[\(&str, usize, bool\)\] = &\[([\s\S]*?)\];/.exec(src);
  assert.ok(table, "the SHAPES table is where this test looks for it; if it moved, move this reader");
  return [...table[1].matchAll(/\("([^"]+)",\s*(\d+),\s*(true|false)\)/g)].map((m) => [m[1], Number(m[2]), m[3] === "true"]);
}

test("the shape table is the Rust one, entry for entry and in its order", () => {
  const rust = rustShapes();
  assert.ok(rust.length >= 28);
  assert.deepEqual(SHAPE_TABLE.map((e) => [...e]), rust, "a shape added to or changed in redact.rs must be added here (or this page is a divergent copy)");
});

test("every shape in the table is redacted once whole, and a generic prefix needs a word boundary", () => {
  for (const [p, min, afterUnderscore] of SHAPE_TABLE) {
    const tail = "a1".repeat(40).slice(0, Math.max(min, 32));
    assert.equal(redactShapes(`key ${p}${tail} end`), "key [redacted] end", p);
    if (!afterUnderscore) assert.equal(redactShapes(`task${p}${tail}`), `task${p}${tail}`, `${p} inside a word is not a key`);
    if (afterUnderscore) assert.equal(redactShapes(`KEY_${p}${tail}`), "KEY_[redacted]", `${p} may follow an underscore`);
    if (min > 3) assert.equal(redactShapes(`${p}${"a1".repeat(1).repeat(min - 1).slice(0, min - 1)}`).includes("[redacted]"), false, `${p} with a tail shorter than ${min} is not yet a key`);
  }
});

test("sk- and hf_ need a digit or 32 characters, as in Rust: prose is left alone", () => {
  assert.equal(redactShapes("sk-" + "abcdefghijklmnop"), "sk-abcdefghijklmnop", "16 letters, no digit, shorter than 32");
  assert.equal(redactShapes("sk-" + "abcdefghijklmnop1"), "[redacted]");
  assert.equal(redactShapes("hf_" + "a".repeat(31)), "hf_" + "a".repeat(31));
  assert.equal(redactShapes("hf_" + "a".repeat(32)), "[redacted]");
});

test("AWS ids, JWTs, a bare Bearer token (the token goes, the word stays) and a private key body (the frame stays)", () => {
  assert.equal(redactShapes("id AKIAABCDEFGHIJKLMNOP."), "id [redacted].");
  assert.equal(redactShapes("id AKIA123"), "id AKIA123");
  const jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r";
  assert.equal(redactShapes(`t ${jwt} x`), "t [redacted] x");
  assert.equal(redactShapes("Authorization is Bearer abc.def-123_456 ok"), "Authorization is Bearer [redacted] ok");
  assert.equal(redactShapes("bearer authentication is used"), "bearer authentication is used", "prose");
  assert.equal(redactShapes("-----BEGIN PRIVATE KEY-----\nMIIEvQIBADAN\nabc\n-----END PRIVATE KEY-----"), "-----BEGIN PRIVATE KEY-----\n[redacted]\n-----END PRIVATE KEY-----");
});

test("what the page does not port stays the host's: a header value, a URL password and an assignment pass through", () => {
  // Stated, not hidden: these are removed by the host's redactor (rung-agent-core), and the contract says the host redacts first.
  assert.equal(redactShapes("password=hunter2hunter2"), "password=hunter2hunter2");
  assert.equal(redactShapes("https://user:hunter2hunter2@host/x"), "https://user:hunter2hunter2@host/x");
});

test("an unfinished shape is hidden for every prefix in the table, and only at the end", () => {
  for (const [p] of SHAPE_TABLE) {
    assert.equal(hideUnfinished(`see ${p}a1b`), "see [redacted]", `${p} at the end of streamed text`);
    assert.equal(hideUnfinished(`see ${p}a1b and more`), `see ${p}a1b and more`, `${p} followed by more text is judged whole by the shape rules, not hidden`);
  }
  assert.equal(hideUnfinished("a Bearer abc"), "a Bearer [redacted]");
  assert.equal(hideUnfinished("an ask- or task_ for help"), "an ask- or task_ for help");
});

test("streamed text: whole shapes replaced, unfinished ones hidden, in one call on the text so far", () => {
  assert.equal(redactStreaming("k sk-or-v1-0123456789abcdef0123456789abcdef then sk-or-v1-01"), "k [redacted] then [redacted]");
});

test("the exact value of a named secret goes first, whatever its shape", () => {
  assert.equal(makeRedactor(["plain-secret-value"])("x plain-secret-value y"), "x [redacted] y");
});

// Word boundaries. A plain "not a word character before it" is wrong in both directions: it redacts inside a word, and it misses a word that
// starts after a JSON escape, whose last character (the `t` of `\t`) is a letter. The host's redactor treats an escape as a break.
test("a bare Bearer needs a word boundary before it: inside a word it is prose", () => {
  assert.equal(redactShapes("xBearer abc.def-123_456"), "xBearer abc.def-123_456");
  assert.equal(redactShapes("a Bearer abc.def-123_456"), "a Bearer [redacted]");
  assert.equal(redactShapes("Bearer abc.def-123_456"), "Bearer [redacted]", "at the start of the text");
  assert.equal(redactShapes("x\tBearer abc.def-123_456"), "x\tBearer [redacted]", "after a real tab");
});

test("a word that starts after a JSON escape is a word: Bearer, and a key shape, after \\t \\n \\r \\b \\f and \\uXXXX", () => {
  for (const esc of ["\\t", "\\n", "\\r", "\\b", "\\f", "\\u000a", "\\u00A0", "\\\\n"]) {
    assert.equal(redactShapes(`x${esc}Bearer abc.def-123_456`), `x${esc}Bearer [redacted]`, `Bearer after ${esc}`);
    assert.equal(redactShapes(`x${esc}sk-or-v1-0123456789abcdef0123456789abcdef`), `x${esc}[redacted]`, `a key after ${esc}`);
    assert.equal(redactShapes(`x${esc}AKIAABCDEFGHIJKLMNOP`), `x${esc}[redacted]`, `an AWS id after ${esc}`);
  }
  // An escape-looking tail that is not an escape is still a word.
  assert.equal(redactShapes("x\\qBearer abc.def-123_456"), "x\\qBearer abc.def-123_456");
  assert.equal(redactShapes("x\\u00zzBearer abc.def-123_456"), "x\\u00zzBearer abc.def-123_456");
});

test("an unfinished key or Bearer after a JSON escape is hidden too", () => {
  assert.equal(hideUnfinished("x\\nsk-or-v1-01"), "x\\n[redacted]");
  assert.equal(hideUnfinished("x\\tBearer abc"), "x\\tBearer [redacted]");
  assert.equal(hideUnfinished("xBearer abc"), "xBearer abc", "inside a word it is prose");
});
