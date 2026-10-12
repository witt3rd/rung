// The queue proof (S1, page side): the real app in a real browser against the mock gateway and its scripted hosts.
//   node --experimental-strip-types census/queue-proof.ts      (needs a built dist/)
// What it shows: the page answers only after the host wrote the item down; a lost answer and a second press add once; the owner's items
// are a group on top and a peer item says why the desk holds it; a long text is whole under "Show more"; a stopped instance says
// "Start the instance first."; the turn view marks what the owner sent. Captures at 390, 1280, 1920 and 2878, judged by the census budgets.
import { chromium } from "./lib.mjs";
import { shootViews } from "./shoot.mjs";
import { startMock } from "../mock/gateway.ts";
import { join } from "node:path";
import { here } from "./lib.mjs";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const m = await startMock({ appDir: join(here, "../dist"), extra: true });
const host = m.hosts.get("alpha")!;
const out: string[] = [];
const say = (s: string) => { console.log(s); out.push(s); };
let failed = 0;
const check = (ok: boolean, what: string) => { say(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };
const accepted = (key?: string) => host.lines.filter((l) => l.kind === "stimulus.accepted" && l.item.role === "owner" && (key === undefined || l.item.id === key));

async function ready(page: import("playwright").Page, hash: string) {
  await page.goto(`${m.url}/${hash}`, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.evaluate(() => document.fonts.ready);
}
let browser!: import("playwright").Browser;
try {
  browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await ctx.newPage();
  await ready(page, "#/i/alpha");
  await page.waitForSelector(".qbox");
  await page.waitForFunction(() => (document.querySelector(".qrule")?.textContent ?? "").trim() !== "");

  host.addPeer("A note from atlas: the backoff file has moved.", "atlas", "at_break");

  // 1. The page answers only after the host has written it down. The POST is held; nothing may show until it is released.
  let release!: () => void;
  const gate = new Promise<void>((r) => { release = r; });
  const bodies: { key: string; text: string; intent: string }[] = [];
  await page.route("**/v1/queue", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    bodies.push(route.request().postDataJSON());
    await gate;
    return route.fallback();
  });
  const T1 = "Also include the appendix in the Q3 summary.";
  const before = accepted().length;
  await page.fill("textarea", T1);
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Sending."));
  await sleep(400);
  check(accepted().length === before, "while the host has not written it down, the record has nothing new");
  check(!(await page.locator(".row.q").allTextContents()).some((t) => t.includes(T1)), "and the list does not show it");
  check(await page.locator("textarea").evaluate((e: HTMLTextAreaElement, t: string) => e.readOnly && e.value === t, T1), "the draft is still in the box, locked, while the host has not answered");
  check((await page.locator(".qctl .btn.primary").isDisabled()), "Send cannot be pressed twice while it waits");
  release();
  await page.waitForFunction((t) => [...document.querySelectorAll(".row.q")].some((r) => r.textContent?.includes(t)), T1, { timeout: 8000 });
  check(accepted().length === before + 1 && accepted()[accepted().length - 1].item.text === T1, "after the answer the record holds it once");
  check(bodies.length === 1 && bodies[0].intent === "next" && bodies[0].text === T1, "the one request carried the text whole and the intent");
  check((await page.inputValue("textarea")) === "", "the box is emptied only after the host answered");
  await page.unroute("**/v1/queue");

  // 2. An answer lost on the way back: the host wrote it; the page says so honestly; the second press adds nothing.
  const keys: string[] = [];
  let lost = true;
  await page.route("**/v1/queue", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    keys.push(route.request().postDataJSON().key);
    if (lost) { lost = false; await route.fetch(); return route.abort("connectionreset"); }
    return route.fallback();
  });
  const T2 = "Summarise the thread.";
  await page.fill("textarea", T2);
  // Only "Do next" is served: the other two are shown, disabled, with one plain sentence saying why.
  check(await page.locator("input[value=free]").isDisabled() && await page.locator("input[value=steer]").isDisabled() && !(await page.locator("input[value=next]").isDisabled()), "Steer and When free are shown disabled; Do next is not");
  check((await page.locator(".qnote").innerText()) === "Steer and When free come later.", "one plain sentence says why");
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Press Send again"));
  check(accepted().filter((l) => l.item.text === T2).length === 1, "the host wrote it although the answer was lost");
  check((await page.inputValue("textarea")) === T2, "the box keeps the text when no answer came");
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("already there"));
  check(keys.length === 2 && keys[0] === keys[1], "the second press carried the same key");
  check(accepted().filter((l) => l.item.text === T2).length === 1, "and the record still holds it once");
  await page.unroute("**/v1/queue");

  // 2b. The text stays when the door refuses, whatever the refusal says; an invisible message is empty.
  const T3 = "Keep this draft when the door says no.";
  await page.route("**/v1/queue", async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    return route.fulfill({ status: 409, contentType: "application/json", body: JSON.stringify({ error: "not_running", message: "the instance is not running: start it first (the gateway has no start door); nothing was sent" }) });
  });
  await page.fill("textarea", T3);
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Start the instance first."));
  check((await page.inputValue("textarea")) === T3, "a refusal keeps the draft, and the page says why in plain words");
  await page.unroute("**/v1/queue");
  await page.fill("textarea", "\u200b\u200d \u2060");
  check(await page.locator(".qctl .btn.primary").isDisabled(), "a message of only invisible characters is empty: Send is disabled");
  await page.fill("textarea", "");

  // 2c. A send that ends after the owner went to another instance must touch nothing there; what was typed for the first stays with it.
  async function hold(refuse: boolean) {
    let open!: () => void;
    const gate = new Promise<void>((r) => { open = r; });
    await page.route("**/api/i/alpha/v1/queue", async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      await gate;
      if (refuse) return route.fulfill({ status: 409, contentType: "application/json", body: JSON.stringify({ error: "not_running", message: "the instance is not running: start it first" }) });
      return route.fallback();
    });
    return () => open();
  }
  const goto = async (id: string) => { await page.evaluate((i) => { location.hash = `#/i/${i}`; }, id); await page.waitForFunction((i) => document.querySelector("h1")?.textContent === i && (document.querySelector(".qrule")?.textContent ?? "").trim() !== "", id, { timeout: 10000 }); };
  const T4 = "Typed for alpha, refused after the owner left.";
  const openRefuse = await hold(true);
  await page.fill("textarea", T4);
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Sending."));
  await goto("cedar");
  check((await page.inputValue("textarea")) === "" && (await page.locator(".qctl .btn.primary").isDisabled()), "on another instance the box is empty and Send is not offered, whatever is in flight elsewhere");
  openRefuse(); await sleep(600);
  check((await page.inputValue("textarea")) === "" && !(await page.locator(".qstatus").innerText()).includes("Start the instance first."), "a refusal that arrives after the owner left leaves the other instance's box and status alone");
  await page.unroute("**/api/i/alpha/v1/queue");
  await goto("alpha");
  check((await page.inputValue("textarea")) === T4, "back on the first instance, the refused text is still there");
  const T5 = "Typed for alpha, written after the owner left.";
  const openOk = await hold(false);
  await page.fill("textarea", T5);
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Sending."));
  await goto("cedar");
  openOk(); await sleep(600);
  check((await page.inputValue("textarea")) === "" && !(await page.locator(".qstatus").innerText()).includes("Sent."), "a send that is answered after the owner left leaves the other instance's box and status alone");
  await page.unroute("**/api/i/alpha/v1/queue");
  await goto("alpha");
  check((await page.inputValue("textarea")) === "" && accepted().some((l) => l.item.text === T5), "back on the first instance the box was emptied, and the host holds the text");
  await page.fill("textarea", "");

  // 3. A second item, and a long text that folds.
  await page.fill("textarea", "Stop checking the old folder; the numbers moved.");
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Sent."));
  const LONG = Array.from({ length: 160 }, (_, i) => `Item ${i + 1} of the long brief, kept whole.`).join("\n");
  await page.fill("textarea", LONG);
  await page.check("input[value=next]", { force: true });
  await page.click(".qctl .btn.primary");
  await page.waitForFunction(() => document.body.innerText.includes("Sent."));
  await page.waitForFunction(() => document.querySelectorAll(".row.q").length >= 5);
  const longRow = page.locator(".row.q", { hasText: "Item 1 of the long brief" });
  const folded = (await longRow.locator(".content").textContent()) ?? "";
  check(folded.length < LONG.length && !/[…]|\.\.\.$/.test(folded), "a long text is shown folded, with no ellipsis");
  await longRow.getByRole("button", { name: "Show more" }).click();
  check(((await longRow.locator(".content").textContent()) ?? "") === LONG, "Show more gives the whole text, character for character");
  await longRow.getByRole("button", { name: "Show less" }).click();

  // 4. The list: owner items in their own group on top, the peer item below it with the desk's reason.
  const text = await page.locator(".qbox ~ *").allInnerTexts();
  const joined = text.join("\n");
  const iOwner = joined.indexOf("Owner items"), iOthers = joined.indexOf("From others"), iPeer = joined.indexOf("A note from atlas");
  check(iOwner >= 0 && iOthers > iOwner && iPeer > iOthers, "owner items are a group on top; the peer item is below it");
  check(joined.includes("Peer items can never be moved above the owner's."), "one line says peer items can never be moved above the owner's");
  check(joined.includes("held by the decision desk until the agent's next break"), "the peer item shows why the desk holds it");
  check((joined.match(/Do next · Owner/g) ?? []).length === 5, "each row says what was asked and by whom");
  await page.fill("textarea", "");
  await shootViews(browser, `${m.url}/#/i/alpha`, "queue-now", { ready: (p) => p.waitForFunction(() => document.querySelector(".row.q") !== null), report: check });
  await ctx.close();

  // 5. A turn that takes the items; the turn view marks what the owner sent, and a steer that came in mid-turn shows the moment.
  const running = host.runTurn({ pace: 60, kind: "committed" });
  await sleep(200);
  // A record from a later slice carries `intent` on the item; the doors serve only "next" today, so the mark is driven from the record.
  const steerKey = "11111111-aaaa-4bbb-8ccc-222222222222";
  host.append("stimulus.accepted", { item: { id: steerKey, kind: "peer", role: "owner", channel: "owner", firm: false, at: Date.now(), text: "Use the March figures, not February's.", intent: "steer" } });
  await running;
  const turn = await host.runTurn({ pace: 20, kind: "responding", take: [steerKey, ...accepted().filter((l) => l.item.text === T1).map((l) => l.item.id)] });
  const ctx2 = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const p2 = await ctx2.newPage();
  await ready(p2, `#/i/alpha/turns?turn=${turn}`);
  await p2.waitForSelector(`#turn-${turn} .tag.owner`);
  const card = (await p2.locator(`#turn-${turn}`).innerText());
  check(/Owner/.test(card) && /steer, sent/.test(card) && /task, sent/.test(card), "the turn view marks each item: a filled Owner tag, steer or task, the time sent");
  check(/came in while turn \d+ was running; the agent saw it at \d\d:\d\d:\d\d UTC/.test(card), "a steer that came in mid-turn shows the moment the agent saw it");
  const bg = await p2.locator(`#turn-${turn} .tag.owner`).first().evaluate((e) => getComputedStyle(e).backgroundColor);
  check(bg !== "rgba(0, 0, 0, 0)" && bg !== "transparent", "the Owner tag is filled");
  await shootViews(browser, `${m.url}/#/i/alpha/turns?turn=${turn}`, "queue-turn", { ready: (p) => p.waitForSelector(`#turn-${turn} .tag.owner`), scroll: { fn: (n: number) => document.querySelector(`#turn-${n}`)!.scrollIntoView({ block: "start" }), arg: turn }, report: check });
  await ctx2.close();

  // 6. A stopped instance: the list can be read, Send cannot be pressed, and the sentence is "Start the instance first."
  const dead = m.hosts.get("bramble")!;
  dead.stop();
  const ctx3 = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const p3 = await ctx3.newPage();
  await ready(p3, "#/i/bramble");
  await p3.waitForFunction(() => document.body.innerText.includes("Start the instance first."), null, { timeout: 8000 });
  check(await p3.locator(".qctl .btn.primary").isDisabled(), "on a stopped instance Send is not offered");
  const direct = await fetch(`${m.url}/api/i/bramble/v1/queue`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ key: "k", text: "x", intent: "next" }) });
  check(direct.status === 409 && (await direct.json()).error === "not_running", "and the door refuses it with 409 not_running");
  await shootViews(browser, `${m.url}/#/i/bramble`, "queue-stopped", { ready: (p) => p.waitForFunction(() => document.body.innerText.includes("Start the instance first."), null, { timeout: 8000 }), report: check });
  await ctx3.close();
} finally {
  await browser?.close();
  await m.stop();
}
say(failed ? `queue proof: ${failed} failure(s)` : "queue proof: all pass");
process.exit(failed ? 1 : 0);
