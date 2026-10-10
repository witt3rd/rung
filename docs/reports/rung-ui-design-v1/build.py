#!/usr/bin/env python3
"""Build "rung-ui: one place to configure, queue and watch several continuous hosts" (v1):
slides -> build/deck.html -> render.mjs -> build/deck.pdf, then a text privacy scan.
The mockup screenshots are made by mockups/capture.mjs (committed in mockups/shots/); this build only embeds them.
Usage: python3 build.py [--publish DIR] [--name FILE]"""
import re, shutil, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_build"))
from common import Deck

HERE = Path(__file__).resolve().parent
D = Deck(HERE, total=20, questions=True)
S = D.slides
svg, foot, slide, q = D.svg, D.foot, D.slide, D.q


def shots(name):
    return f"../mockups/shots/{name}"


def mock(kicker, h1, h2, page, desk_cap, phone_cap, note, src, deskw=1180):
    """A mockup page: the 1920 capture with what looking changed under it, and the 390 capture beside it."""
    return slide(kicker, h1, h2,
      f'''<div style="display:grid; grid-template-columns: 1180px 300px; gap:40px; align-items:start">
<div style="width:{deskw}px"><div class="shot"><img src="{shots(page + "-1920.png")}" alt="{page} at 1920"><div class="cap"><b>1920 wide.</b> {desk_cap}</div></div>
<div class="card" style="margin-top:16px"><p>{note}</p></div></div>
<div class="shot"><img src="{shots(page + "-390.png")}" alt="{page} at 390"><div class="cap"><b>390 wide.</b> {phone_cap}</div></div></div>''', src, cls="tight")


# 1 -------------------------------------------------------------------------
S.append(f'''<section class="slide title short"><div class="kicker">Rung · design, version 1 · nothing is built</div>
<h1>rung-ui: configure, queue and watch several continuous hosts</h1>
<h2>The acceptance scenario comes first. It is the run the owner makes on his phone to accept the whole thing; every later page exists to make it pass.</h2>
<ol>
<li><b>Open.</b> On his phone, over the tailnet, he opens one https address and sees his instances, one row each: a state word, one line on what it is doing, requests used. One row is marked because it needs him.</li>
<li><b>Open one.</b> He taps an instance. What it is doing now is the heaviest thing on the page, and it moves: turn text and tool calls appear as they happen.</li>
<li><b>Ask why.</b> In Turns he finds a decision the host made, and who made it.</li>
<li><b>Queue.</b> He adds a task. It shows under Waiting at once, is taken at the next boundary, and its answer appears in Turns.</li>
<li><b>Configure.</b> He moves one model above another in the ladder and applies. The page says when it takes effect.</li>
<li><b>See the effect.</b> A model-switch line appears, the next turn names the new model, the requests count carries on. No reload.</li>
<li><b>Leave and return.</b> He locks the phone and comes back ten minutes later. The page catches up from where it stopped: nothing missing, nothing twice.</li>
</ol>{foot(1, "Written first, before any design. Names in the mockups (atlas, bramble, cedar) are examples; all mockup data is invented")}</section>''')

# 2 -------------------------------------------------------------------------
S.append(slide("Acceptance scenario · the proofs", "Each step has a check that can fail, and a slice that makes it pass",
  "The checks are written before anything is built. Each is run first against the host as it is, where it must fail, and then after the change, where it must pass.",
  '''<table class="dense topo">
<tr><th style="width:3%">#</th><th style="width:27%">What must be true</th><th style="width:48%">How it is proved</th><th style="width:7%">Slice</th></tr>
<tr><td class="k">1</td><td>The overview lists every registered instance. A stopped or dead one is listed, never missing. The state word comes from the record and the lock, not a guess.</td><td>Three registered instances; one is killed with <b>kill -9</b>. The row reads Down within 30 seconds. Before the lock and registry exist this check cannot even start, so it fails first. The first screen holds at most 60 words of chrome at 390 and at 1920.</td><td>0, 4</td></tr>
<tr><td class="k">2</td><td>A line the host records is on the screen within a second, in order, with the text as the model wrote it.</td><td>A real clock host writes a turn; the record time against the browser's receipt time, median and worst, through the gateway. The first live view is also run against one real free-model hour.</td><td>1</td></tr>
<tr><td class="k">3</td><td>Every decision shows its question, its answer, who made it (rule or model, with cost) and why a rule stepped in.</td><td>Shown for each of the five decision families from a recorded run; a decision with no recorded reason fails the page test.</td><td>0</td></tr>
<tr><td class="k">4</td><td>The add call answers only after the stimulus is on disk. The same call sent twice adds once. A kill right after the answer loses nothing.</td><td>An idempotency key repeated; <b>kill -9</b> straight after the answer, then restart: the item has exactly one disposition (the host's existing restart gate, extended to the new door).</td><td>2</td></tr>
<tr><td class="k">5, 6</td><td>Before applying, the page shows the difference and whether it takes effect now, at the next rollover, or only after a restart. Applying is recorded with who and when.</td><td>Move a ladder rung through the door; a <b>config.changed</b> line appears; the next turn's model matches the new order or a <b>model.switch</b> line explains why not. A restart-only key is refused with the reason and the old value untouched.</td><td>3</td></tr>
<tr><td class="k">7</td><td>Reconnecting with the last seen number replays what was missed once, in order, even across a host restart.</td><td>Cut the connection for 10 minutes while turns run, and restart the host once; compare what the page holds with the record, line by line.</td><td>1</td></tr>
<tr><td class="k">all</td><td>No real credential appears in any answer, stream line or console file.</td><td>A canary key is put in the host's environment and a tool is made to print it. Zero occurrences in every door, the stream and the console files. The check is written first and fails on the host as it stands.</td><td>0 on</td></tr>
</table>''',
  "Proposal. Checks are targets, not measurements; nothing here has been run", cls="tight"))

# 3 -------------------------------------------------------------------------
S.append(slide("The idea", "One small gateway in front of any number of ordinary hosts",
  "An instance is a rung host: one named agent that thinks continuously, with its own state, workspace, inbox, stop file and port. The UI is for those and nothing else.",
  f'''<div class="diagram" style="width:1580px; margin:0 auto">{svg("diagram.svg")}</div>
<div class="cols c3" style="height:auto; gap:22px; margin-top:12px">
<div class="card"><h3><span class="tag mit">Stays</span> as it is</h3><p>Every host runs by itself, with no UI and no gateway. The record stays the one truth. The UI never writes the record; it calls doors, and the host writes.</p></div>
<div class="card"><h3><span class="tag acc">New</span> and part of this design</h3><p>An instance registry, a lock on each state directory, a read API over the record, a live event stream, queue and config doors, a console, one redactor for every door, and the gateway.</p></div>
<div class="card"><h3><span class="tag own">Rule</span> API first</h3><p>Every button is one call that a script can make with a plain HTTP client. Nothing is possible only through the screen. The same doors serve the tests.</p></div></div>''',
  "Proposal. The host's present shape is read from the host design note and its code; everything marked new is absent today", cls="tight"))

# 4 -------------------------------------------------------------------------
S.append(slide("What he looks at and does", "Each screen has one thing to look at and one thing to do",
  "The test: write the two sentences first, then ask of every element whether it serves one of them, would change what he does, and is not already obvious from where he is.",
  '''<table class="dense topo">
<tr><th style="width:11%">Screen</th><th style="width:22%">Look at</th><th style="width:15%">Do</th><th style="width:30%">One reveal away</th><th>Never on the screen</th></tr>
<tr><td class="k">Instances</td><td>One row each: state word, one line of what it is doing, requests used. The row that needs him is marked.</td><td>Open the marked row, or nothing: read</td><td>Model, spend, epoch, the instance's history</td><td>Charts. A count that changes nothing. "All is well" banners</td></tr>
<tr><td class="k">One instance, Now</td><td>What it is doing now, live. Committed project and its done-when, or free time.</td><td>Add to queue (the one filled button)</td><td>Next (schedule, queue), Decided with a Why link, Health: context, cache, spend; recent turns, paged</td><td>Ids, hashes, token counts, raw JSON</td></tr>
<tr><td class="k">Turns</td><td>The newest turn as it happens, text as written, tool calls as they run</td><td>Nothing: read. Search and filter are quiet</td><td>A turn's calls, cost, cache, decisions and who made them; the exact messages; the record line</td><td>A cap on how far back it goes. Nothing is cut: older turns are paged</td></tr>
<tr><td class="k">Queue</td><td>What is being answered now; the next waiting item</td><td>Add (filled). Move and cancel are quiet row actions</td><td>Why an item waits (the decision, in words)</td><td>Kinds of item the host does not have</td></tr>
<tr><td class="k">Configure</td><td>The model ladder, the rung in use marked</td><td>Apply N changes (always present; filled when something changed)</td><td>When each change takes effect; schedule, free time, budget, sandbox</td><td>Keys that need a restart, presented as editable</td></tr>
<tr><td class="k">Console</td><td>The tail of host, agent and tool output, following</td><td>Search</td><td>Filter by source; page earlier; one long line in whole</td><td>A real credential, ever</td></tr>
</table>
<p class="note" style="margin-top:14px">Colour is attention only. <b>Amber</b>: he must act. <b>Red</b>: something failed. <b>One accent</b>: the one filled control and the focus ring. Everything else is ink and grey. State is a word first, never a colour alone. At most six tabs, with Now first.</p>''',
  "Proposal, adapted from the owner's earlier screen-design rules and measured against his own calm-screen test; no screen is built", cls="tight"))

# 5-10 mockups --------------------------------------------------------------
S.append(mock("Instances", "The overview: three rows and one of them is marked",
  "Look at: the marked row. Do: open it, or nothing. No chart, no banner, no filled button.",
  "index", "Full width, no centred column. The marked row uses amber, a dot and the word Stuck: the only colour on the page.",
  "The word and the dot say it; colour is not alone.",
  "<b>Looking changed one thing.</b> The first draft had an \u201cAll systems reachable\u201d line in the top bar. It said nothing he could act on, so it was removed. On the phone each row becomes three short lines, with the whole row as the tap target. The list says it shows all three, once, quietly.",
  "Mockup with invented data, captured with Playwright at 1920 and 390; census run on this page"))

S.append(mock("One instance", "Now: what it is doing, live, and one button",
  "Look at: the Doing now card. Do: Add to queue. Next, Decided and Health sit beside it and say little.",
  "instance", "The card has the heaviest border; the one filled control is the button. Why opens the decision.",
  "The button takes the full width and the tabs fit without clipping.",
  "<b>Looking changed three things.</b> A Spend section was folded into Health (the first draft counted 73 words of chrome, over the budget of 60). \u201cTurn 214\u201d was said twice, in the line and in the card; it is now said once. \u201cBy rule, 12:04\u201d became a Why link. Context and cache are one line; spend is the quiet line under it.",
  "Mockup with invented data; census 60 words at 1920 (budget 60), 46 at 390"))

S.append(mock("Observe · Turns", "Turns: watch it think, then page back as far as he wants",
  "Look at: the newest turn, in progress at the top. Do: nothing, read. Search and filter are quiet.",
  "turns", "A turn shows the text as the model wrote it, then each tool call; a decision shows who made it. Following means new turns arrive at the top.",
  "Same anatomy. Long text wraps in whole; nothing is cut.",
  "<b>Looking changed one thing.</b> The first draft said \u201cFollowing. New turns appear at the top.\u201d and put calls and cache hit on every turn (67 words). The state is now one word, and calls and cache sit in the fold. Paging is a view, never a cap: <i>Show 50 more</i>, <i>Show all 214</i>.",
  "Mockup with invented data; census 39 words at 1920, 28 at 390"))

S.append(mock("Queue", "Queue: add, reorder, cancel, with what it is doing kept apart",
  "Look at: what is being answered now and the next waiting item. Do: Add. A message is answered; a task may be committed to; a pull sits on its shelf for free time.",
  "queue", "Three kinds in one list structure: Message, Task, Pull. The columns on the right line up across rows.",
  "Move up, move down and Cancel are 44-point targets.",
  "<b>Looking changed one thing.</b> At 1920 the right-hand columns did not line up from row to row, so the eye could not scan the ages. They are now fixed-width. Cancel on the item being answered is not offered as a quiet action: it needs a decision card, because it cuts the running turn.",
  "Mockup with invented data; census 51 words at 1920, 45 at 390"))

S.append(mock("Configure", "Configure: the ladder first, and one button that says what it does",
  "Look at: the model ladder with the rung in use marked. Do: Apply 1 change. The line above the button says when it takes effect.",
  "configure", "The ladder is one decision, so its options sit together and in order. Schedule, free time, budget and sandbox follow.",
  "Long model names wrap in whole.",
  "<b>Looking changed two things.</b> The first draft explained the ladder in a sentence and listed the tool groups; at 1920 that made 86 words of chrome. Both were cut from the page; the ladder explanation is to return as a one-click reveal. The apply sentence went from two sentences to one. The sandbox path is shown, not editable: it needs a restart.",
  "Mockup with invented data; census 54 words at 1920, 44 at 390", deskw=1020))

S.append(mock("Observe · Console", "Console: everything the host and its tools print, with secrets removed in code",
  "Look at: the tail, following. Do: search. Lines are whole; long ones fold in place; nothing is cut.",
  "console", "Time, source, text. A secret reads [redacted] before it is stored and again before it is sent.",
  "A line wraps; a very long one folds with Show whole line.",
  "<b>Looking changed one thing.</b> In the first phone capture the last tab, Configure, was cut off to \u201cCon\u201d. The census did not catch it; looking did. The tabs now take less padding at 390, and the census gained a check that the active tab is fully visible. Console lines are paged: Show 500 earlier, or Show all.",
  "Mockup with invented data; census 32 words at 1920, 30 at 390"))

# 11 ------------------------------------------------------------------------
S.append(slide("How the screens were judged", "Criteria first, a census written before the pages, then looking at every capture",
  "The census counts what a script can. It is a flag, not a verdict, so every capture was also looked at, at both widths.",
  '''<div class="cols c2" style="height:auto; gap:30px">
<div class="stack">
<div class="card"><h3>The rules, written before the mockups</h3><ul>
<li><b>One look, one do</b> for every screen, written as two sentences.</li>
<li><b>Colour is attention</b>: amber for a human decision, red for failure, one accent for the one filled control.</li>
<li><b>One decision card</b> at a time, when something truly needs him.</li>
<li><b>Nothing appears or disappears after load.</b> Empty and loading regions keep their place and their heading.</li>
<li><b>Folds, not truncation. Paging, not caps.</b> Every list says how many and offers more or all.</li>
<li><b>Plain words.</b> Say <i>queue</i>, <i>turn</i>, <i>ladder</i>; no ids or hashes in the main view.</li>
<li><b>Each fact once.</b> Four text sizes at most. No key hints drawn on buttons.</li>
<li><b>Looked at</b> at 390 and at 1920 before it counts as done, and <b>proved live</b> against a running host.</li></ul></div>
<p class="note">Fail first: the budgets below were written before the pages. The first draft failed them on 3 pages at 1920 and 1 at 390; the fixes were made, and the census then passed on all 12 captures.</p></div>
<div class="card"><h3>The census, first draft and now</h3>
<table class="dense">
<tr><th>Page</th><th>Words at 1920</th><th>Words at 390</th></tr>
<tr><td class="k">Instances</td><td>42 → 39</td><td>42 → 39</td></tr>
<tr><td class="k">Now</td><td><span class="bad">73</span> → 60</td><td>55 → 46</td></tr>
<tr><td class="k">Turns</td><td><span class="bad">67</span> → 39</td><td>56 → 28</td></tr>
<tr><td class="k">Queue</td><td>55 → 51</td><td>49 → 45</td></tr>
<tr><td class="k">Configure</td><td><span class="bad">86</span> → 54</td><td><span class="bad">62</span> → 44</td></tr>
<tr><td class="k">Console</td><td>36 → 32</td><td>34 → 30</td></tr>
</table>
<p class="note" style="margin-top:10px"><b>Budgets, per page, first screen.</b> Words of chrome (not counting what he came to read) at most 60. One filled control. At most 4 text sizes. No sentence said twice. No key hints. No text under 14 pixels. One level-one heading. At 390: 44-point tap targets, no sideways scroll, active tab fully visible.</p>
<p class="note"><b>Found by eye, not by census:</b> a status line that said nothing; a fact said twice in different words; a clipped tab. The first two cannot be counted; the third now can.</p></div></div>''',
  "Measured on the six mockup pages by mockups/census.mjs; the first-draft output is committed beside the pages", cls="tight"))

# 12 ------------------------------------------------------------------------
S.append(slide("The host today", "What the UI needs, what the host has, and the piece that closes each gap",
  "Read from the host's design note and its code. The host runs one agent per process; the record is its only log.",
  '''<table class="dense topo">
<tr><th style="width:13%">The UI needs</th><th style="width:44%">The host today <span class="meta" style="font-weight:400;text-transform:none">(read in the code and design note)</span></th><th>The piece that closes it</th></tr>
<tr><td class="k">List the instances</td><td>Nothing lists them. Each instance exists only as a separate config: its own state, workspace, inbox, stop file and ACP port.</td><td><b>H2</b> registry: one small file per instance, written at start, kept after stop</td></tr>
<tr><td class="k">Two never share a state</td><td>No lock. A second host started on the same state directory would write into the same record.</td><td><b>H1</b> a lock held for life on the state directory; a clean refusal naming the holder</td></tr>
<tr><td class="k">Is it alive, and doing what</td><td>An ACP status call (turn, mode, project, epoch, model, ladder, quota, degraded, pending) and a plain-text report, over ACP only; the process pings a watchdog.</td><td><b>H3</b> a summary door over HTTP with one state word</td></tr>
<tr><td class="k">Read the past</td><td>The NDJSON record on disk, one line per event, gap-free numbers. No read API. The turn text and messages are in it verbatim.</td><td><b>H3</b> a paged read door and turn, decision, pack and spend folds</td></tr>
<tr><td class="k">Watch live</td><td>An internal hook feeds the ACP bridge. An observer channel sees only the output of work it owns. Nothing streams record lines. A model call is recorded when it ends.</td><td><b>H5</b> an event stream; <b>H5b</b> short-lived deltas, if he wants them (question 4)</td></tr>
<tr><td class="k">Queue</td><td>A directory of message files, an ACP stimulus call (on disk before its answer), cancel that withdraws a waiting prompt, and an owner calendar call. Status counts waiting items but does not list them. No order to change.</td><td><b>H6</b> list, add, cancel and move doors</td></tr>
<tr><td class="k">Configure</td><td>A YAML file read once at start; unknown fields refused; the ladder is configuration, re-listed every six hours. No way to change a running host.</td><td><b>H7</b> validate and apply doors with effect classes, recorded</td></tr>
<tr><td class="k">A console</td><td>No such thing. The host's own output goes to the system journal; the record holds no process output.</td><td><b>H8</b> a console sink and doors</td></tr>
<tr><td class="k">No credentials out</td><td>A pattern redactor exists and is used for short gists and memory. The record is verbatim on purpose, so the cached prompt prefix can be rebuilt byte for byte.</td><td><b>H4</b> one redactor at every door and in the console sink</td></tr>
<tr><td class="k">Metrics, MCP</td><td>None. They are not needed: the record is the log, and the UI reads it.</td><td>No piece. Not added</td></tr>
</table>''',
  "Verified by reading rung-host's design note and source; the registry, lock, read API, stream, queue list and config doors do not exist", cls="tight"))

# 13 ------------------------------------------------------------------------
S.append(slide("The Host API", "Every button is one call; these are the doors",
  "All on the host's own HTTP listener, next to ACP, behind the same bearer tokens. Reads need a read-only token or the owner's; writes need the owner's.",
  '''<table class="dense topo">
<tr><th style="width:9%">Group</th><th style="width:25%">Call</th><th style="width:40%">What it does</th><th>Today</th></tr>
<tr><td class="k" rowspan="2">Who</td><td>GET /v1/summary</td><td>The overview row: id, name, state word, one line of what it is doing, requests used and spend this UTC day, what needs the owner</td><td>new</td></tr>
<tr><td>GET /v1/status · /v1/report</td><td>The ACP status as JSON; the plain-text report</td><td>exists over ACP</td></tr>
<tr><td class="k" rowspan="5">Read</td><td>GET /v1/record</td><td>Lines, paged: offset, limit, total, next. Filter by kind, turn, text. Newest first or oldest first</td><td>new</td></tr>
<tr><td>GET /v1/turns · /v1/turns/{n}</td><td>One summary per turn; one turn in full with its messages, calls, decisions</td><td>new</td></tr>
<tr><td>GET /v1/decisions</td><td>The decision desk's records by family, with who decided and why</td><td>new</td></tr>
<tr><td>GET /v1/pack</td><td>Epoch, context against its budget, rollovers and their cause, cache hit and efficiency, cache breaks</td><td>new</td></tr>
<tr><td>GET /v1/spend</td><td>Requests, tokens and cost by UTC day and by model</td><td>new</td></tr>
<tr><td class="k">Live</td><td>GET /v1/events?after=N</td><td>Server-sent events: each record line as it is written, numbered; resumes from N or Last-Event-ID; short-lived deltas</td><td>new</td></tr>
<tr><td class="k" rowspan="2">Queue</td><td>GET · POST /v1/queue</td><td>List (being answered, waiting, shelf, schedule); add a message, task, pull or calendar entry, with an idempotency key</td><td>parts exist</td></tr>
<tr><td>DELETE /v1/queue/{id} · POST …/move</td><td>Cancel; move before another item</td><td>cancel exists; move new</td></tr>
<tr><td class="k" rowspan="2">Config</td><td>GET /v1/config</td><td>The effective config, secrets as the name of the variable that holds them, each key with its effect class</td><td>new</td></tr>
<tr><td>POST …/validate · PUT /v1/config</td><td>The difference and class for a change; apply it, recorded as who, when, old, new</td><td>new</td></tr>
<tr><td class="k">Console</td><td>GET /v1/console · …/stream</td><td>Paged and live process output; search; source filter</td><td>new</td></tr>
<tr><td class="k">Control</td><td>POST /v1/stop · /v1/release</td><td>The owner's stop and release</td><td>exist over ACP</td></tr>
</table>
<p class="note" style="margin-top:12px">The gateway adds two more: <b>GET /api/instances</b> (the registry and each summary) and <b>/api/i/{id}/v1/…</b> (a pass-through that adds that instance's key and streams events through). Lists follow one paging rule: <b>offset, limit, total, next</b>; limit has no maximum; total is exact.</p>''',
  "Proposal. Paths are names for the pieces, not final; the set must be fixed in the first host pull request", cls="tight"))

# 14 ------------------------------------------------------------------------
S.append(slide("Watching live", "The record is the event log; the stream is the record, followed",
  "A stable, resumable stream needs one durable source. The host already has it: an append-only, gap-free, fsynced record.",
  '''<div class="cols c3" style="height:auto; gap:22px">
<div class="card"><h3>Server-sent events, not a socket</h3><ul>
<li>One direction is all the screen needs; every command is an ordinary call.</li>
<li>Each event carries its record number as its id. A reconnect sends the last one it saw, and the host replays from the record. A host restart changes nothing.</li>
<li>Passes a tailnet https proxy untouched. A socket can be added later if something needs two-way traffic.</li>
<li>A slow reader is dropped and reconnects; nothing is lost because the record holds it all.</li>
<li>A comment line every 15 seconds keeps the path open.</li></ul></div>
<div class="card"><h3>What is stable</h3><ul>
<li>The <b>kinds and fields</b> of record lines are the contract. They are listed in one table in code, the design note is generated from it, and a test fails if they drift.</li>
<li>Changes inside version 1 only add. A removed or renamed field needs version 2.</li>
<li>Short-lived <b>delta</b> events carry the text and tool progress of the turn that is running. They have no number, are never replayed and never enter the record, so the record stays reproducible (question 4).</li>
<li>The host's start line states the contract version.</li></ul></div>
<div class="card"><h3>Credentials stay in</h3><ul>
<li><b>One redactor, three places</b>: the record door, the stream, and the console sink. It removes the exact value of every key the config names by variable, plus the known key shapes already recognised for memory.</li>
<li>It replaces; it never shortens or drops. No length cap anywhere.</li>
<li>The record on disk stays verbatim, because the cached prompt must be rebuilt byte for byte. The door redacts on the way out.</li>
<li>Written first as a failing test: a canary in the environment, a tool that prints it, zero hits across every door and file.</li></ul></div></div>
<div class="card" style="margin-top:18px"><h3>The console is a second, smaller log</h3><p>The record has no process output, so a console sink writes its own numbered lines: <b>host</b> (the host's own log lines), <b>agent</b> (the model route's request and answer lines) and <b>tool</b> (what a tool process printed, as it prints). It is redacted before it is written, because nothing needs it verbatim. Same paging, search and stream as the record. A very long line is stored whole and folded in the screen.</p></div>''',
  "Proposal; built on the host's present record. Record kinds and fields are from the host design note", cls="tight"))

# 15 ------------------------------------------------------------------------
S.append(slide("Several instances", "A registry file and a lock per instance; the state word is derived, never typed",
  "The overview has to be true after a crash, so it never trusts what an instance says about itself: it joins the record with whether the lock is held.",
  '''<div class="cols" style="grid-template-columns: 1fr 1.25fr; height:auto; gap:26px">
<div class="stack">
<div class="card"><h3>The registry</h3><ul>
<li>One small file per instance in a registry folder under the rung home. Fields: id, name, state directory, config path, workspace, address, process number, started, version.</li>
<li>Written when the host starts and kept when it stops, so a stopped instance is still listed.</li>
<li>The host binds its address on any free port and writes the real one, so ports never collide by hand.</li>
<li>A command lists them (<b>rung-host ls</b>); the gateway reads the folder.</li></ul></div>
<div class="card"><h3>The lock</h3><ul>
<li>A lock file in the state directory, held by the process for as long as it lives.</li>
<li>A second start on the same state refuses, exits with the code the host already uses for a bad start, and names the holder. The check is written first: today the second start succeeds.</li>
<li>Alive means the lock is held. A dead host drops it by dying, so a crash cannot leave a false Working.</li></ul></div>
<div class="card"><h3>Start and stop</h3><p>Stop and release go through the doors the host already has. Starting is the system's job: one templated user unit, one instance per name. A stopped row shows the command to start it. A supervisor in rung is a question for the owner (question 2).</p></div></div>
<div class="card"><h3>The seven state words</h3>
<table class="dense">
<tr><th style="width:15%">Word</th><th>When</th><th style="width:14%">Colour</th></tr>
<tr><td class="k">Working</td><td>The lock is held and the agent is committed to a project</td><td>none</td></tr>
<tr><td class="k">Answering</td><td>The lock is held and the turn answers a waiting item</td><td>none</td></tr>
<tr><td class="k">Free time</td><td>The lock is held and nothing pulls it. If its last three free turns made no tool call, the line says it is idle</td><td>none</td></tr>
<tr><td class="k">Waiting</td><td>Slowed by the world: pacing, quota or a provider backoff. The line says until when</td><td>none</td></tr>
<tr><td class="k">Stuck</td><td>A refused key, a copy loop the host had to break, a commitment with no progress for a set number of turns, a turn past its bound, or a missed watchdog</td><td><b class="needs-text" style="color:#985800">amber</b></td></tr>
<tr><td class="k">Down</td><td>The lock is free and the last record line is not a halt: it died</td><td><b style="color:#985800">amber</b></td></tr>
<tr><td class="k">Stopped</td><td>The lock is free and the last line is a halt</td><td>none</td></tr>
</table>
<p class="note" style="margin-top:10px"><b>Needs him</b> is one more mark on a row: a message from the agent to its owner not yet read, or Stuck, or Down, or a spend cap reached. Nothing else raises an alert. Free-model hosts show requests used of the day's quota beside spend, because on the free ladder the quota, not money, is the meter. The thresholds are settings.</p></div></div>''',
  "Proposal. The state words are derived from record kinds and the lock; thresholds are inferred and to be tuned on live runs", cls="tight"))

# 16 ------------------------------------------------------------------------
S.append(slide("Stack, serving, testing", "A directory in the rung repo, built like a normal web app, proved with real captures",
  "Small and boring on purpose. The same pull request can change a door and the screen that uses it.",
  '''<div class="cols c2" style="height:auto; gap:24px">
<div class="card"><h3>Where it lives</h3><ul>
<li>A <b>rung-ui</b> directory in the rung repo: the app, its tokens and its tests. A small <b>gateway</b> crate beside the host crates, not published.</li>
<li>Both are product, not kernel: nothing in the ladder crates changes, and no consumer is named anywhere (the repo's guard already checks this).</li>
<li>Why not its own repo: the door contract, its gates and the screens move together, and the docs and guard checks already run here. The cost is a second toolchain in CI (question 1).</li></ul></div>
<div class="card"><h3>What it is built with</h3><ul>
<li>React, TypeScript and Vite; a query cache for the reads, and a small hook that merges the live stream into it by record number.</li>
<li><b>Design tokens as plain CSS variables</b>, rung's own copy. The stylesheet behind these mockups is the seed: four text sizes, ink and grey, one accent, amber and red held back.</li>
<li>No component kit at first. Dialog and select primitives only if a screen needs them.</li>
<li>Reading measure for prose; lists and tables use the full width.</li></ul></div>
<div class="card"><h3>Serving</h3><ul>
<li>The gateway listens on loopback and embeds the built app. Tailscale's own serve command puts it on tailnet https, so the phone opens one address.</li>
<li>One systemd user unit. It reads the registry folder and a tiny file naming which variable holds each instance's key. Keys never reach the browser.</li>
<li>No login in the prototype; the tailnet is the boundary (question 6).</li></ul></div>
<div class="card"><h3>Testing</h3><ul>
<li>Unit tests for the folds, run on the recorded runs already in the repo.</li>
<li><b>Playwright captures at 390 and 1920 of every screen</b>, against two or three real hosts on a real clock. A capture is looked at before a slice is done.</li>
<li>The <b>census</b> from this note becomes a CI gate. A new screen lands with its budget and a recorded first failure.</li>
<li>Every slice ends with a live proof (the next pages), never with a green mock.</li></ul></div></div>''',
  "Proposal; choices follow the owner's earlier web work, with rung-owned copies. Nothing is imported from another repository", cls="tight"))

# 17 ------------------------------------------------------------------------
S.append(slide("Slices", "Five slices, each ending in something he can use",
  "Effort is a coding agent's days, rough, one host builder and one UI builder working side by side. Each slice is judged by a live proof, then by looking at its captures.",
  '''<table class="dense topo">
<tr><th style="width:11%">Slice</th><th style="width:21%">The loop it closes</th><th style="width:20%">Host pieces</th><th style="width:15%">UI</th><th style="width:23%">Live proof</th><th>Effort</th></tr>
<tr><td class="k">0 · Observer on a recorded log</td><td>Read-only. He opens the instances recorded on disk, reads turns, decisions, context and spend, and cannot change anything.</td><td>H3 read doors and folds, H4 redactor, H9 gateway</td><td>Scaffold, tokens, census; Instances, Now, Turns</td><td>The repo's recorded live runs open in the page; the canary check passes; captures at 390 and 1920</td><td>Host 10, UI 8</td></tr>
<tr><td class="k">1 · Live instance view</td><td>He opens a running instance and watches it think.</td><td>H1 lock, H2 registry, H5 events, H8 console</td><td>Live Now, Turns following, Console, state words</td><td>Two real-clock hosts; record time to screen time; reconnect and replay; one real free-model hour</td><td>Host 11, UI 5</td></tr>
<tr><td class="k">2 · Queue</td><td>He adds, moves and cancels from the phone.</td><td>H6 queue doors</td><td>Queue tab, Add to queue</td><td>Add, answered in Turns; the same call twice adds once; kill after the answer loses nothing</td><td>Host 5, UI 3</td></tr>
<tr><td class="k">3 · Configure</td><td>He changes the model ladder and sees the effect.</td><td>H7 config doors and overrides</td><td>Configure tab</td><td>Move a rung, see config.changed, then the next turn on the new model, live</td><td>Host 7, UI 4</td></tr>
<tr><td class="k">4 · Side by side</td><td>Three instances at a glance, tailnet https, the phone captures.</td><td>Thresholds, needs-him marks, unit template</td><td>Overview polish, alerts</td><td>Three hosts, one killed: Down inside 30 seconds; the phone scenario run once end to end</td><td>Host 2, UI 3, ops 1</td></tr>
</table>
<div class="cols c2" style="height:auto; gap:22px; margin-top:16px">
<div class="card"><h3>Who builds</h3><p>A host builder (Rust, in the host and gateway crates) and a UI builder (TypeScript, in rung-ui), each on their own branch and pull request through the checking pipeline, one slice at a time. The design owner reviews the captures before a slice is called done.</p></div>
<div class="card"><h3>Order</h3><p>Slice 0 first and alone on the UI side. H1, H2 and H3 do not depend on any question and can start in parallel with it. Slices 1 to 4 follow in order; 2 and 3 can overlap once slice 1 is done. About 59 agent-days in all.</p></div></div>''',
  "Proposal. Efforts are estimates, not measurements", cls="tight"))

# 18 ------------------------------------------------------------------------
S.append(slide("Host pieces", "Nine plain-engineering pieces, most of them independent",
  "Each is a small pull request on its own, with its check written first. None needs the screen.",
  '''<table class="dense topo">
<tr><th style="width:5%">#</th><th style="width:19%">Piece</th><th style="width:36%">Done when (the check that fails first)</th><th style="width:8%">Days</th><th style="width:12%">Needs</th><th>Can start now</th></tr>
<tr><td class="k">H1</td><td>State-directory lock</td><td>A second host on the same state exits with the bad-start code, names the holder, and leaves the record's last line unchanged</td><td>1</td><td>nothing</td><td><b class="good">yes</b></td></tr>
<tr><td class="k">H2</td><td>Registry and <b>rung-host ls</b></td><td>Start writes the file, stop keeps it, a killed host reads Down; ls shows all three states</td><td>2</td><td>H1</td><td><b class="good">yes</b></td></tr>
<tr><td class="k">H3</td><td>Read doors and folds</td><td>Turn, decision, pack and spend folds of a recorded run equal what the live host computes at each turn end (the host already hashes its projection); paging totals exact</td><td>5</td><td>nothing</td><td><b class="good">yes</b></td></tr>
<tr><td class="k">H4</td><td>Redactor</td><td>The canary check: zero hits in every door, stream line and console file; no output shorter than its input except by the replaced value</td><td>2</td><td>nothing</td><td><b class="good">yes</b></td></tr>
<tr><td class="k">H9</td><td>Gateway</td><td>Serves the app, passes /v1 and streams through, adds the key, lists the registry; no key in any page or response</td><td>3</td><td>H3 shape</td><td><b class="good">yes</b></td></tr>
<tr><td class="k">H5</td><td>Event stream</td><td>Replay after a cut equals the record; a slow reader does not slow the host; the first line reaches a client within a second of being written. Needs the HTTP listener to carry routes beside ACP</td><td>4 (+3 for deltas)</td><td>H3; question 4</td><td>after the answer</td></tr>
<tr><td class="k">H6</td><td>Queue doors</td><td>List matches the host's waiting items; add on disk before the answer; idempotent; cancel withdraws; move recorded and respected, the desk's limits still winning</td><td>5</td><td>H3; question 3</td><td>after the answer</td></tr>
<tr><td class="k">H7</td><td>Config doors</td><td>Validate equals startup validation; a hot key changes the running host at the next boundary; a restart-only key is refused; base file untouched</td><td>7</td><td>H3; question 5</td><td>after the answer</td></tr>
<tr><td class="k">H8</td><td>Console sink</td><td>Host, agent and tool output numbered and redacted before written; a 12 MB tool output stored whole and paged; search and follow work</td><td>4</td><td>H4</td><td>with slice 1</td></tr>
</table>''',
  "Proposal. The host's existing restart, stop and record gates are the pattern for each check", cls="tight"))

# 19, 20 --------------------------------------------------------------------
S.append(slide("Calls for the owner", "Six questions, most blocking first",
  "Default if unanswered: only slice 0, the read-only observer, and the host pieces that no question touches (H1 to H4, H9) are built. Nothing live, queued or configurable is built until he answers.",
  '<div class="cols" style="grid-template-columns: 1fr 1fr; height:auto; gap:22px"><div class="stack">' +
  q(1, "Where does rung-ui live?", "In the rung repo, or in its own repository?",
    "In the rung repo, as a directory and a gateway crate. The door contract, its checks and the screens change in one pull request.",
    "A second toolchain in CI and a mixed-language repo, against two repos that must agree on a contract by copy.") +
  q(2, "Who starts and stops instances?", "Should the page only stop and release, with starting left to the system's unit manager?",
    "Yes for now: one templated unit, and the page shows the command to start a stopped one. No supervisor in rung yet.",
    "Whether the page ever has a Start or Create instance button. A supervisor adds process management and its failure modes.") +
  q(3, "What does moving a queue item mean?", "May the owner's move override the decision desk?",
    "No. A move orders items waiting for the same boundary; owner items still go first; the desk's limits still win, and the page says so.",
    "The admit code and a new check. If a move could override everything, one tap could starve a commitment.") +
  '</div><div class="stack">' +
  q(4, "How live is live?", "Stream the model's partial text and tool progress as it is written (short-lived, never recorded), or show only finished calls?",
    "Stream partial text. Watching it think is the most important thing he asked for. The record keeps only the finished text, so replays stay exact.",
    "The engine adapter must stream, which it does not today: about 3 more days. He then sees the model's text as it forms. Finished calls only is simpler, with a few seconds' lag.") +
  q(5, "How are config changes stored?", "Edit the base config file, or layer an overrides file over it?",
    "An overrides file in the state directory, applied over the base. The base file, which fleet tooling places, never drifts, and a value set in the page shows as set there, with a way back.",
    "Where the truth for a changed value lives, and whether the page and the config manager can fight.") +
  q(6, "Who may reach it?", "Is the tailnet the only boundary, with no login in the app and keys held in the gateway?",
    "Yes, plus a read-only token role so a view-only link is possible later. Prototype posture: protect real credentials and data, nothing more.",
    "Anyone on the tailnet can queue and configure. A login would add a user store and sessions.") +
  '</div></div>',
  "Recommendations are mine; the owner decides. Nothing beyond slice 0 and the independent pieces is built before he does", cls="tight"))

S.append(slide("How this was made", "What was read, what was measured, and what is still a guess",
  "A design note, so most of it is proposal. These lines say which parts rest on evidence.",
  '''<div class="cols c3" style="height:auto; gap:22px">
<div class="card"><h3><span class="tag mit">Verified</span> read in the code or notes</h3><ul>
<li>The host's loop, record kinds, ACP calls, status fields, ladder and listing, governor, pack and desk, as in its design note and source.</li>
<li>The host has no registry, no state lock, no read API over HTTP, no record stream, no queue list or order, no config door and no console.</li>
<li>The ACP HTTP listener already serves server-sent streams for ACP.</li>
<li>The record is numbered without gaps, written before use, and recovers from a torn last line.</li></ul></div>
<div class="card"><h3><span class="tag acc">Measured</span> by running</h3><ul>
<li>Six real HTML mockups, captured with Playwright at 390 and 1920 (12 images), each looked at.</li>
<li>The census, run on all 12 captures: first draft, then final. Both outputs are committed.</li>
<li>Nothing else. No host was changed or run for this note.</li></ul></div>
<div class="card"><h3><span class="tag svc">Unknown</span> or inferred</h3><ul>
<li>Latency of a stream through the tailnet proxy to a phone.</li>
<li>Which config keys can change on a running host without a restart; the classes here are read from the code, not tried.</li>
<li>The cost of searching a very large record; an index may be needed.</li>
<li>Whether the engine adapter can stream partial text (question 4).</li>
<li>The state-word thresholds, and what the agent's own messages to its owner look like in practice.</li>
<li>Whether free-model quota, rather than cost, should be the headline meter.</li></ul></div></div>
<p class="note" style="margin-top:18px">Everything for this note lives in <b>docs/reports/rung-ui-design-v1/</b>: the build recipe, the deck source, the mockup pages and their capture and census scripts, the 12 captures and both census outputs. The mockups are static pages with invented data, not the app.</p>''',
  "Design scout: no host change, no model call", cls="tight"))

assert len(S) == D.total, len(S)

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"janus-?infra|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|janus-platform|treehouse|firstmate|doppler|\broger\b|captain|spire|venue|animus|hermes|"
                     r"\bcookie\b|\bforge\b|\baugur\b|\bdonald\b|thompson|continuum|minicpm|"
                     r"[\w.+-]+@[\w-]+\.[\w.]+|\.(ts|tsx|py|md|sql|rs|json|yml|toml)\b", re.I)


def main():
    D.build("rung-ui: configure, queue and watch several continuous hosts", "\n".join(S), PRIVATE, "rung-ui-design-v1.pdf")


if __name__ == "__main__":
    main()
