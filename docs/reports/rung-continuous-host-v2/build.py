#!/usr/bin/env python3
"""Build "A continuous rung host" (v2): slides -> build/deck.html -> render.mjs -> build/deck.pdf, then a text privacy scan.
Same toolchain as the choir-coordination PDF. Usage: python3 build.py [--publish DIR] [--name FILE]"""
import re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_build"))
from common import Deck

HERE = Path(__file__).resolve().parent
D = Deck(HERE, total=8, questions=True)
S = D.slides
svg, foot, slide, q = D.svg, D.foot, D.slide, D.q


S.append(f'''<section class="slide title short"><div class="kicker">Rung · design, version 2 · read-only, nothing built</div>
<h1>A continuous rung host</h1>
<h2>A host that keeps one rung agent alive with no resting state. It owns the agent's context, time, memory, workers and stop. It replaces the resting design of version 1.</h2>
<ol>
<li><b>What it is.</b> A new product crate. It imports the agent crate as a library, runs one continuous agent, speaks the agent protocol outward, and runs as a container or a system service.</li>
<li><b>No rest.</b> After every turn the next one begins. When nothing has arrived from outside, the host gives the agent its strongest pull: an obligation, a stale trajectory, a test that is due, curiosity, an integrity check.</li>
<li><b>Its main job is context.</b> The host assembles every turn's context, bounded and labelled. The record is written before anything is forgotten, so a restart loses nothing but time.</li>
<li><b>Anticipation starts small.</b> A register of explicit expectations, each one settled and scored by the host, never by the agent. The agent's own first project is a fuller model of itself and its world.</li>
<li><b>The first slice is free.</b> A library split, then the host running against a mock model, with eleven pass-or-fail gates. Three questions for the owner are on the last page.</li>
</ol>{foot(1, "Design scout: no code, no model call. Marks: verified = read in code at current master; claimed = a research record&#39;s verdict; inferred = mine")}</section>''')

S.append(slide("The idea", "The loop never rests, but each turn is still bounded",
  "Stimuli and the agent's own pulls compete at one boundary. One bounded turn runs, the record is written, and the next boundary comes at once.",
  f'''<div class="diagram" style="width:1580px; margin:0 auto">{svg("diagram.svg")}</div>
<div class="cols" style="grid-template-columns: 1.6fr 1fr; height:auto; gap:24px; margin-top:16px">
<div class="card"><h3><span class="tag mit">Settled</span> by the owner, and treated as fixed</h3>
<p>A new rung crate hosts the agent and imports it directly · it speaks the agent protocol outward · it runs as a service or a container · it provides memory, and managing context is its main job · <b>there is no resting state</b> · cost is only a safety valve · time is just context, and the host keeps the schedule · pulls drive the next turn · anticipation is research, so ship a minimal register first · the agent commissions workers and is never the sole worker.</p></div>
<div class="card"><h3><span class="tag acc">Changed</span> from version 1</h3>
<p>Resting and zero-cost waiting are gone. The caller-driven mode and the wake-me tool are retired. Spending caps become a safety valve. Seven owner questions become three.</p></div>
</div>''',
  "Proposal. The type of the loop has no resting state, so the agent cannot choose to rest or to stop; the only waits are ones the world forces", cls="tight"))

S.append(slide("The pieces", "One new crate, and a cleaner library underneath it",
  "Everything continuous goes in the new crate. The agent crate only gets easier to import, and its command line behaves exactly as before.",
  '''<div class="cols" style="grid-template-columns: 1fr 1.15fr; height:auto; gap:30px">
<div class="card"><h3>Crates</h3>
<table class="dense">
<tr><th style="width:30%">Crate</th><th>Role and change</th></tr>
<tr><td class="k">ladder runtime</td><td>No change.</td></tr>
<tr><td class="k">standard blocks</td><td>Small changes: the reason a provider failed, and how long to wait, reach the caller. Tools can take a workspace root.</td></tr>
<tr><td class="k">memory</td><td>No change. The host drives the provider directly.</td></tr>
<tr><td class="k">agent</td><td>Becomes library-first: one call runs one turn. Its command line and protocol server become clients of that call.</td></tr>
<tr><td class="k"><b>host</b> (new)</td><td>The loop, record, inbox, calendar, work registers, arbiter, context assembly, memory, workers, governor, stop authority, outward protocol, container and service templates.</td></tr>
</table>
<p class="note" style="margin-top:12px">The host is a product crate. Pulls and arbitration are judgments of worth, so they stay out of the kernel until a second, unrelated host needs them.</p></div>
<div class="card"><h3><span class="tag mit">Verified</span> what blocks importing the agent now, and the change</h3><ul>
<li>A job is described by the command-line argument set. <b>Change:</b> a plain specification of values.</li>
<li>Configuration is read from files and the environment inside every turn. <b>Change:</b> values are given once.</li>
<li>Each turn rebuilds context by replaying the whole session file. <b>Change:</b> run one turn on context the caller assembled, with no session side effects.</li>
<li>Memory recall and retention happen inside the job. <b>Change:</b> the host owns them; the agent runs with memory set to external.</li>
<li>Tool servers reconnect on every prompt. <b>Change:</b> one connection for the life of the host.</li>
<li>Tools resolve paths against the process's one working directory, so isolated workers cannot run side by side. <b>Change:</b> a workspace root per run; child processes until then.</li>
<li>Diagnostics go to standard error, and a provider failure arrives as plain text. <b>Change:</b> an event sink, and a typed failure with its wait time.</li></ul></div>
</div>
<div class="card" style="margin-top:24px"><h3>Running it</h3><p>One binary or one container image. Its configuration names the providers (a primary and a fallback) by the variable that holds each key, never the key itself, plus the memory provider, the seed identity and first projects, the protocol channels with their role tokens, and the record directory. The container mounts the record and memory as volumes. The service unit restarts the host after a crash, but not after a deliberate stop. Both templates ship with the crate, free of any consumer's names.</p></div>''',
  "Each gap is cited by file and line in the report; the library split is slice 0 and keeps every existing test passing", cls="tight"))

S.append(slide("What drives the next turn", "Pulls compete; time arrives as context",
  "With nothing waiting from outside, the host surfaces one of the agent's own concerns and hands over the agent's own material for it. It never writes the thought.",
  '''<div class="cols" style="grid-template-columns: 1.25fr 1fr; height:auto; gap:30px">
<div class="card"><h3>Six pulls</h3>
<table class="dense">
<tr><th style="width:22%">Pull</th><th style="width:40%">Grows with</th><th>Aims at</th></tr>
<tr><td class="k">obligation</td><td>open commitments, their age and due dates</td><td>an empty list</td></tr>
<tr><td class="k">trajectory</td><td>time since the last step, against a staleness limit</td><td>one concrete step</td></tr>
<tr><td class="k">inquiry</td><td>a pre-registered prediction ready to test; a replication owed</td><td>confirm or falsify</td></tr>
<tr><td class="k">curiosity</td><td>time since the last turn spent for its own sake</td><td>nothing useful; leave a trace</td></tr>
<tr><td class="k">integrity</td><td>a period, plus alarms: cycling, repetition, unchecked claims</td><td>fix, not just observe</td></tr>
<tr><td class="k">consolidation</td><td>context near its budget; turns since the last note</td><td>a cumulative note; memory</td></tr>
</table>
<p class="note" style="margin-top:12px">If several self-directed turns in a row touch nothing in the world, an integrity turn is forced. Loops that think with no outside contact tend to drift.</p></div>
<div class="stack">
<div class="card"><h3>The arbiter, version 0</h3><ul>
<li>A waiting owner message always goes first.</li>
<li>Otherwise the highest score wins. Fairness floors guarantee curiosity and integrity at least one turn in ten each, and the same item winning three of five turns is damped.</li>
<li>The agent may defer a winning item, with a reason. It may never raise its own score.</li>
<li>The whole ballot is recorded every turn, so every self-directed turn can be audited later. The policy can be swapped.</li></ul></div>
<div class="card"><h3>Time and the calendar</h3><ul>
<li>Every item carries its time, and the context states the clock, the gaps and any lateness.</li>
<li>The host owns one calendar with entries from the owner, from standing routines and from the dates on the agent's own work items. Each fires into the stream when due.</li></ul></div>
<p class="note"><span class="tag acc">Open problem</span> No one knows the right weights. Version 0 makes them explicit and logged so they can be studied.</p></div>
</div>''',
  "Pull kinds follow a reference persona agent's six skills (read only, not a dependency); the arbiter is my proposal", cls="tight"))

S.append(slide("Expecting and delegating", "An honest ledger of expectations, and workers that never block the top",
  "Both are kept deliberately small: what the agent believes will happen, and who does the long work.",
  '''<div class="cols c2" style="height:auto; gap:30px">
<div class="stack">
<div class="card"><h3>The expectation register</h3><ul>
<li>The agent states <b>what</b> it expects, <b>why</b> (links to the record or memory), <b>by when</b>, and <b>how sure</b>, as a probability.</li>
<li>The host settles it, <b>never the agent</b>. A checkable claim is settled from the record and the world. A judged claim goes to a separate judge who sees the evidence but not the agent's reasoning.</li>
<li>Every outcome records its surprise: how unlikely the result was by the agent's own stated odds. Outcomes come back as stimuli, and are kept in memory with their surprise.</li>
<li>Calibration and resolution are shown together, so safe, trivially true expectations cannot pass for skill.</li></ul></div>
<p class="note"><span class="tag svc">Claimed</span> A reference research harness found that choosing what to recall by surprise did no better than choosing by similarity (5 of 10 pairs each). So version 0 records surprise and makes no promise that it improves memory.</p>
<p class="note"><span class="tag acc">Open problem</span> A real anticipatory system holds a model of itself and its world and acts now on what it predicts. That is the agent's own first project, seeded by the owner. Rung ships the register, not the theory.</p></div>
<div class="stack">
<div class="card"><h3>Commissioned workers</h3><ul>
<li>One call commissions work with a brief, a tool scope, a deadline, a budget and acceptance checks. It returns a ticket <b>at once</b>; the top turn never waits.</li>
<li>Workers are ordinary agent runs from the same library, up to four at a time. They cannot commission others: one hub, many spokes, no side channels.</li>
<li><b>The host runs the acceptance checks itself</b> before the agent sees a result. The verdict is accepted, unverified, failed or timed out. Results never write memory or work items directly.</li>
<li>The top turn has a time bound. A tool call that would run long is refused with a hint to commission it, so long work is always delegated.</li>
<li>A ticket running at a crash is marked lost and reported. It is not re-run automatically.</li></ul></div>
<div class="card"><h3>Talking to it from outside</h3><ul>
<li>There is <b>one agent</b>. Opening a protocol session opens a <b>channel</b> to it, with an owner, peer or observer role and its own token.</li>
<li>A prompt is a stimulus. It is recorded on arrival and answered when the turn that admits it ends. Several channels may share a turn, and each prompt still gets exactly one answer.</li>
<li>Messages the agent starts on its own wait in a per-channel outbox. Cancelling withdraws a waiting prompt; only the owner can cut a running turn.</li>
<li>Extensions report status, and let the owner stop the host or add standing calendar items.</li></ul></div></div>
</div>''',
  "Proposal; the surprise result is a reference record's own verdict, measured at its scope only", cls="tight"))

S.append(slide("Staying up", "Stopping, restarting and surviving the provider",
  "Nothing here depends on the loop choosing to exit. A wedged loop, a lost provider and a hard kill each have a defined way back.",
  '''<div class="cols c3" style="height:auto; gap:26px">
<div class="card"><h3>Stop authority</h3><ul>
<li>A stop comes from a signal, a stop file or an owner channel. It is checked at every boundary, cuts the running turn and cancels the workers.</li>
<li>Every wait listens for a stop.</li>
<li>A model call already sent cannot be cancelled, so the grace period is the call's timeout.</li>
<li>Outside the process, the service manager or container runtime stops what overruns. A watchdog fed from the loop itself restarts a loop that wedges.</li>
<li>A deliberate stop is not restarted. There is no pause: a stop and a start make a recovery.</li></ul></div>
<div class="card"><h3>Durability and restart</h3><ul>
<li>One append-only record. A stimulus is written durably before it is acknowledged.</li>
<li>Work items, calendar and tickets are rebuilt from the record at start, checked against a stored hash.</li>
<li>An unfinished turn is re-admitted and marked as cut by the restart. Missed calendar items fire once, late, with the lateness stated.</li>
<li>The next context opens with the gap: when the host was down, and what arrived meanwhile.</li></ul>
<p class="note" style="margin-top:10px"><span class="tag acc">Open problem</span> Is the restarted agent the same one? The honest claim is continuity of commitments and knowledge, with the gap stated.</p></div>
<div class="card"><h3>Provider trouble: back off, never die</h3><ul>
<li><b>Rate limits, overload, network loss:</b> return the stimuli to the head of the line, wait as asked or with growing jittered delays up to 15 minutes, and switch to a fallback provider if one is set.</li>
<li><b>Bad key or exhausted quota:</b> mark the host blocked, tell the owner once, probe every 15 minutes, keep accepting stimuli.</li>
<li><b>Refusals:</b> recorded, then the next turn.</li>
<li>Waits are recorded as degraded time, not chosen rest.</li></ul></div>
</div>
<div class="card" style="margin-top:24px"><h3>Safety valves, not budgets</h3>
<table class="dense">
<tr><th style="width:26%">Valve</th><th style="width:44%">Rule</th><th>When it trips</th></tr>
<tr><td class="k">Spending cap</td><td>A per-day dollar cap, for paid providers only. Off for free and local ones.</td><td>Halts the host; the owner restarts it</td></tr>
<tr><td class="k">Turn-rate ceiling</td><td>A generous maximum number of turns per minute, as a guard against a runaway bug</td><td>Waits briefly, recorded as degraded</td></tr>
<tr><td class="k">Grounding streak</td><td>Several self-directed turns in a row with no tool use, worker, change to work items or message</td><td>Forces an integrity turn</td></tr>
<tr><td class="k">Repetition guard</td><td>Turn output or the carried note nearly copies recent ones</td><td>Forces an integrity turn and tells the owner</td></tr>
</table></div>''',
  "Proposal; the uncancellable model call and the untyped failure text are verified gaps", cls="tight"))

S.append(slide("The first slice", "Prove the loop that never rests, at no cost",
  "Slice 0 splits the agent library with no change in behaviour. Slice 1 builds the host against a scripted mock model, a fake world and injected provider faults. The gates are fixed before the first run.",
  '''<div class="cols" style="grid-template-columns: 1.55fr 1fr; height:auto; gap:28px">
<div class="card"><h3>Eleven gates</h3>
<table class="dense">
<tr><th style="width:24%">Gate</th><th>Passes when</th></tr>
<tr><td class="k">No rest</td><td>Over 30 quiet minutes, under 2% of the time is spent outside a turn, apart from declared degraded time. The next turn starts within 50 ms of the last one ending, p99.</td></tr>
<tr><td class="k">Responsive</td><td>Owner messages are admitted within one turn's duration, p95. No top turn runs past its bound.</td></tr>
<tr><td class="k">Fair pulls</td><td>Every live pull is served within 12 self-directed turns. No pull takes more than half of any 50-turn window.</td></tr>
<tr><td class="k">Schedule</td><td>Due items enter at the next boundary. Items missed while down fire once, marked late.</td></tr>
<tr><td class="k">Expectations</td><td>Only the host can settle one. Calibration recomputed from the record matches exactly.</td></tr>
<tr><td class="k">Workers</td><td>Three or more run while the top keeps turning. Each carries a verdict from the host. Workers cannot commission.</td></tr>
<tr><td class="k">Provider faults</td><td>The process never exits. Requeued stimuli are admitted exactly once, and requested waits are honoured.</td></tr>
<tr><td class="k">Stop</td><td>A halt within the remaining mock call plus 1 s, or within 1 s from a wait. The watchdog catches a wedged loop.</td></tr>
<tr><td class="k">Hard kill</td><td>After 50 random kills, every stimulus has exactly one outcome, the rebuilt state matches, and the gap is stated.</td></tr>
<tr><td class="k">Bounded</td><td>Over 10,000 turns, context stays flat once full and never exceeds its budget.</td></tr>
<tr><td class="k">Cost</td><td>Zero: no network beyond loopback, no live model.</td></tr>
</table></div>
<div class="card"><h3>Main risks</h3><ul>
<li><b>Rumination:</b> a loop that cannot rest copies itself. Grounding duty, repetition guard and integrity turns stand against it, but they are untested on a real model.</li>
<li><b>Gaming the arbiter:</b> the agent shapes its own work items to win the turns it likes.</li>
<li><b>Expectation theatre:</b> only safe bets get made.</li>
<li><b>Free-provider starvation:</b> rate limits keep the host degraded much of the time.</li>
<li><b>Runaway delegation:</b> capped by worker count, ticket budgets and depth one.</li>
<li><b>Exposure</b> of an always-on endpoint: loopback by default, role tokens, owner-only stop.</li>
<li><b>Disk growth</b> of a record that is never deleted, only rotated.</li></ul>
<p class="note" style="margin-top:10px">Size, inferred: slice 0 is about 1,000 lines changed, slice 1 about 3,000 to 4,000 with tests. The protocol and the real engine come in slice 2; a live run in slice 3.</p></div>
</div>''',
  "Proposal; nothing built. Gate figures are frozen targets, not measurements", cls="tight"))

S.append(slide("Calls for the owner", "Three questions, most blocking first",
  "Each is one decision, with my recommendation and what the answer changes. Everything else I decided myself; those choices are listed alongside so any can be overruled.",
  '<div class="cols" style="grid-template-columns: 1.5fr 1fr; height:auto; gap:26px"><div class="stack">' +
  q(1, "Build slices 0 and 1 now?", "May the agent library be split into a clean library, with no change in behaviour, and the host then built against a mock model, at no cost and behind the gates?",
    "Yes. Both are free, reversible and gated, and nothing reaches a live model.",
    "Whether the held first-slice task is released to build, or the design waits.") +
  q(2, "First live run: a local GPU or a free hosted router?", "Which substrate hosts the first real run?",
    "A local GPU behind a standard chat endpoint, with the free router as fallback. A loop that never rests meets free-tier rate limits constantly. A local model also exposes token probabilities for real surprise.",
    "Which machine runs slice 3, the backoff defaults, whether token-level surprise enters the register, and whether the host shares a GPU with other work.") +
  q(3, "May the agent change the world itself, or only through workers?", "Should the always-on agent hold write, shell and web tools itself?",
    "No. Give it reading, memory, its work items, messages and commissioning only. Every change to the world goes through a worker whose result the host checks.",
    "The top agent's default tools, the safety envelope of an agent no one watches, and whether the host check covers every effect on the world or only delegated work.") +
  '</div><div class="card"><h3>Decided by me</h3><ul>'
  '<li>Name and place: the host crate lives in the rung workspace.</li>'
  '<li>Stimuli enter only at the next turn boundary.</li>'
  '<li>One agent, many protocol channels. Turns may be shared, each prompt still gets exactly one answer, and agent-initiated messages go to an opt-in outbox.</li>'
  '<li>The host owns memory: baseline by default, an external provider by configuration.</li>'
  '<li>The calendar is the host\'s, and there is no wake tool.</li>'
  '<li>The arbiter is scored, logged and swappable. The agent may defer, never raise.</li>'
  '<li>The agent never settles its own expectations.</li>'
  '<li>Workers: non-blocking, one level deep, checked by the host, never re-run on a crash.</li>'
  '<li>Stop and watchdog work outside the loop. There is no pause state.</li>'
  '<li>Spending caps only for paid providers.</li>'
  '<li>The seed identity and first projects are the operator\'s configuration, not rung code.</li></ul></div></div>',
  "Recommendations are mine; the owner decides", cls="tight"))
assert len(S) == D.total, len(S)

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"janus-?infra|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|janus-platform|treehouse|firstmate|doppler|\broger\b|captain|spire|venue|animus|continuum|cookie|minicpm|"
                     r"[\w.+-]+@[\w-]+\.[\w.]+|\.(ts|tsx|py|md|sql|rs|json|yml|toml)\b", re.I)


def main():
    D.build("A continuous rung host", "\n".join(S), PRIVATE, "rung-continuous-host-v2.pdf")


if __name__ == "__main__":
    main()
