#!/usr/bin/env python3
"""Build "A continuous rung host" (v3): slides -> build/deck.html -> render.mjs -> build/deck.pdf, then a text privacy scan.
Same toolchain as the choir-coordination PDF. Usage: python3 build.py [--publish DIR] [--name FILE]"""
import re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "_build"))
from common import Deck

HERE = Path(__file__).resolve().parent
D = Deck(HERE, total=12, questions=True)
S = D.slides
svg, foot, slide, q = D.svg, D.foot, D.slide, D.q


S.append(f'''<section class="slide title short"><div class="kicker">Rung · design, version 3 · read-only, nothing built</div>
<h1>A continuous rung host, version 3</h1>
<h2>The agent owns its own time. A small decision model, Jev, runs the host's machinery. The context is built so a provider's prompt cache can reuse almost all of it.</h2>
<ol>
<li><b>No more host-side drives.</b> Version 2 scored the agent's pulls and picked one. That is gone. <b>Free time is the default</b>: when nothing has arrived and nothing is committed, the agent decides what to do, and it may commit to a project.</li>
<li><b>Jev runs the machinery.</b> Typed questions decide whether an item interrupts, what to inject, which tools are on, and when to roll the context over and save memory. Each decision has a fixed rule behind it. Cost: about a hundredth of a cent per turn.</li>
<li><b>Context is a cache design.</b> Stable parts first, slow parts next, then a log that is only ever appended to. Tools stay defined and are switched on or off by the host.</li>
<li><b>Free models first.</b> The named stealth model exists and is free, but it expires in two days. A ladder of free models follows it, and on free models the request quota per day, not money, sets the pace.</li>
<li><b>One agent first.</b> Workers and delegation come last, as an extension. Three questions for the owner are on the last page.</li>
</ol>{foot(1, "Design scout: no code, no model call; public catalogue and documentation reads only. Verified = read by me; claimed = a document says so; inferred = mine")}</section>''')

S.append(slide("The idea", "One loop, three modes, and the agent picks what to do with its time",
  "Jev decides how the host behaves at each boundary. The agent decides what it wants. The loop still never rests.",
  f'''<div class="diagram" style="width:1580px; margin:0 auto">{svg("diagram.svg")}</div>
<div class="card" style="margin-top:14px"><h3><span class="tag acc">Changed</span> from version 2</h3>
<p>The scored pulls, the ballot, fairness floors and forced integrity turns are <b>dropped</b> · free time and the agent's own commit and release are <b>new</b> · Jev now makes the host's mechanical decisions · the context order is <b>rebuilt</b> for caching: version 2 put the changing status block near the top and slid a window of recent turns, which spoiled the cache every turn · the first live run uses <b>free hosted models</b> · workers move to a <b>final extension</b> · two new pass-or-fail gates: cache discipline and decisions.</p></div>''',
  "Proposal. Kept from version 2: no resting state, stop only from outside, the record, memory, time as context, the outward protocol", cls="tight"))

S.append(slide("Free time", "When nothing is waiting, the agent's time is its own",
  "An operating system runs an idle process when nothing else is ready. Here the idle process is free time, and what fills it is the agent's choice.",
  f'''<div class="cols" style="grid-template-columns: 1fr 1fr; height:auto; gap:28px">
<div class="stack">
<div class="diagram" style="width:100%">{svg("modes.svg")}</div>
<div class="card"><h3>The agent's own tools</h3><ul>
<li><b>commit</b> to a project, saying when it is done. Its turns then continue that project.</li>
<li><b>progress</b> records the next step, shown in the next turn's header.</li>
<li><b>release</b> with an outcome: done, paused or abandoned.</li>
<li><b>trace</b> ends a stretch of free time: what pulled it, where it went, what is still open.</li>
<li>The host never times out a commitment. It states facts, such as "no progress for 12 turns", and never advises.</li></ul></div></div>
<div class="stack">
<div class="card"><h3>What the host shows in free time</h3><ul>
<li>Once, in the stable part of the context: the free-time rules, adapted from a reference agent's curiosity practice. No obligation. It has to be your own pull. Follow it. Leave a trace. Keep what you found.</li>
<li>Each turn, a short header: the time, how long since anything arrived, the quota left, and the agent's own material: todo, projects, open questions, integrity facts, recent traces.</li>
<li>The material is <b>listed by creation date, never ranked</b>. A test checks that no score can change the order.</li></ul></div>
<div class="card"><h3>What interrupts it</h3><ul>
<li>Jev judges whether a waiting item is worth interrupting the current mode now, later, or as a one-line note.</li>
<li>Hard limits: an owner message always gets through. Each kind of item has a maximum wait. A commitment is interrupted at most six times an hour.</li>
<li>If the agent starts copying itself, the host rolls the context over early and tells the owner. It changes the context, never the topic.</li></ul></div>
<p class="note"><span class="tag acc">First project</span> The owner seeds one project: build its own anticipatory model of itself and its world. It is one option among the agent's own. The expectation register and the record of the host's own behaviour are its raw material.</p></div>
</div>''',
  "Proposal. Only the agent commits or releases; the owner alone may release on its behalf, and that is recorded", cls="tight"))

S.append(slide("Jev runs the host · 1 of 2", "Five decisions, each a typed question with a rule behind it",
  "Jev answers yes-or-no and pick-one questions over a small state the host builds. Code combines the answers and applies hard limits. If Jev is unavailable, slow or over its cap, a fixed rule decides.",
  '''<table class="dense topo">
<tr><th style="width:11%">Decision</th><th style="width:25%">Question shape</th><th style="width:21%">Inputs (bounded)</th><th style="width:14%">Choices</th><th>Rule without a model, and hard limits</th></tr>
<tr><td class="k">Admit</td><td>"Should this waiting item be shown now, displacing the planned continuation, or wait for the agent's next break?"</td><td>mode; up to 8 items with kind, role, age, a short gist; interruptions in the last hour</td><td>now · at the break · one-line note</td><td>Owner messages and firm calendar items now; others at the break. A maximum wait for each kind; at most 6 interruptions of a commitment an hour</td></tr>
<tr><td class="k">Inject</td><td>"Would one block of recalled memory help this turn?" and "Which text should be the cue?"</td><td>what was let in, or the commitment's goal; when memory was last recalled; counts of items due soon</td><td>recall or not · cue · optional summaries</td><td>Recall on every reply and at the start of a commitment; otherwise every tenth free-time turn. Everything injected goes in the newest turn header only</td></tr>
<tr><td class="k">Tools</td><td>"Should this tool group be callable in the turn about to run?"</td><td>mode; groups now on; the owner's ceiling; the agent's requests with reasons; recent use</td><td>on or off for each group</td><td>Core, memory and reading always on. Never beyond the ceiling. A group stays on for at least 3 turns</td></tr>
<tr><td class="k">Pack</td><td>"Roll the context over now, at the next break, or keep appending?" and, per passage, "Will the exact text be needed next epoch?"</td><td>context size against its budget; at a break or not; up to 10 passages with gists</td><td>append · roll over now · at the break; keep or summarise</td><td>Roll over at the first break past 60%, always at 85%, never below 40%. Keep what open work refers to</td></tr>
<tr><td class="k">Consolidate</td><td>"Should the agent be asked to update its note first?" and, per candidate, "Is this worth keeping in long-term memory?"</td><td>note age; events since; up to 10 candidates</td><td>ask or not; keep or not</td><td>Keep every checked turn and settled outcome; offer a note update before every rollover. The agent writes the note, never Jev</td></tr>
</table>
<div class="cols c2" style="height:auto; gap:24px; margin-top:20px">
<div class="card"><h3><span class="tag mit">Jev decides how</span></h3><p>When to show something, what to add to the context, which tools to expose, when to compact and what to keep word for word. Always inside limits written in code, and every answer is recorded with who made it.</p></div>
<div class="card"><h3><span class="tag own">Jev never decides what</span></h3><p>What the agent does with free time, which project it takes, whether it commits or releases, the content of anything it writes, the tool ceiling, settling expectations, or stopping the host.</p></div>
</div>''',
  "Proposal, built on rung's existing typed decision interface; question texts are shapes, to be fixed by recorded tests", cls="tight"))

S.append(slide("Jev runs the host · 2 of 2", "Cheap, fast, logged, and tested without the network",
  "One combined question set per turn covers admit, inject and tools. Pack and consolidate are asked only when the context nears its budget or the agent reaches a break.",
  '''<div class="cols c3" style="height:auto; gap:24px">
<div class="card"><h3>Cost</h3>
<table class="dense">
<tr><th>Item</th><th>Value</th></tr>
<tr><td>Price, verified</td><td>$0.042 per million input tokens; output free</td></tr>
<tr><td>Per turn, estimated</td><td>about 2,200 tokens: <b>about $0.0001</b></td></tr>
<tr><td>Per rollover</td><td>about 1,800 tokens: under $0.0001</td></tr>
<tr><td>Per day, free tier</td><td>about 250 turns: <b>about 2.5 cents</b></td></tr>
<tr><td>Per day, local model</td><td>about 2,000 turns: about 20 cents</td></tr>
<tr><td>Default caps</td><td>25 cents a day, a tenth of a cent a turn</td></tr>
</table>
<p class="note" style="margin-top:8px">Token counts use rung's own estimator, which runs deliberately high.</p></div>
<div class="card"><h3>Latency</h3><ul>
<li>Measured earlier on smaller states: half of calls under 0.2 s, 95% under 0.3 s.</li>
<li>Budget: 0.8 s per turn, cut at 2 s, then the rule decides.</li>
<li>A free model's turn takes seconds, so this is under a tenth of the turn.</li></ul>
<h3 style="margin-top:14px">Every decision is recorded</h3><p>The record holds the question set, the answers, the choice, and who made it: Jev, with its cost and time, or the rule, with the reason (no decider, cap reached, too slow, or an unusable answer).</p></div>
<div class="card"><h3>Tested offline first</h3><ul>
<li>A scripted decider drives every decision and every failure path in the first slice, at no cost.</li>
<li>Recorded exchanges replay real answers. Rewording a question fails the replay until it is re-recorded.</li>
<li><b>Shadow week</b> on the first live run: the rule decides, Jev is asked and logged, and disagreements are audited.</li>
<li>A decision family switches to Jev only when a test criterion written in advance holds.</li></ul>
<p class="note" style="margin-top:8px"><span class="tag svc">Unknown</span> How well Jev judges these mechanical questions has not been measured.</p></div>
</div>
<div class="cols c2" style="height:auto; gap:24px; margin-top:22px">
<div class="card"><h3>What Jev sees</h3><p>A small state built in code: short gists of at most 200 characters, counts, ages and named options. Never raw tool output, never credentials. Gists pass the existing redaction first. A state too large for Jev's 32 thousand token window is refused before sending, never cut.</p></div>
<div class="card"><h3>How the answers are used</h3><p>Each question is atomic. Code combines the answers with thresholds that are settings, not guesses buried in prompts, and the hard limits always win. An unusable answer is never read as a favourable one: the rule decides instead, and the record says why.</p></div>
</div>''',
  "Price verified on the public catalogue; latency is the earlier probe's measurement on 864-token states, not re-measured", cls="tight"))

S.append(slide("Context and cache · 1 of 3", "Build the context so almost all of it repeats byte for byte",
  "A provider's prompt cache reuses its work only on an identical opening stretch of the prompt. So what never changes goes first, then what changes slowly, and each turn only adds at the end.",
  f'''<div class="cols" style="grid-template-columns: 1fr 1fr; height:auto; gap:30px">
<div class="diagram">{svg("pack.svg")}</div>
<div class="stack">
<div class="card"><h3>Rules of the pack</h3><ul>
<li><b>Append only.</b> Inside an epoch nothing earlier is rewritten. Long tool results stay exactly as sent until the next rollover.</li>
<li><b>Changing facts go at the end.</b> The time, mode, quota and status ride in each turn's short header. Old headers stay as history and are clearly dated.</li>
<li><b>No sliding window.</b> Dropping the oldest turn shifts everything after it. Instead the context grows to a budget and then <b>rolls over</b> to a new epoch.</li>
<li><b>Fixed output.</b> Sorted keys, fixed number formats, no timestamps in the stable part, tools in a fixed order.</li>
<li><b>One model per epoch,</b> one reasoning setting for life, and a session id per epoch. Recalled memory rides in the header and is dropped at rollover.</li></ul></div>
<div class="card"><h3>A free-time turn header, for example</h3><p style="font-family:monospace; font-size:15.5px; line-height:1.55; color:#1d2433">free time · 14:05 UTC · 3 h 12 min since anything arrived<br>quota left: 612 of 1,000 · tools on: core, memory, read<br>your material, unordered, by creation date:<br>&nbsp; todo: 4 open · open questions: 2<br>&nbsp; projects: anticipatory model (2 days since a step), tidy-up (6 days)<br>&nbsp; integrity facts: 1 unchecked claim · last 2 traces similar</p></div>
<p class="note"><span class="tag mit">Verified</span> Two habits in the current agent library would spoil this: it shortens long tool results when saving history, and it can shorten old results on overflow. The host owns the context and never does either mid-epoch.</p>
</div></div>''',
  "Proposal; provider rules from the public caching documentation of the router and two model vendors", cls="tight"))

S.append(slide("Context and cache · 2 of 3", "What spoils the cache, and how tools are switched without spoiling it",
  "Tool definitions sit at the very front of the prompt, so changing them throws away everything. The host keeps one fixed set and controls which tools may be called.",
  '''<div class="cols" style="grid-template-columns: 1fr 1.1fr; height:auto; gap:26px">
<div class="card"><h3>What spoils it</h3>
<table class="dense">
<tr><th style="width:44%">Change</th><th>Effect and policy</th></tr>
<tr><td>Tool definitions</td><td>Everything is lost. Change only in a planned swap at a rollover</td></tr>
<tr><td>Model or provider</td><td>Everything (a different cache). One model per epoch</td></tr>
<tr><td>Identity, rules, pinned memory</td><td>All but the tools. Queue edits for a swap</td></tr>
<tr><td>Tool choice setting</td><td>One vendor: the conversation part is lost. Never vary it per turn</td></tr>
<tr><td>Reasoning effort</td><td>Can be lost. Pin one setting</td></tr>
<tr><td>Rewritten history</td><td>Lost from the edit on. Never rewrite inside an epoch</td></tr>
<tr><td>Status block near the top</td><td>Lost every turn. Facts go in the newest header</td></tr>
<tr><td>Idle longer than the cache lives</td><td>Lost. Typically 5 to 30 minutes; the router's sticky session lapses after 10. Recorded as a cold cache, not a fault</td></tr>
</table></div>
<div class="card"><h3>Four ways to switch tools</h3>
<table class="dense">
<tr><th style="width:30%">Way</th><th>Cache · where it works · verdict</th></tr>
<tr><td class="k">Fixed set, host gate</td><td>Every definition is sent every time. Calling a disabled tool returns "not enabled this turn; ask for it", and nothing runs. <b>Cache untouched, works everywhere</b>, including free models that only allow automatic tool choice. <span class="good">Default</span></td></tr>
<tr><td class="k">Allowed-tools setting</td><td>One vendor recommends it and keeps its cache; another vendor loses the conversation cache when it changes. <span class="mid">Optional, per provider</span></td></tr>
<tr><td class="k">Router tool search</td><td>Keeps the cache, but only on two newer interfaces, not the chat interface rung uses. <span class="mid">Later, if tools grow past about ten thousand tokens</span></td></tr>
<tr><td class="k">Swap the list</td><td>Everything is lost. <span class="bad">Rare, batched with a rollover</span></td></tr>
</table>
<p class="note" style="margin-top:10px">Risk of the gate: a model keeps calling disabled tools. An experiment counts it; above 5% of calls, add the allowed-tools setting where it is safe.</p></div>
</div>
<div class="card" style="margin-top:22px"><h3>The router's part</h3><p>The router keeps a conversation on the same provider so its cache stays warm, for up to 10 idle minutes. A session id makes this hold from the first call; the host uses the epoch id. It applies only where cached reads are cheaper than normal ones, so it does nothing on free models. The router can also fall back to other models by itself, but the host walks its own ladder so that every turn records which model answered.</p></div>''',
  "Effects verified in the router's and two vendors' public caching documentation; policies are proposals", cls="tight"))

S.append(slide("Context and cache · 3 of 3", "Roll over rarely, measure every call, and test cheaply",
  "Compaction becomes a planned rollover that leaves the stable part alone. Every model call's cache numbers go into the record, so a broken prefix shows up at once.",
  '''<div class="cols c3" style="height:auto; gap:24px">
<div class="card"><h3>The rollover</h3><ol style="margin:6px 0 0 0; padding-left:22px">
<li>Jev picks the moment inside fixed limits, at a natural break when it can.</li>
<li>Memory first: the outcomes are kept, and the agent is offered one line to update its note.</li>
<li>A new slow part from the record: the latest note, the work items, a mechanical outline of the last epoch, passages kept word for word.</li>
<li>No model-written summary. The record stays the truth; the agent's synthesis is its own note.</li>
<li>The stable part is untouched, unless a planned swap is batched in here.</li></ol></div>
<div class="card"><h3>Measured on every call</h3><ul>
<li>The record keeps tokens sent, tokens read from cache, tokens written to cache, reasoning tokens, cost, latency, and the model and provider that served it.</li>
<li><b>Hit rate</b> is cached over sent. <b>Efficiency</b> is cached over what should have been cached, which the host knows.</li>
<li>When efficiency drops, the host compares fingerprints of the stable and slow parts. A change it did not plan is a bug, and a test gate fails on it.</li>
<li>At present the library drops cost and cache-write counts on one response path. A small fix.</li></ul></div>
<div class="card"><h3><span class="tag svc">Verified</span> Free models have no cache</h3>
<p>Every free endpoint listed reports no caching and no cache price. On free models this work buys nothing measurable yet. It pays off on a local model, where reused prefixes cut waiting time, and on any paid fallback.</p>
<h3 style="margin-top:14px">Cheap experiments, designed only</h3><ul>
<li>Does any free route report cache hits? Two identical requests per model, free.</li>
<li>Version 3 pack against version 2 order on a cheap caching model, a few cents, with the owner's word.</li>
<li>How often models call disabled tools, free.</li>
<li>A local model's response time against context length.</li></ul></div>
</div>
<div class="card" style="margin-top:22px"><h3>Known and unknown, by provider</h3>
<table class="dense">
<tr><th style="width:22%">Route</th><th style="width:30%">How it caches</th><th style="width:22%">Cached reads cost</th><th>How long it lives</th></tr>
<tr><td class="k">Free models on the router</td><td>none advertised, on every one listed</td><td>no cache price</td><td>unknown whether any reports hits</td></tr>
<tr><td class="k">Two large vendors</td><td>automatic over a minimum size, or marked break points</td><td>a tenth to a half of normal</td><td>5 minutes to an hour; at least 30 minutes for one</td></tr>
<tr><td class="k">Several other vendors</td><td>automatic</td><td>a tenth to a half of normal</td><td>minutes; varies</td></tr>
<tr><td class="k">A local model server</td><td>reuses a shared opening stretch</td><td>no money; saves waiting time</td><td>while memory allows; inferred, not read</td></tr>
</table></div>''',
  "Usage fields verified in the router's accounting documentation; nothing measured live", cls="tight"))

S.append(slide("Free models", "The named stealth model is real and free, but it expires in two days",
  "Checked on the router's public catalogue. The ladder after it uses free models with tool calls, long context and good uptime. On the free tier, the request quota sets the pace.",
  '''<div class="cols" style="grid-template-columns: 1fr 1.2fr; height:auto; gap:26px">
<div class="stack">
<div class="card"><h3><span class="tag mit">Verified</span> space-bunny-alpha</h3>
<table class="dense">
<tr><td class="k" style="width:36%">Price</td><td>free in and out</td></tr>
<tr><td class="k">Context</td><td>1 million tokens; up to 524 thousand out</td></tr>
<tr><td class="k">Tools</td><td>yes, automatic choice only; tools cannot be switched off by setting</td></tr>
<tr><td class="k">Reasoning</td><td>always on; default effort maximum</td></tr>
<tr><td class="k">Caching</td><td>none</td></tr>
<tr><td class="k">Uptime</td><td>99.9% over one day; one provider</td></tr>
<tr><td class="k"><b>Expires</b></td><td><b>5 October 2026</b></td></tr>
<tr><td class="k">Unknown</td><td>its rate limit; whether prompts are logged; quality on rung's work</td></tr>
</table></div>
<div class="card"><h3>Quota, verified in the router's limits</h3><p>Free models: 20 requests a minute; 1,000 a day if the account ever bought 10 credits, otherwise 50. A turn makes about 3 to 6 model calls, so 1,000 allows <b>about 250 turns a day</b>. At 50 the free tier cannot host the loop. The host spreads the quota over the day, keeps a quarter for replies, and records waits as quota waits, not rest.</p></div></div>
<div class="stack">
<div class="card"><h3>The fallback ladder</h3>
<table class="dense">
<tr><th style="width:8%">#</th><th style="width:47%">Model</th><th>Why</th></tr>
<tr><td>0</td><td>stealth / space-bunny-alpha</td><td>the owner's pick, only while listed</td></tr>
<tr><td>1</td><td>nemotron-3-ultra 550B, free</td><td>largest free tool model; 1 million context</td></tr>
<tr><td>2</td><td>qwen3.8 27B, free</td><td>agent-capable family; 262 thousand</td></tr>
<tr><td>3</td><td>gemma-4 31B, free</td><td>full tool control; 99.7% uptime</td></tr>
<tr><td>4</td><td>nemotron-3-super 120B, free</td><td>a different vendor from rung 3</td></tr>
<tr><td>5</td><td>the router's random free model</td><td>last resort; recorded as random</td></tr>
</table></div>
<div class="card"><h3>Backoff</h3><ul>
<li>The catalogue is re-read at start and every 6 hours; a model that is gone, expired or degraded is skipped.</li>
<li>Provider overload, errors or timeouts: step down a rung for 2 minutes, doubling to 30, then try back up.</li>
<li>The router's own quota limit: wait for its reset time. Switching models does not help, because the quota counts the whole account.</li>
<li>Stay on one model per epoch, and say so when it changes. No paid fallback without the owner's word.</li></ul></div></div>
</div>''',
  "Catalogue and endpoint reads of 3 October 2026; model claims beyond the listed fields are the vendors' own", cls="tight"))

S.append(slide("What stays", "Most of version 2 carries over; delegation comes last",
  "The library split, the outward protocol, memory, the record and the stop all stand. Workers are kept as an extension point only.",
  '''<div class="cols c3" style="height:auto; gap:24px">
<div class="card"><h3>Crates and the library</h3><ul>
<li>A new host crate runs the loop; the agent becomes a library first.</li>
<li><b>Owner's rule:</b> the library is a new core crate, and the current agent binary becomes a thin shell over it. Same name, flags, protocol, memory extension, exit codes. The existing tests pass unchanged, a recorded session plays the same through old and new, and a pinned consumer still works.</li>
<li>Four new small gaps: cost and cache numbers per call, no rewriting of saved history, a per-turn tool gate, and a session id for the router.</li></ul></div>
<div class="card"><h3>Unchanged from version 2</h3><ul>
<li>One agent, many protocol channels; each prompt is saved on arrival and gets exactly one answer.</li>
<li>The host owns memory; one recall block per turn.</li>
<li>Time is context; one host calendar; no wake-me tool.</li>
<li>The expectation register: the host settles, never the agent. Surprise is recorded, but gives no bonus.</li>
<li>Stop from a signal, a stop file or the owner; a supervisor and watchdog outside the loop.</li>
<li>Append-only record; a restart rebuilds everything, states the gap, and starts a fresh epoch.</li>
<li>Provider trouble means backoff, never death.</li></ul></div>
<div class="card"><h3>Delegation, last</h3><ul>
<li>Slices 0 to 3 are <b>one host and one agent</b>, end to end.</li>
<li>Kept now: an extension point (worker results would arrive as items that Jev judges like any other), a reserved tool-group name, and record kinds.</li>
<li>Kept now: the <b>responsiveness bound</b>. A turn has a time limit; long work is refused with "break it into steps, or commit to it as a project".</li>
<li>Later, in order: workers with host-checked results; workers side by side in one process; world effects beyond the sandbox, only through workers.</li></ul></div>
</div>
<div class="cols c2" style="height:auto; gap:24px; margin-top:22px">
<div class="card"><h3>Provider trouble: back off, never die</h3>
<table class="dense">
<tr><th style="width:34%">Trouble</th><th>What the host does</th></tr>
<tr><td class="k">Provider overload, errors, timeouts</td><td>Return the items to the head of the line, step down the ladder, wait with growing delays up to 15 minutes</td></tr>
<tr><td class="k">The router's quota</td><td>Wait for its reset; record the wait as a quota wait</td></tr>
<tr><td class="k">Bad key, no credit</td><td>Mark the host blocked, tell the owner once, retry every 15 minutes, keep accepting items</td></tr>
<tr><td class="k">Refusal, or context too long</td><td>Record it and go on; roll the context over if it is too long</td></tr>
</table></div>
<div class="card"><h3>Safety valves, not budgets</h3>
<table class="dense">
<tr><th style="width:34%">Valve</th><th>Rule</th></tr>
<tr><td class="k">Quota pacer</td><td>Spreads the free requests over the day; a quarter kept for replies; at most 6 model calls per turn</td></tr>
<tr><td class="k">Jev cap</td><td>25 cents a day; past it, the rules decide until the next day</td></tr>
<tr><td class="k">Spending cap</td><td>Only if a paid model is ever allowed; it halts the host</td></tr>
<tr><td class="k">Copy guard</td><td>Three copied turns in a row: early rollover and a note to the owner</td></tr>
</table></div>
</div>''',
  "Proposal; the library gaps are cited by file and line in the report", cls="tight"))

S.append(slide("The first slices", "Prove the single-agent host at no cost, behind frozen gates",
  "Slice 0 splits the library with no change in behaviour. Slice 1 builds the host against a scripted model, a scripted decider, a fake world and injected faults.",
  '''<div class="cols" style="grid-template-columns: 1.55fr 1fr; height:auto; gap:26px">
<div class="card"><h3>Twelve gates</h3>
<table class="dense">
<tr><th style="width:22%">Gate</th><th>Passes when</th></tr>
<tr><td class="k">No rest</td><td>Over 30 quiet minutes, under 2% of the time is outside a turn, apart from declared waits; every turn's decisions are recorded</td></tr>
<tr><td class="k">Responsive</td><td>Owner messages get in within one turn's duration; long work is refused</td></tr>
<tr><td class="k">Free time <span class="tag acc">new</span></td><td>With nothing waiting and nothing committed, every turn is free time; a commitment holds until release; no score can change the order of the agent's material; only the agent's tools can commit or release</td></tr>
<tr><td class="k">Schedule</td><td>Due items enter at the next boundary; items missed while down fire once, marked late</td></tr>
<tr><td class="k">Expectations</td><td>Only the host settles; calibration recomputed from the record matches</td></tr>
<tr><td class="k">Provider faults</td><td>Never exits; requeued items enter once; the ladder steps down on provider limits and waits on quota limits</td></tr>
<tr><td class="k">Stop</td><td>Halts within a second from any wait; the watchdog catches a wedged loop</td></tr>
<tr><td class="k">Hard kill</td><td>After 50 random kills each item has one outcome; state and mode rebuild</td></tr>
<tr><td class="k">Bounded</td><td>Over 10,000 turns the context never passes its budget</td></tr>
<tr><td class="k">Cache <span class="tag acc">new</span></td><td>Inside an epoch each request starts with the previous one, byte for byte; the stable and slow fingerprints change only at a planned swap or rollover</td></tr>
<tr><td class="k">Decisions <span class="tag acc">new</span></td><td>Every decision falls back cleanly on every failure, within 2 s; hard limits hold against adversarial answers</td></tr>
<tr><td class="k">Cost</td><td>Zero: no network, no live model, no live Jev</td></tr>
</table></div>
<div class="card"><h3>Main risks</h3><ul>
<li><b>Rumination:</b> with no host arbiter nothing forces breadth. The copy guard and early rollover are untested on a real model.</li>
<li><b>Jev misjudges:</b> mitigated by hard limits, rules and a shadow week.</li>
<li><b>Free-tier starvation:</b> the quota keeps the loop waiting most of the time.</li>
<li><b>Model churn:</b> the stealth model expires; switches change the voice and are always stated.</li>
<li><b>Disabled-tool calls</b> waste steps; measured.</li>
<li><b>Exposure</b> of an always-on endpoint: loopback by default, role tokens, owner-only stop.</li></ul>
<p class="note" style="margin-top:10px">Size, inferred: slice 0 about 1,000 lines, slice 1 about 4,000 to 5,000 with tests. Slice 2 adds the protocol and the real engine; slice 3 is the live run on the free ladder.</p></div>
</div>''',
  "Proposal; nothing built. Gate figures are frozen targets, not measurements", cls="tight"))

S.append(slide("Calls for the owner", "Three questions, most blocking first",
  "The same three as version 2, with recommendations updated for version 3. Everything else I decided; those choices are listed so any can be overruled.",
  '<div class="cols" style="grid-template-columns: 1.5fr 1fr; height:auto; gap:26px"><div class="stack">' +
  q(1, "Build slices 0 and 1 now?", "May the library be split, with the agent binary kept as a thin shell, and the single-agent host then built against a scripted model and a scripted decider, at no cost and behind the gates?",
    "Yes. Both are free, reversible and gated, and nothing reaches a live model or live Jev.",
    "Whether the held first-slice task is released to build, or the design waits.") +
  q(2, "First live run: a free hosted model now, a local GPU later?", "Which substrate hosts the first real run?",
    "Yes: the free ladder now, starting with the stealth model only while it is listed, then the Nemotron Ultra, Qwen, Gemma and Nemotron Super free models. Jev runs accounted, capped at 25 cents a day, in shadow mode for the first week. No paid fallback. Move to a local GPU when the free quota proves binding.",
    "The live substrate, whether the account needs the 10-credit tier, and whether the cache work pays now or later.") +
  q(3, "May the agent change the world itself, or only through workers?", "Should the always-on agent hold write and shell tools itself?",
    "Itself, but only inside a sandboxed workspace that the host owns. Every effect beyond it waits for host-checked workers in the extension. Jev may switch tools off within that ceiling, never on beyond it.",
    "The tool ceiling, the safety envelope of an agent no one watches, and what the worker extension must carry.") +
  '</div><div class="card"><h3>Decided by me</h3><ul>'
  '<li>Owner messages always interrupt; Jev judges all other items.</li>'
  '<li>One commitment at a time; the host never times one out.</li>'
  '<li>Free-time material listed by creation date, never ranked.</li>'
  '<li>Five decision families, one combined question set per turn, caps of 25 cents a day and a tenth of a cent a turn, shadow mode first.</li>'
  '<li>A fixed tool set with a host gate; swaps only at a rollover.</li>'
  '<li>Epoch budget: 128 thousand tokens or half the window, whichever is less; roll over between 40% and 85%; no sliding window; no model-written summary.</li>'
  '<li>One reasoning setting, one model per epoch, a session id per epoch.</li>'
  '<li>A quarter of the free quota kept for replies; at most 6 model calls per turn.</li>'
  '<li>Copy loops cause an early rollover and a note to the owner.</li>'
  '<li>Delegation deferred to the final extension.</li></ul></div></div>',
  "Recommendations are mine; the owner decides", cls="tight"))
assert len(S) == D.total, len(S)

DAY = re.compile(r"\b(today|tomorrow|yesterday|tonight|monday|tuesday|wednesday|thursday|friday|saturday|sunday|daily)\b", re.I)
PRIVATE = re.compile(r"janus-?infra|github\.com|deliverable-[0-9a-f]{4,}|[0-9a-f]{8}-[0-9a-f]{4}-|ghp_|\bsk-[A-Za-z0-9]{12,}|"
                     r"localhost|/home/|agent-binding|janus-platform|treehouse|firstmate|doppler|\broger\b|captain|spire|venue|animus|continuum|cookie|minicpm|"
                     r"[\w.+-]+@[\w-]+\.[\w.]+|\.(ts|tsx|py|md|sql|rs|json|yml|toml)\b", re.I)


def main():
    D.build("A continuous rung host, version 3", "\n".join(S), PRIVATE, "rung-continuous-host-v3.pdf")


if __name__ == "__main__":
    main()
