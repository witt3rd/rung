// The strata page in a real browser, against the built app and its recorded data (the door's answers saved as fixtures).
//   node census/strata-proof.mjs        (needs a built dist/)
// The answers it reads are the real door's (test/fixtures/context/). What it proves: the bands are in wire order with the two cache boundaries between the right ones; each band's marker follows the call's
// cached count; a band opens to its whole text (compared with the fixture byte for byte, long ones behind Show more); the call
// selector switches the call; a turn of an old run shows partial strata and says which bands are missing and why; the page offers no
// control that writes. Exits non-zero on any failure.
import { chromium, serve, open } from "./lib.mjs";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const fx = (turn, call) => JSON.parse(readFileSync(join(import.meta.dirname, `../test/fixtures/context/ctx-demo/${turn}-${call}.json`), "utf8"));
const band = (d, id) => d.bands.find((b) => b.band === id);
const srv = await serve();
const browser = await chromium.launch();
let failed = 0;
const check = (ok, what) => { console.log(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const n = (v) => v.toLocaleString("en-US");
for (const width of (process.env.STRATA_WIDTHS ?? "390,1280,1920").split(",").map(Number)) {
  const ctx = await browser.newContext({ viewport: { width, height: 1000 } });
  console.log(`-- ${width}`);

  // the turns page offers a Context link on every turn, and it leads to the strata page
  let page = await open(ctx, srv.url, "#/i/ctx-demo/turns", null);
  const links = await page.locator("article.turn a", { hasText: "Context" }).count();
  const turns = await page.locator("article.turn").count();
  check(links === turns && turns === 14, `every turn has a Context link (${links} of ${turns})`);
  await page.locator("#turn-4 a", { hasText: "Context" }).click();
  await page.waitForSelector('.page[data-ready="true"] .bands');
  check(page.url().endsWith("#/i/ctx-demo/turns/4/context"), "the link opens turn 4's context");
  check((await page.locator("select[aria-label='Model call']").inputValue()) === "4", "no call named: the last call is shown");

  // call 2: the bands, in wire order, with the host's cache marks
  await page.locator("select[aria-label='Model call']").selectOption("2");
  await page.waitForFunction(() => document.body.innerText.includes("Call 2 of turn 4"));
  const d2 = fx(4, 2);
  const titles = await page.locator(".bands .band .bt b").allInnerTexts();
  check(eq(titles, ["Tools", "System prompt", "Epoch summary", "History", "Message", "Memory recall", "This turn so far"]), "seven bands in wire order (the header's message, then its recalled memory)");
  const seq = await page.locator(".bands > div").evaluateAll((els) => els.map((e) => (e.querySelector("[data-band]") ? e.querySelector("[data-band]").dataset.band : "") + (e.querySelector(".boundary") ? "|" + e.querySelector(".boundary").textContent : "")));
  check(seq[1].includes("Cache boundary 1") && seq[2].includes("Cache boundary 2") && !seq[0].includes("|") && !seq[3].includes("|"), "boundary 1 after the system prompt, boundary 2 after the epoch summary");
  check(/at ~[\d,]+ tokens/.test(seq[1]) && /at ~[\d,]+ tokens/.test(seq[2]), "each boundary says where the host placed it");
  const marks = await page.locator(".band .mk").allInnerTexts();
  check(eq(marks, ["cached", "cached", "cached", "cached", "cached", "cached", "partly cached"]), `markers follow the cached count (${marks.join(", ")})`);
  const p2 = d2.provider;
  const bottom = await page.locator(".total .line").innerText();
  check(bottom === `Cached ${n(p2.cached_tokens)} of ${n(p2.prompt_tokens)} provider tokens (${Math.round((p2.cached_tokens / p2.prompt_tokens) * 100)}%).`, `the bottom line: ${bottom}`);
  check((await page.locator(".total").innerText()).includes(`The provider counted ${n(p2.prompt_tokens)}`), "the provider's count stands beside the host's estimates");
  check((await page.locator('[data-band=tools] .bs').innerText()) === `~${n(band(d2, "tools").est_tokens)} tok`, "an estimated size is marked ~");
  check((await page.locator("body").innerText()).includes("Read from the host's context door."), "it says the strata are the host's door's answer");

  // call 1: part of the history is cached; the later bands are new
  await page.locator("select[aria-label='Model call']").selectOption("1");
  await page.waitForFunction(() => document.body.innerText.includes("Call 1 of turn 4"));
  check(eq(await page.locator(".band .mk").allInnerTexts(), ["cached", "cached", "cached", "partly cached", "new", "new", "nothing sent"]), "call 1: the history is partly cached, the message and the recall are new, nothing yet from this turn");

  // the first call of the host's run: a cache break is named, nothing was cached
  await page.close();
  page = await open(ctx, srv.url, "#/i/ctx-demo/turns/1/context?call=1", null);
  check((await page.locator("body").innerText()).includes("A cache break came before this call: rollover."), "the cache break before the first call is named");
  check((await page.locator("body").innerText()).includes("Cached 0 of"), "the first call: cached 0 of the provider's count");
  check(eq(await page.locator(".band .mk").allInnerTexts(), ["new", "new", "new", "nothing sent", "new", "nothing sent", "nothing sent"]), "nothing cached: every band with something in it is new; the empty ones say so");
  await page.close();

  // a band opens to its whole text, in the page
  page = await open(ctx, srv.url, "#/i/ctx-demo/turns/4/context?call=2", null);
  const sys = band(d2, "system").text;
  await page.click("[data-band=system] .bh");
  check((await page.locator("[data-band=system] .bh").getAttribute("aria-expanded")) === "true", "clicking a band opens it");
  const shown = await page.locator("[data-band=system] pre").evaluate((e) => e.textContent);
  check(shown === sys, "the system prompt is in the page whole, character for character");
  check(await page.locator("[data-band=system] pre.folded").count() === 1, "a long text is folded");
  const fh = await page.locator("[data-band=system] pre").evaluate((e) => [e.clientHeight, e.scrollHeight]);
  check(fh[1] > fh[0], `the fold keeps it short (${fh[0]} of ${fh[1]} px) without removing any of it`);
  await page.click("[data-band=system] button:has-text('Show more')");
  const fh2 = await page.locator("[data-band=system] pre").evaluate((e) => [e.clientHeight, e.scrollHeight]);
  check(fh2[1] <= fh2[0] + 1 && (await page.locator("[data-band=system] pre.folded").count()) === 0, "Show more opens the whole text");
  await page.click("[data-band=system] .bh");
  check((await page.locator("[data-band=system] pre").count()) === 0, "clicking again closes it");

  // tools: each definition is its own row, whole
  await page.click("[data-band=tools] .bh");
  const rows = await page.locator("[data-band=tools] details").count();
  const tools = band(d2, "tools").items;
  check(rows === tools.length && rows === 20, `each tool definition is its own row (${rows})`);
  await page.locator("[data-band=tools] details summary").nth(2).click();
  const opened = await page.locator("[data-band=tools] details[open] pre").first().evaluate((e) => e.textContent);
  check(opened === tools[2].text, "a tool row opens to its whole definition");
  // history: each past turn is a row, header and messages
  await page.click("[data-band=history] .bh");
  check((await page.locator("[data-band=history] details").count()) === 3, "each earlier turn is its own row");
  await page.locator("[data-band=history] details summary").first().click();
  const row0 = await page.locator("[data-band=history] details[open] pre").first().evaluate((e) => e.textContent);
  check(row0.startsWith(band(d2, "history").rows[0].header), "a history row opens to the turn's header and its messages, whole");
  // the recall and the message: the host's own text
  await page.click("[data-band=recall] .bh");
  check((await page.locator("[data-band=recall] pre").evaluate((e) => e.textContent)) === band(d2, "recall").text, "the recalled memory is shown whole");
  await page.click("[data-band=this_turn] .bh");
  check((await page.locator("[data-band=this_turn] details").count()) === band(d2, "this_turn").messages.length, "this turn so far: one row per message");

  // the top line carries the provider's count, not a sum of the estimates
  check((await page.locator("p.line", { hasText: "Call 2 of turn 4" }).innerText()).includes(`sent ${n(p2.prompt_tokens)} tokens (provider count)`), "the top line: the provider's count for the call");

  // read-only: the page has nothing that writes
  const writes = await page.locator("textarea, input[type=text], input[type=submit], form").count();
  check(writes === 0, "no field or form that could write");
  check((await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)), "no sideways scrolling");
  await page.close();

  // a hostile door answer: markup, a key shape the host failed to redact, a very long line. Nothing runs, nothing leaks, nothing is cut, nothing freezes.
  {
    const page2 = await ctx.newPage();
    const consoleMsgs = [], methods = new Set(), pageErrors = [];
    page2.on("console", (m) => { if (!/Failed to load resource/.test(m.text())) consoleMsgs.push(m.text()); });
    page2.on("pageerror", (e) => pageErrors.push(String(e)));
    page2.on("request", (r) => methods.add(r.method()));
    const KEY = "sk-or-v1-" + "a1b2c3d4e5f6".repeat(6);
    const MARKUP = `<img src=x onerror="window.__x=1"><script>window.__x=2</script><svg onload="window.__x=3"></svg>`;
    const LONG = "this is 世 text, ".repeat(Math.ceil(1_000_000 / 16));
    const hostile = structuredClone(fx(4, 2));
    band(hostile, "system").text = `${MARKUP}\nthe key is ${KEY} ok\n${LONG}`;
    band(hostile, "tools").items[0].text = `${MARKUP} ${KEY}`;
    band(hostile, "tools").items[1].text = JSON.stringify({ d: "世界 and words ".repeat(70_000) });
    band(hostile, "recall").text += `\n${MARKUP} ${KEY}`;
    band(hostile, "message").text += `\n${MARKUP}`;
    band(hostile, "history").rows[0].header += `\n${MARKUP} ${KEY}`;
    band(hostile, "this_turn").messages[0] = { role: "assistant", content: [{ type: "text", text: `${MARKUP} ${KEY}` }] };
    hostile.note = `${MARKUP}`;
    await page2.route("**/data/ctx-demo/context/4-2.json", (r) => r.fulfill({ contentType: "application/json", body: JSON.stringify(hostile) }));
    await page2.goto(srv.url + "#/i/ctx-demo/turns/4/context?call=2", { waitUntil: "load" });
    await page2.waitForSelector('.page[data-ready="true"] .bands', { timeout: 30000 });
    const title0 = await page2.title();
    const url0 = page2.url();
    const t0 = Date.now();
    await page2.click("[data-band=system] .bh", { timeout: 10000 });
    await page2.waitForSelector("[data-band=system] pre", { timeout: 10000 });
    const sysText = await page2.locator("[data-band=system] pre").evaluate((e) => e.textContent);
    const opened = Date.now() - t0;
    check(opened < 5000, `a 1 MB line of mixed CJK and ASCII opens in ${opened} ms, not tens of seconds`);
    check(sysText.endsWith(LONG) && sysText.includes("<img src=x onerror") && !sysText.includes(KEY) && sysText.includes("[redacted]"), "that band is whole (the 1 MB line to its last character), the markup is text, the key is gone");
    check(await page2.locator("[data-band=system] pre.nowrap").count() === 1 && (await page2.locator("[data-band=system] .drill").innerText()).includes("scrolls sideways"), "a very long line scrolls sideways and the page says so");
    await page2.click("[data-band=system] button:has-text('Show more')", { timeout: 10000 });
    check((await page2.locator("[data-band=system] pre").evaluate((e) => e.textContent)) === sysText, "Show more keeps the same whole text");
    for (const id of ["tools", "epoch", "history", "message", "recall", "this_turn"]) await page2.click(`[data-band=${id}] .bh`, { timeout: 10000 });
    const rowsT = await page2.locator("[data-band=tools] details summary").count();
    for (let i = 0; i < 2; i++) await page2.locator("[data-band=tools] details summary").nth(i).click({ timeout: 10000 });
    const t1 = Date.now();
    await page2.locator("[data-band=tools] details[open] pre").nth(1).evaluate((e) => e.textContent.length);
    check(Date.now() - t1 < 5000 && rowsT === 20, "a tool definition that is one 1 MB line of CJK JSON opens at once too");
    await page2.locator("[data-band=history] details summary").first().click();
    await page2.locator("[data-band=this_turn] details summary").first().click();
    const everything = await page2.evaluate(() => document.body.innerHTML + document.body.innerText);
    check(!everything.includes(KEY) && !everything.includes("a1b2c3d4e5f6a1b2"), "no key shape anywhere in the page, in any band or row");
    check((await page2.evaluate(() => window.__x)) === undefined, "markup in a band ran nothing");
    check((await page2.locator(".bands img, .bands script, .bands svg, .drill img, .drill script").count()) === 0, "markup in a band made no element");
    check((await page2.evaluate(() => [localStorage.length, sessionStorage.length, document.cookie].join("|"))) === "0|0|", "the page stored nothing: no local or session storage, no cookie");
    check((await page2.title()) === title0 && !(await page2.title()).includes("<"), "the title did not follow a band's text");
    check(page2.url() === url0, "no band text in the address: it is what it was when the page opened");
    check(consoleMsgs.length === 0 && pageErrors.length === 0, `nothing written to the console, no page error (${consoleMsgs.length}, ${pageErrors.length})`);
    check([...methods].join() === "GET", `every request was a GET (${[...methods].join()})`);
    await page2.close();
  }

  // more than 50 rows are paged, never dropped
  page = await open(ctx, srv.url, "#/i/2026-10-05-qwen-2h/turns/60/context", null);
  await page.click("[data-band=history] .bh");
  const total = Number((await page.locator("[data-band=history] .fold").innerText()).match(/of (\d+)/)[1]);
  check(total > 50 && (await page.locator("[data-band=history] details").count()) === 50, `history of ${total} turns: the first 50 rows, the rest one click away`);
  await page.click("[data-band=history] button:has-text('Show all')");
  check((await page.locator("[data-band=history] details").count()) === total, "Show all: every earlier turn is a row");
  await page.close();

  // an old run: the door's answer for a record that predates context recording: partial strata, with what is missing and why
  page = await open(ctx, srv.url, "#/i/2026-10-05-qwen-2h/turns/5/context?call=2", null);
  check((await page.locator("body").innerText()).includes("This turn predates context recording."), "old run: the host's note is on the page");
  check(eq(await page.locator(".band.gap .bt b").allInnerTexts(), ["Tools", "System prompt", "Epoch summary", "Memory recall"]), "old run: the four bands the host cannot show are marked not recorded");
  const marksOld = await page.locator(".band .mk").allInnerTexts();
  check(!marksOld.includes("cached") && !marksOld.includes("new"), `old run: no band is drawn cached or new on the host's marks, which were laid against fewer bands (${marksOld.join(", ")})`);
  await page.click("[data-band=tools] .bh");
  check((await page.locator("[data-band=tools] .drill").innerText()).includes("predates context recording"), "old run: the tools band says why it is missing");
  await page.click("[data-band=recall] .bh");
  check((await page.locator("[data-band=recall] .drill").innerText()).includes("may have been shown recalled memory"), "old run: the recall band says it may have had one");
  await page.click("[data-band=history] .bh");
  check((await page.locator("[data-band=history] details").count()) > 0, "old run: the history it does have is there, one row per earlier turn");
  check((await page.locator("body").innerText()).includes("Cached 9,984 of 10,292 provider tokens (97%)."), "old run: the provider's cached count is still the bottom line");
  await page.close();

  // a turn with no saved door answer: drawn from the record alone, and it says so
  page = await open(ctx, srv.url, "#/i/2026-10-05-qwen-2h/turns/51/context", null);
  check((await page.locator("body").innerText()).includes("what the record holds"), "record only: says it is the record's strata, not the host's");
  check(eq(await page.locator(".band.gap .bt b").allInnerTexts(), ["Tools", "System prompt"]), "record only: tools and system prompt are shown as not recorded");
  await page.click("[data-band=epoch] .bh");
  check((await page.locator("[data-band=epoch] .drill").innerText()).includes("Only the epoch's title line was recorded"), "record only: the epoch band says only its title was recorded");
  await page.click("[data-band=history] .bh");
  check((await page.locator("[data-band=history] details").count()) === 50, "record only: history is one row per earlier turn, 50 here");
  await page.close();

  // a turn that does not exist
  page = await open(ctx, srv.url, "#/i/ctx-demo/turns/99/context", null);
  check((await page.locator("body").innerText()).includes("There is no turn 99 in this record."), "no such turn: said in words");
  await page.close();
  await ctx.close();
}
await browser.close(); srv.close();
console.log(failed ? `strata-proof: ${failed} failure(s)` : "strata-proof: all pass");
process.exit(failed ? 1 : 0);
