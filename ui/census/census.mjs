// The element and word census, written before the pages were judged (and run on the first draft: see census-first-draft.txt).
// Per page and width, first screen. Budgets (a failure exits non-zero):
//   chrome words <= 60       words outside [data-content]. data-content marks a fact the record gives
//                            (a name, a state, a turn's text, a count); chrome is what the page adds around it.
//   filled controls <= 1     (accent fill)          amber marks only on an element that needs the owner ([data-needs], .next)
//   text sizes <= 4          no repeated sentence (>= 4 words, twice)
//   key hints = 0 (kbd)      no horizontal scroll at 390
//   tap targets >= 44 x 44 at 390: links, buttons, summaries, selects, inputs; an input is measured by its label's box
//   text >= 14 px            heading level 1 exactly once
//   the active tab is fully visible
//   no id or hash in the main view (a uuid or a 16+ hex string in visible text)
//   no truncation: no text-overflow ellipsis and no line clamp anywhere
import { chromium, PAGES, VIEWS, serve, open } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";

const srv = await serve();
const browser = await chromium.launch();
let fails = 0;
const rows = [];
for (const [vn, vp] of Object.entries(VIEWS)) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height } });
  for (const p of PAGES) {
    const page = await open(ctx, srv.url, p.hash, p.focus);
    const r = await measure(page, vp);
    const bad = judge(r);
    fails += bad.length;
    rows.push(`${p.name.padEnd(15)} ${vn.slice(1).padEnd(5)} chrome ${String(r.words).padStart(3)} of ${String(r.all).padStart(4)} words  links ${String(r.links).padStart(2)} controls ${String(r.buttons).padStart(2)} filled ${r.filled}  sizes ${r.sizes.length}  ${bad.length ? "FAIL " + bad.join("; ") : "pass"}`);
    await page.close();
  }
  await ctx.close();
}
await browser.close(); srv.close();
console.log(rows.join("\n")); console.log(fails ? `census: ${fails} failure(s)` : "census: all pass");
process.exit(fails ? 1 : 0);
