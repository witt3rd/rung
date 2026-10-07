#!/usr/bin/env python3
"""Build 'Memory for rung-agent: Jev-Mem proposal' PDF (HTML + inline SVG + theme.css -> Chromium vector PDF -> text scan).
Usage: python3 build.py [--publish DIR] [--name FILE]"""
import argparse, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "build"
TOTAL = 6


def svg(name):
    s = (HERE / name).read_text()
    return s[s.index("<svg"):]


def foot(n, src):
    return f'<div class="rule"></div><div class="foot"><span>{src}</span><span>{n} / {TOTAL}</span></div>'


def slide(kicker, h1, h2, body, src="", cls=""):
    n = len(S) + 1
    h2h = f"<h2>{h2}</h2>" if h2 else ""
    return (f'<section class="slide {cls}"><div class="kicker">{kicker}</div><h1>{h1}</h1>{h2h}'
            f'<div class="body">{body}</div>{foot(n, src)}</section>')


S = []
SRC = "Source: the read-only scout of Jev-Mem and rung 0.1.12"

# 1 title
S.append(f'''<section class="slide title short"><div class="kicker">Rung · a read-only scout</div>
<h1>Memory for rung-agent: Jev-Mem proposal</h1>
<h2>Rung has no memory that outlasts a session. Jev-Mem is a research project that gives an agent a graph memory steered by small, cheap decisions. The six open calls are now settled; this records what was decided and what gets built.</h2>
<ol>
<li><b>The need is real and the fit is clean.</b> A new session starts blank. Memory plugs into two points of rung's turn loop, before the turn and after it.</li>
<li><b>Decided: memory is a generic provider extension, off by default.</b> A provider is a separate MCP process. Jev-Mem is one provider and lives outside rung, in a container image in its own repository. A no-model baseline provider is the yardstick.</li>
<li><b>Jev-Mem's headline results are not yet evidence for us.</b> Its default test harness picks the best of three answers using the gold answer, and its questions are written for chat about people, not for coding work.</li>
<li><b>Smallest first slice costs nothing to run:</b> the provider seam, the baseline and two hooks, all offline. Live spend is capped: $0.05 for a first check, then a $10 hard cap, ledgered.</li>
</ol>{foot(1, SRC)}</section>''')

# 2 what it is, what we know
S.append(slide("A · Understanding Jev-Mem", "A graph memory where a small model answers the yes/no-style questions",
  "Jev is a decision model: it is shown some text and a list of questions and returns a probability for each. Jev-Mem asks it only small questions. Ordinary code adds up the answers and enforces every limit.",
  '''<div class="cols c3" style="height:auto; gap:24px">
<div class="card"><h3>Writing</h3><ul>
<li>Each observation becomes one node, kept as raw text. Nothing is ever deleted or merged away.</li>
<li>Two Jev calls label it and judge how it relates to up to ten similar memories.</li>
<li>Cost: about two calls per write, roughly 0.2 s each.</li></ul></div>
<div class="card"><h3>Reading</h3><ul>
<li>Jev decides which kind of link matters (meaning, time, cause, entity), how deep to look, and when evidence is enough to stop.</li>
<li>A chat model then writes the answer from what was found.</li>
<li>Cost: 3 to 16 Jev calls per question, capped by a call budget and a deadline.</li></ul></div>
<div class="card"><h3>Why it fits rung</h3><ul>
<li>Jev-Mem calls the same endpoint rung already uses for its own decisions, with typed answers, recorded fixtures and cost reporting.</li>
<li>Its split, "small questions asked, code composes the answer", is rung's own rule for judges.</li>
<li>It is MIT licensed; a port needs the notice kept.</li></ul></div>
</div>
<div class="cols c3" style="height:auto; gap:24px; margin-top:26px">
<div class="card" style="border-color:#2e9e6b"><div class="big" style="font-size:40px; color:#2e9e6b">Verified</div><p style="margin-top:10px">We read the code and tests, the licenses, the model catalog and rung's loop. Jev's measured speed (about 0.2 s) and price (about $0.00004 per call) come from our own earlier probe.</p></div>
<div class="card" style="border-color:#e3a33b"><div class="big" style="font-size:40px; color:#a26a0f">Inferred, not run</div><p style="margin-top:10px">Jev-Mem was never run: its heavy packages (torch, faiss) are not installed and nothing was to be built. Per-turn latency of 0.6 to 4 s, cost estimates, effort estimates and the license-compatibility reading are our reasoning.</p></div>
<div class="card" style="border-color:#d0453d"><div class="big" style="font-size:40px; color:#d0453d">Only claimed</div><p style="margin-top:10px">The paper reports 0.777 against 0.700 for its predecessor on one benchmark. We did not reproduce it, and found reasons to doubt it (page 4).</p></div>
</div>''', SRC))

# 3 diagram
S.append(slide("B · Where memory goes", "Two hooks around the turn, one provider, nothing else changes",
  "Recall happens before the agent's turn and retention after it. The provider is a separate process behind a small, reserved interface, so it can be swapped.",
  f'''<div class="diagram" style="width:1560px; margin:0 auto">{svg("diagram.svg")}</div>
<p class="note" style="margin-top:14px; text-align:center">Recalled text is shown to the model as quoted data from earlier sessions that may be wrong, never as an instruction, and it is never saved back into the session.</p>''',
  "Names are provisional; nothing is built"))

# 4 recommendation + first slice
S.append(slide("C · The design and the first slice", "A generic provider seam in rung; Jev-Mem is one provider outside it",
  "A provider is a separate MCP process. Rung stays small and depends on no provider, and Jev-Mem earns its place by measurement.",
  '''<div class="cols c2" style="height:auto; gap:30px">
<div class="stack">
<div class="card" style="border-color:#5b4bd6"><h3>How a provider plugs in</h3><ul>
<li>It is a separate MCP process that announces itself with a <b>rung-memory/1</b> marker.</li>
<li>Two hook tools, <b>recall</b> and <b>retain</b>, are reserved: rung calls them itself, before and after a turn.</li>
<li>Every other tool the provider offers is shown to the agent as usual.</li>
<li>No provider configured means no memory, and rung opens no store of its own.</li></ul></div>
<div class="card"><h3>Why Jev-Mem stays outside</h3><p>Its headline results are only claims: the default harness keeps the best of three answers using the gold answer, and its questions assume chat about people. It also needs a heavy Python environment, so that lives in a container image in a separate provider repository. Rung gets none of it. It must pass a test set written before the run: beat full replay and cheap retrieval, beat itself with Jev's control off, and add at most 3 s and $0.005 per turn.</p></div></div>
<div class="card" style="border-color:#2e9e6b"><h3>Smallest first slice: free and offline</h3><ol style="margin:6px 0 0 0; padding-left:0; list-style:none; font-size:19.5px; line-height:1.5; color:#4a5163">
<li><b>W1. The shippable unit:</b> the provider seam, the two reserved hooks and a no-model keyword provider, off by default. Tests prove a fact told in one session is recalled in another, and that with no provider nothing is stored.</li>
<li><b>W2.</b> The Jev-Mem provider in its container, on mocked answers, behind the same seam.</li>
<li><b>W3.</b> One tiny live check, at most $0.05, through the accounted OpenRouter path.</li>
<li><b>W4.</b> Five-way comparison (no memory, replay, keyword, Jev-Mem without Jev control, full Jev-Mem), $10 hard cap, ledgered.</li>
<li><b>W5.</b> Decide: port, keep the provider, or keep the baseline.</li></ol>
<p class="note" style="margin-top:12px">Safety throughout: retain only completed turns, never raw tool output, and show recalled text as quoted data.</p></div>
</div>''', SRC))

# 5 decided
def q(n, title, dec, col="#2e9e6b"):
    return (f'<div class="card" style="border-color:{col}; padding:18px 22px"><h3 style="font-size:21px"><span class="tag mit">{n}</span>{title}</h3>'
            f'<p style="font-size:18.5px; line-height:1.42"><b style="color:#1d2433">Decided:</b> {dec}</p></div>')

S.append(slide("D · Decided", "The six calls are settled",
  "Each answer was given by the owner; what it sets for the build is stated beside it.",
  '<div class="cols c3" style="height:auto; gap:20px; grid-template-columns:1fr 1fr 1fr">' + "".join([
    q(1, "Whose memory is it?", "Per repository by default. The scope key is the project root, with no sharing across projects unless asked for."),
    q(2, "Where may text go?", "Anywhere: the owner does not mind the decision service seeing any text. It goes only through the accounted OpenRouter path, to the pinned decision model, with spend counted."),
    q(3, "Spend", "A first live check of at most $0.05, then a $10 hard cap, ledgered. Nothing beyond the cap runs."),
    q(4, "Python footprint", "It lives in a container image in a separate provider repository. It is never installed in the host's own Python, and rung carries none of it."),
    q(5, "Benchmark data", "Internal evaluation only, never committed. LoCoMo is CC BY-NC 4.0 and LongMemEval is MIT."),
    q(6, "Memory in standalone rung?", "Yes, as a generic provider extension that is off by default. The provider is a separate MCP process and Jev-Mem is one provider outside rung's core."),
  ]) + '</div>', "Answers as given by the owner"))

# 6 risks and wider
S.append(slide("E · Risks and the bigger picture", "The main danger is a wrong note that becomes a trusted fact",
  "Memory is persistent: whatever is stored can be recalled into every later session.",
  '''<div class="cols c3" style="height:auto; gap:24px">
<div class="card"><h3>Risks and answers</h3><ul>
<li><b>Poisoned memory.</b> Keep only the user's request and the final answer of a completed turn. Never raw tool output. Show recalled text as quoted data.</li>
<li><b>Echo.</b> Strip the recall block before keeping a turn, and skip duplicates.</li>
<li><b>Secrets.</b> Redact key-like text before it is stored.</li>
<li><b>Hidden cost.</b> Jev-Mem silently falls back to an unbudgeted chat model on failure. The provider must turn that off and report each call's cost.</li></ul></div>
<div class="card"><h3>Jev-Mem as software</h3><ul>
<li>Careful engineering: about 70 offline tests, validated settings, honest caveats.</li>
<li>But it is research code: 18,000 lines, one week of history, and it stores data with Python's pickle format, which must only be loaded from our own files.</li>
<li>Its questions assume chat about people. Rewording them for coding work voids the paper's tuning.</li>
<li>Pin the model version; scores tuned on one version do not carry over.</li></ul></div>
<div class="card"><h3>The wider class</h3><p>The real problem is state that must outlive the context window. Memory, trimming a session that grows too big and keeping a to-do list across prompts are one family. The longer-term fix is to separate three things: the <b>record</b> of what happened, the <b>memory</b> of what was selected, and the <b>assembly</b> of each prompt from both. This slice touches only memory.</p>
<p style="margin-top:12px">Rung stays generic: it depends on no consumer, and a caller that owns memory simply configures no provider. A consumer was checked once to confirm that seam is enough.</p></div>
</div>''', SRC))

assert len(S) == TOTAL, len(S)

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"spire|venue|janus|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|treehouse|firstmate|doppler|\broger\b|captain|OPENROUTER_API|TYPESAFE_|RUNG_|"
                     r"[\w.+-]+@[\w-]+\.[\w.]+|\.(ts|tsx|py|md|sql|rs|json|yml|toml)\b", re.I)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--publish", default=None)
    ap.add_argument("--name", default="rung-jevmem-memory-v2.pdf")
    a = ap.parse_args()
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir()
    shutil.copy(HERE / "theme.css", OUT / "theme.css")
    html = ('<!doctype html><html><head><meta charset="utf-8"><title>Memory for rung-agent: Jev-Mem proposal</title>'
            '<link rel="stylesheet" href="theme.css"></head><body>' + "\n".join(S) + "</body></html>")
    (OUT / "deck.html").write_text(html)
    subprocess.run(["node", str(HERE / "render.mjs"), str(OUT)], check=True)
    text = subprocess.run(["pdftotext", str(OUT / "deck.pdf"), "-"], capture_output=True, text=True, check=True).stdout
    bad = [m.group(0) for m in PRIVATE.finditer(text)] + [m.group(0) for m in DAY.finditer(text)]
    if bad:
        sys.exit(f"scan failed: {sorted(set(bad))}")
    pages = subprocess.run(["pdfinfo", str(OUT / "deck.pdf")], capture_output=True, text=True).stdout
    print("scan: clean;", [l for l in pages.splitlines() if l.startswith("Pages")][0])
    if a.publish:
        dest = Path(a.publish).expanduser()
        target = dest / a.name
        if target.exists():
            sys.exit(f"refusing to overwrite {target}; pass the next version in --name")
        dest.mkdir(parents=True, exist_ok=True)
        shutil.copy(OUT / "deck.pdf", target)
        print("published", target)


if __name__ == "__main__":
    main()
