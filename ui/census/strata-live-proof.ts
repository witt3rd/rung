// The strata page on a RUNNING instance (the mock gateway and a simulated host that keeps writing record lines): a band the reader has open
// stays open, and the context door is asked once for a past turn, not once per record line. Needs a built dist/.
//   node --experimental-strip-types census/strata-live-proof.ts
import { chromium } from "playwright";
import { startMock } from "../mock/gateway.ts";
import { join } from "node:path";
import { here } from "./lib.mjs";

const m = await startMock({ appDir: join(here, "../dist") });
const browser = await chromium.launch();
let failed = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
try {
  const page = await (await browser.newContext({ viewport: { width: 1280, height: 900 } })).newPage();
  const doorAsks: string[] = [];
  const methods = new Set<string>();
  page.on("request", (r) => { if (r.url().includes("/api/")) methods.add(r.method()); if (/\/v1\/turns\/\d+\/context/.test(r.url())) doorAsks.push(r.url()); });
  await page.goto(`${m.url}/#/i/alpha/turns/2/context?call=1`, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"] .bands', { timeout: 20000 });
  await page.click("[data-band=epoch] .bh");
  await page.click("[data-band=history] .bh");
  const open = async () => [await page.getAttribute("[data-band=epoch] .bh", "aria-expanded"), await page.getAttribute("[data-band=history] .bh", "aria-expanded")];
  check((await open()).join() === "true,true", "two bands opened");
  const asked0 = doorAsks.length;
  check(asked0 === 1, `the door was asked once for the past turn (${asked0})`);
  const lastSeq0 = (await (await fetch(`${m.url}/api/i/alpha/v1/summary`)).json()).last_seq;
  // The host writes record lines, one every 0.4 s, while the reader reads.
  let closedAt = 0;
  for (let i = 0; i < 12; i++) {
    await m.control!.poke(1);
    await sleep(400);
    if ((await open()).join() !== "true,true") closedAt++;
  }
  const lastSeq1 = (await (await fetch(`${m.url}/api/i/alpha/v1/summary`)).json()).last_seq;
  check(lastSeq1 - lastSeq0 >= 12, `the host wrote ${lastSeq1 - lastSeq0} lines while the page was open`);
  check(closedAt === 0, `the opened bands stayed open through every line (${closedAt} polls found one closed)`);
  check(doorAsks.length === asked0, `the door was not asked again for a turn that did not change (${doorAsks.length} asks in all)`);
  check([...methods].join() === "GET", `every request was a GET (${[...methods].join()})`);
} finally {
  await browser.close();
  await m.stop();
}
console.log(failed ? `strata-live-proof: ${failed} failure(s)` : "strata-live-proof: all pass");
process.exit(failed ? 1 : 0);
