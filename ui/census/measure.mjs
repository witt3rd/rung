// The measures and the verdicts of the census. Used by census.mjs (the pages) and selftest.mjs (pages made to fail).
export const B = { words: 60, filled: 1, sizes: 4 };

export async function measure(page, vp) {
  return page.evaluate(({ vh, narrow }) => {
    const vis = (e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none" && r.top < vh; };
    const words = []; let all = 0; const sizes = new Set(); const sentences = {}; let small = 0; let ids = 0;
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n; (n = w.nextNode()); ) {
      const t = n.textContent.replace(/\s+/g, " ").trim(); if (!t) continue;
      const e = n.parentElement; if (!vis(e) || e.closest("summary + *, details:not([open]) > :not(summary)")) continue;
      const fs = parseFloat(getComputedStyle(e).fontSize); sizes.add(fs); if (fs < 14) small++;
      all += t.split(" ").length;
      if (/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}\b|\b[0-9a-f]{16,}\b/i.test(t)) ids++;
      if (e.closest("[data-content]")) continue;
      words.push(...t.split(" "));
      if (t.split(" ").length >= 4) sentences[t] = (sentences[t] || 0) + 1;
    }
    const ctrl = [...document.querySelectorAll("a,button,input,select,label,summary")].filter(vis);
    const filled = ctrl.filter((e) => e.matches(".primary")).length;
    const amberBad = [...document.querySelectorAll(".mark.needs,.needs-text")].filter((e) => !e.closest(".next") && !e.closest("[data-needs]")).length;
    const tiny = narrow ? [...new Set(ctrl.filter((e) => e.matches("a,button,select,input,textarea,summary"))
      .filter((e) => !(e.matches("a") && (e.closest("p") || e.closest(".fold") || e.closest(".crumb"))))
      .map((e) => e.closest("label") || e))].filter((e) => { const r = e.getBoundingClientRect(); return r.height < 43.5 || r.width < 43.5; }).length : 0;
    const trunc = [...document.querySelectorAll("*")].filter((e) => { const s = getComputedStyle(e); return s.textOverflow === "ellipsis" || (s.webkitLineClamp && s.webkitLineClamp !== "none"); }).length;
    return { words: words.length, all, sizes: [...sizes].sort((a, b) => a - b), filled, repeated: Object.values(sentences).filter((c) => c > 1).length,
      kbd: document.querySelectorAll("kbd").length, h1: document.querySelectorAll("h1").length, small, amberBad, tiny, ids, trunc,
      links: ctrl.filter((e) => e.matches("a")).length, buttons: ctrl.filter((e) => e.matches("button,select,input,summary")).length,
      tabClipped: (() => { const t = document.querySelector(".tabs"), a = t && t.querySelector("[aria-current]"); if (!a) return false; const tr = t.getBoundingClientRect(), ar = a.getBoundingClientRect(); return ar.left < tr.left - 1 || ar.right > tr.right + 1; })(),
      hscroll: narrow && document.documentElement.scrollWidth > innerWidth };
  }, { vh: vp.height, narrow: vp.width < 500 });
}

export function judge(r) {
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
if (r.ids) bad.push(`id or hash shown ${r.ids}`);
if (r.trunc) bad.push(`truncation ${r.trunc}`);
  return bad;
}
