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
type AnyProps = Record<string, unknown>;
let TurnCard: FunctionComponent<AnyProps>;
before(async () => {
  vite = await createServer({ root: join(import.meta.dirname, ".."), appType: "custom", server: { middlewareMode: true }, logLevel: "silent" });
  const mod = await vite.ssrLoadModule("/src/pages/LongText.tsx");
  LongText = mod.LongText;
  WholeToggle = mod.WholeToggle;
  TurnCard = (await vite.ssrLoadModule("/src/pages/TurnsPage.tsx")).TurnCard;
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

test("a turn with nothing to show offers nothing to show: no dead Show whole button", () => {
  for (const line of ["In progress", "No reply (failed: provider output)", "Cut off before it finished"]) {
    assert.equal(renderToString(createElement(WholeToggle, { line, whole: "" })), "", `${line}: the turn has no text`);
    assert.equal(renderToString(createElement(WholeToggle, { line, whole: "  \n " })), "", `${line}: only blank space`);
  }
});

// The Turns page folds a long text in each place it puts one. Each place is rendered through the page's own TurnCard, so that taking the fold
// out of any one of them fails here.
const turn = (over: Record<string, unknown>) => ({
  n: 7, kind: "free", mode: null, project: null, model: "m", epoch: 1, startedAt: 1, endedAt: 2, status: "completed", failure: null, text: "Short.",
  asked: [], calls: [], decisions: [{ seq: 1, at: 1, family: "tools", boundary: 1, turn: 7, question: "Q", answer: "A", modelWould: null, who: "rule", label: "By rule", reason: "R", wallMs: 1, trivial: false }],
  llmCalls: 1, promptTokens: 1, cachedTokens: 0, completionTokens: 1, costUsd: 0, elapsedMs: 1, firstSeq: 1, lastSeq: 2, ...over,
});
const render = (t: Record<string, unknown>) => renderToString(createElement(TurnCard, { id: "alpha", t, now: 10, open: false })).replace(/<!-- -->/g, "");
const folded = (html: string) => (html.match(/folded/g) ?? []).length;

test("the Turns page folds a turn's reply", () => {
  const html = render(turn({ text: big("REPLY") }));
  assert.equal(folded(html), 1);
  assert.ok(html.includes("REPLY line 0 of") && html.includes("long reply"), "and it is whole");
  assert.equal(folded(render(turn({}))), 0, "a short reply is not folded");
});

test("the Turns page folds the text of a message the turn was asked", () => {
  const html = render(turn({ asked: [{ id: "m1", from: "owner", text: big("ASKED") }] }));
  assert.equal(folded(html), 1);
  assert.ok(html.includes("ASKED line 0 of"));
  assert.equal(folded(render(turn({ asked: [{ id: "m1", from: "owner", text: "Which notes?" }] }))), 0);
});

test("the Turns page folds a long tool argument under its call's line, and keeps a short one on the line", () => {
  const long = render(turn({ calls: [{ name: "note_write", group: null, ok: true, refused: null, input: { text: big("ARG") }, result: null }] }));
  assert.equal(folded(long), 2, "the argument is folded under the call, and again in the details of the turn");
  assert.ok(long.includes("ARG line 0 of"));
  const short = render(turn({ calls: [{ name: "ws_list", group: null, ok: true, refused: null, input: { path: "notes/a.md" }, result: null }] }));
  assert.ok(short.includes("ws_list notes/a.md"), "a path rides on the line");
});

test("the Turns page folds a long tool result and a long argument in the details of the turn", () => {
  const html = render(turn({ calls: [{ name: "ws_read", group: null, ok: true, refused: null, input: { path: "notes/a.md" }, result: big("RESULT") }] }));
  assert.equal(folded(html), 1, "the result in the details is folded (the argument is a short path: nothing else is)");
  assert.ok(html.includes("RESULT line 0 of"));
});
