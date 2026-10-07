#!/usr/bin/env python3
"""One-page PDF: the rung memory provider contract (rung-memory/1).
Sources, pinned: rung master 1930206 (PR 150) docs page on memory providers, and the settled contract notes.
Builder as in data/rung-jevmem-pdf/pdf: HTML + inline SVG + theme.css -> Chromium (Playwright) vector PDF -> text scan.
Usage: python3 build.py [--publish DIR] [--name FILE]   (never overwrites an existing file)"""
import argparse, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "build"


def svg(name):
    s = (HERE / name).read_text()
    return s[s.index("<svg"):]


def card(h, body, col="#dcd9d0"):
    return f'<div class="card" style="border-color:{col}; padding:16px 20px"><h3 style="font-size:20px">{h}</h3><p style="font-size:17.5px; line-height:1.38">{body}</p></div>'


RULES = [
    card("1 · Off unless chosen", "One setting, read only from rung's own flag, environment or config file, in that order. Choices: <b>off</b> (default, no change at all), <b>external</b> (caller owns memory, rung stays out), <b>baseline</b> (built-in, offline, no model) or a provider process. A bad value is an error, never a quiet fallback.", "#5b4bd6"),
    card("2 · An opaque scope key", "Rung sends a hashed key by default, made from the repository's origin address, never a raw path. The provider owns who may see what: tenancy, visibility, revocation.", "#5b4bd6"),
    card("3 · Rung bounds what goes out", "Only redacted, bounded text: the prompt up to 2,000 characters, up to three earlier answers of 500 characters, and each side of a turn up to 2,000.", "#5b4bd6"),
    card("4 · Rung caps what comes in", "At most 5 records and 4,000 characters, kept whole or left out, never cut. A recall that costs more than the provider's declared budget (default zero) is discarded. A record from the wrong scope is discarded.", "#5b4bd6"),
    card("5 · Time and failure", "Each call has a timeout (10 s by default). An overrun stops the provider for the rest of the run. Any failure ends in a typed outcome and the turn still runs. Only a completed turn is ever retained.", "#2e9e6b"),
    card("6 · Recall is data, not orders", "Recalled records are placed in front of the current user message, quoted, with provenance and a warning that they may be stale. They are never system text and never saved into the session.", "#2e9e6b"),
    card("7 · Cost is reported", "Each turn reports, for recall and retain: status, records, provider calls, dollars and latency, under <b>_meta.rung.memory</b> on the wire and in the command-line JSON.", "#2e9e6b"),
    card("8 · A conformance check", "<b>--memory-check</b> runs seven clauses against any provider: marker, declared hooks, hidden hook tools, store, recall in scope, budget kept, no leak across scopes. <b>--memory-fixture</b> is a reference provider to compare against.", "#2e9e6b"),
]

html_body = f'''<section class="slide tight" style="padding:44px 80px 96px 80px"><div class="kicker">Rung · memory provider contract · rung-memory/1</div>
<h1 style="font-size:40px">How a memory provider plugs into rung-agent</h1>
<h2 style="font-size:22px; margin-top:8px">Rung asks a separate process for memory before a turn and hands it the result after one. Rung keeps the limits; the provider keeps the memories.</h2>
<div class="body" style="margin-top:14px">
<div class="diagram" style="width:1560px; margin:0 auto">{svg("diagram.svg")}</div>
<div class="cols" style="height:auto; grid-template-columns:repeat(4,1fr); gap:16px; margin-top:16px">{"".join(RULES)}</div>
</div>
<div class="rule" style="left:80px; right:80px"></div><div class="foot" style="left:80px; right:80px"><span>Source: the rung memory provider documentation (rung master 1930206, PR 150) and the settled contract notes. Providers live outside rung's core.</span><span>1 / 1</span></div>
</section>'''

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"spire|venue|janus|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|treehouse|firstmate|doppler|\broger\b|captain|[\w.+-]+@[\w-]+\.[\w.]+", re.I)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--publish", default=None)
    ap.add_argument("--name", default="rung-memory-provider-contract-v1.pdf")
    a = ap.parse_args()
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir()
    shutil.copy(HERE / "theme.css", OUT / "theme.css")
    (OUT / "deck.html").write_text('<!doctype html><html><head><meta charset="utf-8"><title>Rung memory provider contract</title>'
        '<link rel="stylesheet" href="theme.css"></head><body>' + html_body + "</body></html>")
    subprocess.run(["node", str(HERE / "render.mjs"), str(OUT)], check=True)
    text = subprocess.run(["pdftotext", str(OUT / "deck.pdf"), "-"], capture_output=True, text=True, check=True).stdout
    bad = [m.group(0) for m in PRIVATE.finditer(text)] + [m.group(0) for m in DAY.finditer(text)]
    if bad:
        sys.exit(f"scan failed: {sorted(set(bad))}")
    pages = subprocess.run(["pdfinfo", str(OUT / "deck.pdf")], capture_output=True, text=True).stdout
    print("scan: clean;", [l for l in pages.splitlines() if l.startswith("Pages")][0])
    if a.publish:
        dest = Path(a.publish).expanduser(); target = dest / a.name
        if target.exists():
            sys.exit(f"refusing to overwrite {target}")
        dest.mkdir(parents=True, exist_ok=True)
        shutil.copy(OUT / "deck.pdf", target)
        print("published", target)


if __name__ == "__main__":
    main()
