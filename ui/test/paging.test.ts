import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pageControls } from "../src/pages/paging.ts";

test("a page of fifty: more, all and less, each offered only when it does something", () => {
  assert.deepEqual(pageControls({ shown: 50, need: 0, total: 154, page: 50 }), { count: 50, more: 50, all: true, less: false });
  assert.deepEqual(pageControls({ shown: 100, need: 0, total: 154, page: 50 }), { count: 100, more: 50, all: true, less: true });
  assert.deepEqual(pageControls({ shown: 154, need: 0, total: 154, page: 50 }), { count: 154, more: 0, all: false, less: true });
  assert.deepEqual(pageControls({ shown: 50, need: 0, total: 20, page: 50 }), { count: 50, more: 0, all: false, less: false }, "a short list has nothing to page");
});

test("a deep link keeps the list open as far as the linked turn: Show less is not offered when it would do nothing", () => {
  // Linked to the 63rd turn of 64: the list is held open at 63 whatever `shown` says, so a Show less at one page would change nothing.
  const c = pageControls({ shown: 50, need: 63, total: 64, page: 50 });
  assert.equal(c.count, 63);
  assert.equal(c.less, false, "shown is one page: nothing to go back to");
  // After the owner opens another page the button is real: it goes back to one page, which the link then holds at 63 again, but the button is gone with it.
  assert.equal(pageControls({ shown: 100, need: 63, total: 64, page: 50 }).less, true);
});

test("the fold's styles are written once: no second copy scoped to the strata page", () => {
  const css = readFileSync(join(import.meta.dirname, "../src/styles.css"), "utf8");
  const rules = (sel: string) => css.split("\n").filter((l) => l.startsWith(sel + " {")).length;
  assert.equal(rules(".content.folded"), 1);
  assert.equal(rules(".nowrap"), 1);
  assert.equal(rules(".drill .text.folded"), 0, "the general .folded serves the strata page too");
  assert.equal(rules(".drill .text.nowrap"), 0);
});
