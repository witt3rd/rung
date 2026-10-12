// No cuts anywhere in the app: a long text is folded for the eye and whole in the page; a long argument is not dropped from its line.
import assert from "node:assert/strict";
import { test, before, after } from "node:test";
import { renderToString } from "react-dom/server";
import { createElement, type FunctionComponent } from "react";
import { createServer, type ViteDevServer } from "vite";
import { join } from "node:path";
import { argFold } from "../src/pages/argFold.ts";

// A component is loaded the way the app builds it (Vite compiles the .tsx), then rendered to a string.
let vite: ViteDevServer;
let LongText: FunctionComponent<{ text: string }>;
let WholeToggle: FunctionComponent<{ line: string; whole: string }>;
before(async () => {
  vite = await createServer({ root: join(import.meta.dirname, ".."), appType: "custom", server: { middlewareMode: true }, logLevel: "silent" });
  const mod = await vite.ssrLoadModule("/src/pages/LongText.tsx");
  LongText = mod.LongText;
  WholeToggle = mod.WholeToggle;
});
after(async () => { await vite.close(); });

const MB = 1024 * 1024;
const big = (marker: string) => Array.from({ length: Math.ceil(MB / 30) }, (_, i) => `${marker} line ${i} of the long reply\n`).join("");

test("a 1 MB text is whole in the page, folded for the eye, with a way to open it", () => {
  const text = big("REPLY");
  assert.ok(text.length >= MB);
  const html = renderToString(createElement(LongText, { text }));
  assert.ok(html.includes("REPLY line 0 of"), "the start is there");
  assert.ok(html.includes(`REPLY line ${Math.ceil(MB / 30) - 1} of the long reply`), "and the very end: nothing was cut");
  assert.ok(html.length >= MB, "every character is in the page");
  assert.match(html, /class="[^"]*folded/, "folded until opened");
  assert.match(html, /aria-expanded="false"/);
  assert.match(html, /Show more/);
  assert.equal(/…|\.\.\.<|\u2026/.test(html.replace(text, "")), false, "no ellipsis added around it");
});

test("a short text has no fold and no button", () => {
  const html = renderToString(createElement(LongText, { text: "Two notes still lack a summary line." }));
  assert.equal(/folded|Show more/.test(html), false);
});

test("a long argument is not left out of its line: it folds, and the whole of it is kept", () => {
  const long = JSON.stringify({ text: "x".repeat(300) });
  const f = argFold(JSON.parse(long));
  assert.equal(f.inline, "");
  assert.equal(f.whole, "x".repeat(300), "the whole argument is offered, none of it dropped");
  const short = argFold({ path: "notes/a.md" });
  assert.equal(short.inline, " notes/a.md");
  assert.equal(short.whole, null);
  assert.equal(argFold(undefined).whole, null);
  const multi = argFold({ text: "line one\nline two" });
  assert.equal(multi.inline, "");
  assert.equal(multi.whole, "line one\nline two");
});


test("a summary line offers the whole text when there is more, and nothing when the line is all of it", () => {
  const more = renderToString(createElement(WholeToggle, { line: "First line", whole: "First line\nand a second, and a third" }));
  assert.match(more, /Show whole/);
  assert.match(more, /aria-expanded="false"/);
  assert.equal(renderToString(createElement(WholeToggle, { line: "Only line", whole: "Only line\n" })), "", "the line is the whole text: no button");
});
