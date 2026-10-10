// The element and word census, written before the pages were judged.
// Per page and width, first screen. Budgets (a failure exits non-zero):
//   chrome words <= 60 (words outside [data-content]; content to read is not counted)
//   filled controls <= 1 (accent fill)       amber marks only on a "needs you" element
//   text sizes <= 4                          no repeated sentence (>= 4 words, twice)
//   key hints = 0 (kbd)                      no horizontal scroll at 390
//   tap targets >= 44 x 44 at 390: links, buttons, selects, inputs; an input is measured by its label's box
//   text >= 14 px                            heading level 1 exactly once
//   the active tab is fully visible (added after the first capture showed it clipped)
import { chromium, PAGES, VIEWS } from "./lib.mjs";
import { resolve, join } from "node:path";
const here = resolve(new URL(".", import.meta.url).pathname);
const B = { words: 60, filled: 1, sizes: 4 };
const browser = await chromium.launch();
let fails = 0; const rows = [];
for (const [vn, vp] of Object.entries(VIEWS)) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height } });
  for (const p of PAGES) {
    const page = await ctx.newPage();
    await page.goto("file://" + join(here, p + ".html"), { waitUntil: "load" });
    const r = await page.evaluate(({ vh, narrow }) => {
      const vis = (e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none" && r.top < vh; };
      const words = []; const sizes = new Set(); const sentences = {}; let small = 0;
      const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n; (n = w.nextNode()); ) {
        const t = n.textContent.replace(/\s+/g, " ").trim(); if (!t) continue;
        const e = n.parentElement; if (!vis(e)) continue;
        const fs = parseFloat(getComputedStyle(e).fontSize); sizes.add(fs); if (fs < 14) small++;
        if (e.closest("[data-content]")) continue;
        words.push(...t.split(" "));
        if (t.split(" ").length >= 4) sentences[t] = (sentences[t] || 0) + 1;
      }
      const ctrl = [...document.querySelectorAll("a,button,input,select,label")].filter(vis);
      const filled = ctrl.filter((e) => e.matches(".primary")).length;
      const amberBad = [...document.querySelectorAll(".mark.needs,.needs-text")].filter((e) => !e.closest(".next") && !e.closest("[data-needs]")).length;
      // Hit area: a control inside a label is hit through the label, so the label's box is measured.
      // Inline text links (in a sentence, a fold line or the breadcrumb) are exempt, as WCAG 2.5.8 allows.
      const tiny = narrow ? [...new Set(ctrl.filter((e) => e.matches("a,button,select,input,textarea"))
        .filter((e) => !(e.matches("a") && (e.closest("p") || e.closest(".fold") || e.closest(".crumb"))))
        .map((e) => e.closest("label") || e))].filter((e) => { const r = e.getBoundingClientRect(); return r.height < 43.5 || r.width < 43.5; }).length : 0;
      return { words: words.length, sizes: [...sizes].sort((a, b) => a - b), filled, repeated: Object.values(sentences).filter((c) => c > 1).length,
        kbd: document.querySelectorAll("kbd").length, h1: document.querySelectorAll("h1").length, small, amberBad, tiny,
        tabClipped: (() => { const t = document.querySelector(".tabs"), a = t && t.querySelector("[aria-current]"); if (!a) return false; const tr = t.getBoundingClientRect(), ar = a.getBoundingClientRect(); return ar.left < tr.left - 1 || ar.right > tr.right + 1; })(),
        hscroll: narrow && document.documentElement.scrollWidth > innerWidth };
    }, { vh: vp.height, narrow: vp.width < 500 });
    const bad = [];
    if (r.words > B.words) bad.push(`words ${r.words}>${B.words}`);
    if (r.filled > B.filled) bad.push(`filled ${r.filled}`);
    if (r.sizes.length > B.sizes) bad.push(`sizes ${r.sizes}`);
    if (r.repeated) bad.push(`repeated ${r.repeated}`);
    if (r.kbd) bad.push("key hints");
    if (r.h1 !== 1) bad.push(`h1 x${r.h1}`);
    if (r.small) bad.push(`small text ${r.small}`);
    if (r.amberBad) bad.push("amber off-meaning");
    if (r.tiny) bad.push(`small tap targets ${r.tiny}`);
    if (r.hscroll) bad.push("horizontal scroll");
    if (r.tabClipped) bad.push("active tab clipped");
    fails += bad.length;
    rows.push(`${p.padEnd(9)} ${vn.slice(1).padEnd(5)} words ${String(r.words).padStart(3)}  filled ${r.filled}  sizes ${r.sizes.length}  ${bad.length ? "FAIL " + bad.join("; ") : "pass"}`);
    await page.close();
  }
  await ctx.close();
}
await browser.close();
console.log(rows.join("\n")); console.log(fails ? `census: ${fails} failure(s)` : "census: all pass");
process.exit(fails ? 1 : 0);
