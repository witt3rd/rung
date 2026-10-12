// The queue page against a real host behind a real gateway (the doors of the host's S1 queue): the real app in a real browser.
//   RUNG_HOST_BIN=… RUNG_GATEWAY_BIN=… node --experimental-strip-types census/real-queue-proof.ts      (needs a built dist/)
// What it shows: a message sent from the box is written by the real host, shows in the real list, a repeat adds once, the text is whole, the
// switch offers only Do next, an invisible-only message cannot be sent, and a host that has stopped is refused with the plain sentence.
import { chromium } from "./lib.mjs";
import { shootViews } from "./shoot.mjs";
import { startRealHost } from "../mock/real-host.ts";
import { join } from "node:path";
import { here } from "./lib.mjs";

const real = await startRealHost({ appDir: join(here, "../dist") });
const out: string[] = [];
const say = (s: string) => { console.log(s); out.push(s); };
let failed = 0;
const check = (ok: boolean, what: string) => { say(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };
const api = (p: string) => `${real.url}/api/i/alpha${p}`;
const queue = async () => (await (await fetch(api("/v1/queue?state=all&limit=100000"))).json()).items as { id: string; text: string; role: string; state: string }[];

let browser!: import("playwright").Browser;
try {
  browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await ctx.newPage();
  await page.goto(`${real.url}/#/i/alpha`, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.waitForFunction(() => (document.querySelector(".qrule")?.textContent ?? "").trim() !== "");
  check(await page.locator(".qctl .btn.primary").isDisabled(), "with nothing typed Send is not offered");

  const T1 = "Also include the appendix in the Q3 summary.";
  const bodies: string[] = [];
  page.on("request", (r) => { if (r.method() === "POST" && r.url().endsWith("/v1/queue")) bodies.push(r.postData() ?? ""); });
  await page.fill("textarea", T1);
  await page.click(".qctl .btn.primary");
  await page.waitForFunction((t) => [...document.querySelectorAll(".row.q")].some((r) => r.textContent?.includes(t)), T1, { timeout: 10000 });
  const mine = (await queue()).filter((i) => i.text === T1);
  check(mine.length === 1 && mine[0].role === "owner", "the real host wrote the message, once, as the owner's");
  check((await page.inputValue("textarea")) === "", "the box was emptied after the real host answered");
  check((await page.locator(".row.q", { hasText: T1 }).innerText()).includes("Do next · Owner"), "the row says Do next and Owner");

  // The same press again (as after a lost answer): same key, one item.
  const key = (JSON.parse(bodies[0]) as { key: string }).key;
  const again = await fetch(api("/v1/queue"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ key, text: T1 }) });
  check(again.status === 200 && (await again.json()).repeat === true && (await queue()).filter((i) => i.text === T1).length === 1, "a repeat with the same key adds nothing, through the real gateway");

  // Whole text, folded and unfolded.
  const LONG = Array.from({ length: 200 }, (_, i) => `Item ${i + 1} of the long brief, kept whole.`).join("\n");
  await page.fill("textarea", LONG);
  await page.click(".qctl .btn.primary");
  const row = page.locator(".row.q", { hasText: "Item 1 of the long brief" });
  await row.waitFor({ timeout: 10000 });
  check((await queue()).some((i) => i.text === LONG), "a long text is kept whole by the real host");
  await row.getByRole("button", { name: "Show more" }).click();
  check(((await row.locator(".content").textContent()) ?? "") === LONG, "and Show more gives it back character for character");
  await row.getByRole("button", { name: "Show less" }).click();

  check(await page.locator("input[value=free]").isDisabled() && await page.locator("input[value=steer]").isDisabled(), "Steer and When free are shown disabled");
  await page.fill("textarea", "\u200b\u200d");
  check(await page.locator(".qctl .btn.primary").isDisabled(), "a message of only a zero-width space cannot be sent");
  await page.fill("textarea", "");

  await shootViews(browser, `${real.url}/#/i/alpha`, "real-queue-now", { ready: (p) => p.waitForSelector(".row.q"), report: check });

  // The host stops: the page offers no Send and says what to do.
  await real.control!.halt();
  await page.reload({ waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.waitForFunction(() => document.body.innerText.includes("Start the instance first."), null, { timeout: 15000 });
  check(await page.locator(".qctl .btn.primary").isDisabled(), "a stopped real instance offers no Send, and says: start the instance first");
  const refused = await fetch(api("/v1/queue"), { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ key: "after-stop", text: "x" }) });
  check(refused.status === 409 && (await refused.json()).error === "not_running", "and the real gateway refuses an add with 409 not_running");
  check((await queue()).some((i) => i.text === T1), "the stopped instance's queue still lists, from its record");
  await shootViews(browser, `${real.url}/#/i/alpha`, "real-queue-stopped", { sizes: [390, 1920], ready: (p) => p.waitForFunction(() => document.body.innerText.includes("Start the instance first."), null, { timeout: 15000 }), report: check });
  await ctx.close();
} finally {
  await browser?.close();
  await real.stop();
}
say(failed ? `real queue proof: ${failed} failure(s)` : "real queue proof: all pass");
process.exit(failed ? 1 : 0);
