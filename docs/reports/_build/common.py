"""Shared builder for the rung report decks: slide helpers, theme, render, text scan, publish.

A deck dir keeps only its content in build.py and calls Deck(...). Layout, theme.css,
theme-questions.css, render.mjs and make.sh live here, once.
"""
import re, shutil, subprocess, sys, argparse
from pathlib import Path

SHARED = Path(__file__).resolve().parent
DAY = re.compile(r"(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)", re.I)


class Deck:
    def __init__(self, here, total=0, questions=False):
        self.here, self.out = Path(here), Path(here) / "build"
        self.total, self.questions, self.slides = total, questions, []

    def svg(self, name):
        s = (self.here / name).read_text()
        return s[s.index("<svg"):]

    def foot(self, n, src):
        return f'<div class="rule"></div><div class="foot"><span>{src}</span><span>{n} / {self.total}</span></div>'

    def slide(self, kicker, h1, h2, body, src="", cls=""):
        n = len(self.slides) + 1
        h2h = f"<h2>{h2}</h2>" if h2 else ""
        return (f'<section class="slide {cls}"><div class="kicker">{kicker}</div><h1>{h1}</h1>{h2h}'
                f'<div class="body">{body}</div>{self.foot(n, src)}</section>')

    def q(self, n, title, ask, rec, chg):
        return (f'<div class="q"><h3><span>{n}.</span> {title}</h3><p>{ask}</p>'
                f'<p><b>Recommendation.</b> {rec}</p><p><b>What the answer changes.</b> {chg}</p></div>')

    def build(self, title, body, private, default_name):
        """Write build/, render it, scan the PDF text against `private`, optionally publish."""
        ap = argparse.ArgumentParser()
        ap.add_argument("--publish", default=None)
        ap.add_argument("--name", default=default_name)
        a = ap.parse_args()
        out = self.out
        if out.exists():
            shutil.rmtree(out)
        out.mkdir()
        css = (SHARED / "theme.css").read_text()
        if self.questions:
            css += (SHARED / "theme-questions.css").read_text()
        (out / "theme.css").write_text(css)
        (out / "deck.html").write_text(
            f'<!doctype html><html><head><meta charset="utf-8"><title>{title}</title>'
            f'<link rel="stylesheet" href="theme.css"></head><body>{body}</body></html>')
        subprocess.run(["node", str(SHARED / "render.mjs"), str(out)], check=True)
        text = subprocess.run(["pdftotext", str(out / "deck.pdf"), "-"], capture_output=True, text=True, check=True).stdout
        bad = [m.group(0) for m in private.finditer(text)] + [m.group(0) for m in DAY.finditer(text)]
        if bad:
            sys.exit(f"scan failed: {sorted(set(bad))}")
        pages = subprocess.run(["pdfinfo", str(out / "deck.pdf")], capture_output=True, text=True).stdout
        print("scan: clean;", [l for l in pages.splitlines() if l.startswith("Pages")][0])
        if a.publish:
            dest = Path(a.publish).expanduser()
            target = dest / a.name
            if target.exists():
                sys.exit(f"refusing to overwrite {target}; pass the next version in --name")
            dest.mkdir(parents=True, exist_ok=True)
            shutil.copy(out / "deck.pdf", target)
            print("published", target)
