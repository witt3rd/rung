// The census must be able to fail. Each page below is built to break exactly one budget (and a first draft that breaks several);
// the census has to name each break. If any case passes, the census is too loose and this exits non-zero.
import { chromium, VIEWS } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";

const wrap = (body, css = "") => `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>
body{font:16px/1.4 sans-serif;margin:0;padding:16px} .primary{background:#3446d2;color:#fff;border:0;min-height:44px;min-width:44px}
.needs-text{color:#985800} .tabs{display:flex;overflow-x:auto;width:100%} .tabs a{padding:10px;white-space:nowrap;min-height:44px;display:inline-block}${css}</style></head><body>${body}</body></html>`;
const words = (n) => Array.from({ length: n }, (_, i) => `w${i}`).join(" ");

const CASES = [
  ["chrome words over budget", wrap(`<h1>Page</h1><p>${words(90)}</p>`), "words"],
  ["a banner saying nothing, said twice", wrap(`<h1>Page</h1><p>All systems are reachable right now.</p><p>All systems are reachable right now.</p>`), "repeated"],
  ["two filled controls", wrap(`<h1>Page</h1><button class="primary">One</button><button class="primary">Two</button>`), "filled"],
  ["five text sizes", wrap(`<h1>Page</h1><p style="font-size:15px">a</p><p style="font-size:17px">b</p><p style="font-size:19px">c</p><p style="font-size:21px">d</p>`), "sizes"],
  ["a key hint drawn on a button", wrap(`<h1>Page</h1><button>Send <kbd>Ctrl+Enter</kbd></button>`), "key hints"],
  ["text under 14 pixels", wrap(`<h1>Page</h1><p style="font-size:11px">tiny words here</p>`), "small text"],
  ["no heading, or two", wrap(`<h1>A</h1><h1>B</h1>`), "h1"],
  ["amber with no owner action", wrap(`<h1>Page</h1><span class="needs-text">Careful</span>`), "amber"],
  ["a small tap target at 390", wrap(`<h1>Page</h1><a href="#" style="display:inline-block;width:20px;height:20px">x</a>`), "tap"],
  ["sideways scroll at 390", wrap(`<h1>Page</h1><div style="width:900px;height:10px;background:#ccc"></div>`), "horizontal"],
  ["the active tab clipped", wrap(`<h1>Page</h1><nav class="tabs"><a href="#">Now</a><a href="#">Turns</a><a href="#">Queue</a><a href="#">Console</a><a href="#">Configure and more words</a><a href="#" aria-current="page" style="margin-left:300px">Last</a></nav>`), "tab"],
  ["an id shown in the main view", wrap(`<h1>Page</h1><p>turn 0123456789abcdef0123 done</p>`), "id or hash"],
  ["text cut with an ellipsis", wrap(`<h1>Page</h1><p style="width:80px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">${words(30)}</p>`), "truncation"],
];

const browser = await chromium.launch();
const vp = VIEWS.w390;
const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height } });
let missed = 0;
for (const [name, html, expect] of CASES) {
  const page = await ctx.newPage();
  await page.setContent(html);
  const bad = judge(await measure(page, vp));
  const caught = bad.some((b) => b.toLowerCase().includes(expect.toLowerCase()));
  if (!caught) missed++;
  console.log(`${caught ? "caught" : "MISSED"}  ${name.padEnd(42)} ${bad.join("; ") || "(no failure)"}`);
  await page.close();
}
await browser.close();
console.log(missed ? `selftest: ${missed} case(s) the census missed` : `selftest: the census failed all ${CASES.length} cases it was built to fail`);
process.exit(missed ? 1 : 0);
