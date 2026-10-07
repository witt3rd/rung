#!/usr/bin/env python3
"""One-page PDF: the rung memory provider contract (rung-memory/1).
Sources, pinned: rung master 1930206 (PR 150) docs page on memory providers, and the settled contract notes.
Builder as in data/rung-jevmem-pdf/pdf: HTML + inline SVG + theme.css -> Chromium (Playwright) vector PDF -> text scan.
Usage: python3 build.py [--publish DIR] [--name FILE]   (never overwrites an existing file)"""
import re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_build"))
from common import Deck

HERE = Path(__file__).resolve().parent
D = Deck(HERE)
svg = D.svg


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
    D.build("Rung memory provider contract", html_body, PRIVATE, "rung-memory-provider-contract-v1.pdf")


if __name__ == "__main__":
    main()
