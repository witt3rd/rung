#!/usr/bin/env python3
"""Build "A continuous rung": slides -> build/deck.html -> render.mjs -> build/deck.pdf, then a text privacy scan.
Same toolchain as the choir-coordination PDF. Usage: python3 build.py [--publish DIR] [--name FILE]"""
import argparse, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "build"
TOTAL = 6
S = []


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


def q(n, title, ask, rec, chg):
    return (f'<div class="q"><h3><span>{n}.</span> {title}</h3><p>{ask}</p>'
            f'<p><b>Recommendation.</b> {rec}</p><p><b>What the answer changes.</b> {chg}</p></div>')


S.append(f'''<section class="slide title short"><div class="kicker">Rung · a design scout, read-only</div>
<h1>A continuous rung</h1>
<h2>Can a rung agent run without ever reaching a natural end, taking in the world between turns? This is the short answer, the one thing to decide first, and seven questions for the owner.</h2>
<ol>
<li><b>"Never exit" does not mean one endless model call.</b> It means turns that never reach a natural terminal, with carry between them and a boundary where the world gets in. The research this draws on says the same: continuity is a tick, not an analog process.</li>
<li><b>Recommendation: add a continuous ladder in the product crate, not the kernel.</b> It rests for free, admits one batch of stimuli at each turn boundary, and can only be stopped by an outside authority. The existing agent turn is reused unchanged.</li>
<li><b>The hard part is the record, not the loop.</b> Rung replays the whole session every turn, so an endless session fails in the limit. It needs a durable log, bounded context and spending limits.</li>
<li><b>Smallest first slice costs $0.</b> A flag on the agent, a fake stimulus source, a mock model, a log and a governor, with four pass or fail gates. Nothing has been built yet.</li>
</ol>{foot(1, "A scout report: design only; no code was written and no model was called")}</section>''')

S.append(slide("The idea", "The loop never ends, but each turn does",
  "One new ladder wraps the existing agent turn. It adds a place to rest, a place where the world is let in, and one exit that the model cannot take.",
  f'''<div class="diagram" style="width:1560px; margin:0 auto">{svg("diagram.svg")}</div>''',
  "Proposal; the ladder shape is Resting, Boundary, then Again, Settle or Halted"))

S.append(slide("What we know", "Where rung stands now, and what each claim rests on",
  "Every statement in the report carries a mark. Read the marks before relying on a claim.",
  '''<div class="cols" style="grid-template-columns: 1fr 1fr; height:auto; gap:30px">
<div class="stack">
<div class="card"><h3><span class="tag mit">Verified</span> read in the code</h3><ul>
<li>An agent run is one bounded turn per prompt. Prompts already queue in one first-in, first-out line, so a stimulus that arrives mid-turn already waits for the turn to finish.</li>
<li>That line takes one prompt per turn, with no merging, no priority, no rest state and no carry.</li>
<li>The whole session is replayed on every turn. When context overflows, old tool results are trimmed in memory only, and the trim is not saved.</li>
<li>A cancel flag is already checked before each model call and around each tool. A model call itself cannot be cancelled once started.</li></ul></div>
<div class="card"><h3><span class="tag svc">Claimed</span> from the research records</h3><ul>
<li>Results come from a related research project's frozen records, on small fixtures and an untrained model. Examples: memory re-entered once, as one labelled block, works; re-entered over and over, it destabilises the stream; a summary that replaces its predecessor overwrites its own notes.</li></ul></div></div>
<div class="stack">
<div class="card"><h3><span class="tag acc">Inferred</span> my reading, not tested</h3><ul>
<li>That those findings carry over to hosted language models. They were measured on a different kind of model.</li>
<li>The cost figures. $21.60 a day at one idle turn a minute is an illustration, not a measurement.</li>
<li>The size of the work, about 1,500 to 2,500 lines.</li>
<li>That an endless session needs compaction by construction: user lines and assistant text are never trimmed, so it overflows on text alone.</li></ul></div>
<div class="card"><h3>Not done</h3><ul>
<li>No rung build was started, nothing was run against a live model, and nothing was published.</li>
<li>One correction to the original brief: the research's "codebook" experiment never ran, and its third experiment has not started.</li></ul></div></div>
</div>''',
  "Marks: verified is read in code at the cited commit; claimed is a record's own verdict; inferred is mine"))

S.append(slide("The first slice", "Start with the smallest thing that proves the loop, at no cost",
  "Everything lives in the product crate behind a flag, with the agent loop unchanged and a mock model standing in for a real one.",
  '''<div class="cols c2" style="height:auto; gap:30px">
<div class="card" style="border-color:#5b4bd6"><h3>What gets built</h3><ul>
<li>A <b>stimulus and inbox</b> with a directory source that takes message files.</li>
<li>The <b>continuous ladder</b>: rest with zero calls, then admit, turn and record.</li>
<li>Default <b>admission rules</b>: priority, merging of repeated readings, a batch size, class limits.</li>
<li>A <b>bounded context</b>: a pinned start, a ring of the last few turns, the admitted batch. The log is written before anything leaves the ring.</li>
<li>An <b>append-only log</b> and a <b>governor</b> with call and token limits and a stop file.</li>
<li>A <b>scripted fake source</b>: operator messages, a burst of 20 peer messages, a once-a-second sensor, timers and one mid-turn interrupt.</li></ul>
<p class="note" style="margin-top:12px">Left for later slices: a cumulative carried note, a scheduled wake tool, the protocol extensions for hosted use, and checking each turn before its content is kept.</p></div>
<div class="card"><h3>Four gates, fixed before the first run</h3>
<table class="dense">
<tr><th style="width:36%">Measure</th><th>Pass when</th></tr>
<tr><td class="k">Model calls during ten quiet minutes</td><td>exactly 0</td></tr>
<tr><td class="k">Context size over 1,000 stimuli</td><td>flat once the ring fills, never above budget</td></tr>
<tr><td class="k">Time from arrival to turn start</td><td>within one turn's duration, at p95; an interrupt within the call in flight</td></tr>
<tr><td class="k">Hard kill and restart</td><td>every stimulus has exactly one outcome: admitted, merged, dropped with a reason, or pending. The gap is logged</td></tr>
</table>
<p class="note" style="margin-top:14px">Also checked: the spend cap reaches rest with no further calls, and the stop file halts within one call. Cost: $0. One optional live smoke test under a $1 cap, only if wanted.</p></div>
</div>''',
  "Proposal; nothing built. Also unrelated and worth a note: trimmed context is not saved, so a session at its limit pays one failed request every turn"))

S.append(slide("Calls for the owner · 1 of 2", "Seven questions: authority, idleness, spending and boundary",
  "Each has a recommendation. The ones marked shape the first slice; the rest can wait.",
  '<div class="stack">' +
  q(1, "Authority <span class='tag acc'>shapes slice 1</span>", "Should rung ship a self-driven mode, where it owns timers and sources, or only boundary semantics for a caller who owns every loop?",
    "Self-driven for slice 1, because the gates need rung to own an inbox. Build the same ladder so a caller can later own admission instead.",
    "Whether rung owns timers and sources at all, which decides slice 1's scope and the size of the work.") +
  q(2, "Idle policy", "Is resting the only default, with a scheduled-wake tool allowed later? If so, what floor and what per-day count?",
    "Rest only, with no idle turns. The wake tool comes after slice 1, with a floor of at least 5 minutes and a small per-day count.",
    "The self-turn guard, the cost model, and whether any turn can start without an outside stimulus.") +
  q(3, "Spending limits <span class='tag acc'>shapes slice 1</span>", "What token and dollar caps apply per hour and per day on hosted models, and who may refill them?",
    "Conservative caps, with a soft stop that rests and a higher hard cap that halts. Only an operator or control message refills. The numbers are the owner's to set.",
    "The governor's defaults, and when the loop rests versus halts.") +
  q(4, "Boundary", "Admit strictly at the next turn, as the brief said, or also steer at step boundaries for operator messages?",
    "Strictly the next turn. An operator can already cancel a running turn. Step steering needs a kernel change and can be decided on its own later.",
    "Whether the inbox must reach inside the agent's carry, which weakens the guarantee that only cancel crosses into a turn.") +
  '</div>', "Recommendations are mine; dollar and time figures are the owner's to choose", cls="tight"))

S.append(slide("Calls for the owner · 2 of 2", "Three more: first user, the research link and the protocol",
  "Only the first user touches slice 1. The other two can be deferred without blocking it.",
  '<div class="stack">' +
  q(5, "First user <span class='tag acc'>shapes slice 1</span>", "Which instance is the target: a standalone supervisor agent, the dormant rung identity on the task queue, or the research project's slow loop?",
    "The standalone supervisor. It already works from a directory of message files, which is the shape slice 1's source takes.",
    "The fake source's shape, the inbox format, and which real workload proves the loop after the gates pass.") +
  q(6, "The research project", "Should rung become a specialist the research project's loop calls on, and is GPU time wanted for three proposed experiments?",
    "Not yet. That path needs an outcome of the research that has not been built, and its own doctrine says rung must be a specialist, never a conductor. Schedule the experiments separately.",
    "Nothing in slice 1. It decides only whether rung is shaped by a second, unrelated consumer.") +
  q(7, "Protocol", "May one merged turn answer several prompt requests from the agent protocol, or must each prompt keep its own turn?",
    "Keep one turn per prompt for now. The protocol defines one response per prompt, so merging needs a protocol decision, and a consumer that already merges above rung does not need it.",
    "How hosted use is wired: merged answers, a drain prompt, or a plain prompt per stimulus. Slice 1 uses no protocol.") +
  '<div class="card" style="border-color:#2e9e6b"><h3>If these are answered</h3><p>Questions 1, 3 and 5 are enough to promote the first slice. The held backlog task for the first slice stays held until then.</p></div></div>',
  "Recommendations are mine; the protocol limit is from the agent protocol schema, read in the report", cls="tight"))

assert len(S) == TOTAL, len(S)

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"janus-?infra|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|janus-platform|treehouse|firstmate|doppler|\broger\b|captain|spire|venue|"
                     r"[\w.+-]+@[\w-]+\.[\w.]+|\.(ts|tsx|py|md|sql|rs|json|yml|toml)\b", re.I)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--publish", default=None)
    ap.add_argument("--name", default="rung-continuous-agent-v1.pdf")
    a = ap.parse_args()
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir()
    shutil.copy(HERE / "theme.css", OUT / "theme.css")
    html = ('<!doctype html><html><head><meta charset="utf-8"><title>A continuous rung</title>'
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
        dest.mkdir(parents=True, exist_ok=True)
        target = dest / a.name
        if target.exists():
            sys.exit(f"refusing to overwrite {target}")
        shutil.copy(OUT / "deck.pdf", target)
        print("published", target)


if __name__ == "__main__":
    main()
