#!/usr/bin/env python3
"""Write the mockup pages (static HTML, mock data). Run: python3 gen.py"""
from pathlib import Path
HERE = Path(__file__).resolve().parent

TABS = [("now", "Now", "instance.html"), ("turns", "Turns", "turns.html"), ("queue", "Queue", "queue.html"),
        ("console", "Console", "console.html"), ("config", "Configure", "configure.html")]

def page(title, body, extra_css=""):
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title><link rel="stylesheet" href="app.css"><style>{extra_css}</style></head><body>{body}</body></html>'''

def top(crumb):
    return f'<div class="top"><span class="crumb"><a href="index.html">Instances</a>{crumb}</span></div>'

def inst(active, action):
    tabs = "".join(f'<a href="{h}"{" aria-current=\"page\"" if k == active else ""}>{n}</a>' for k, n, h in TABS)
    return f'''{top(" / atlas")}<div class="page">
<div class="head"><div><h1>atlas</h1><p class="line"><span class="mark live"></span>Working. Turn 214 is running.</p></div><span class="sp"></span>{action}</div>
<nav class="tabs" aria-label="atlas">{tabs}</nav>'''

ADD = '<a class="btn primary" href="queue.html">Add to queue</a>'
out = {}

# 1 overview ------------------------------------------------------------
out["index.html"] = page("Instances", f'''{top("")}<div class="page">
<h1>Instances</h1><p class="line">Three running. One needs you.</p>
<div style="height:24px"></div>
<a class="row ov" href="instance.html" style="text-decoration:none;color:inherit"><span class="t"><b>atlas</b></span><span>Working</span><span data-content>Tidying the notes index</span><span class="meta">357 of 1,000 requests</span><span class="meta">3 s ago</span></a>
<a class="row ov" href="instance.html" style="text-decoration:none;color:inherit"><span class="t"><b>bramble</b></span><span>Free time</span><span data-content>Nothing pulls it. Last real work 3 h ago.</span><span class="meta">198 of 1,000 requests</span><span class="meta">41 s ago</span></a>
<a class="row ov next" href="instance.html" style="text-decoration:none;color:inherit"><span class="t"><b>cedar</b></span><span class="needs-text"><span class="mark needs"></span>Stuck</span><span data-content>The model key was refused. Retrying every 15 min.</span><span class="meta">0 of 1,000 requests</span><span class="meta">9 min ago</span></a>
<p class="fold" style="margin-top:16px">Showing all 3 instances.</p>
</div>''', '''
.ov{grid-template-columns: 9rem 9rem minmax(0,1fr) 12rem 6rem} .ov .meta:last-child{text-align:right}
@media (max-width:760px){.ov{grid-template-columns:1fr auto;} .ov>:nth-child(3){grid-column:1/-1;grid-row:2} .ov>:nth-child(4){grid-column:1;grid-row:3} .ov>:nth-child(5){grid-column:2;grid-row:3;text-align:right} .ov>:nth-child(2){text-align:right}}''')

# 2 instance ------------------------------------------------------------
out["instance.html"] = page("atlas", f'''{inst("now", ADD)}
<div class="cols"><div>
<section class="sec"><h2>Doing now</h2>
<div class="card look"><p style="font-size:var(--s-look);margin:0" data-content>Tidy the notes index</p>
<p class="meta" style="margin:6px 0 0" data-content>Committed 12 turns ago. Done when every note has an index entry.</p>
<p style="margin:14px 0 0"><span class="mark live"></span>Live: writing a note, 38 s in.</p></div></section>
<section class="sec"><h2>Recent turns</h2>
<div class="row tr"><span data-content>213 · Wrote 3 index entries</span><span class="meta">1 min ago</span></div>
<div class="row tr"><span data-content>212 · Read the notes folder</span><span class="meta">2 min ago</span></div>
<div class="row tr"><span data-content>211 · Committed to the notes index</span><span class="meta">3 min ago</span></div>
<p class="fold" style="margin-top:12px">Showing 3 of 214. <a href="turns.html">Show more</a></p></section>
</div><aside>
<section class="sec"><h2>Next</h2><p style="margin:0" data-content>Stand-up at 14:30</p><p class="meta" style="margin:2px 0 0"><a href="queue.html">2 queued</a></p></section>
<section class="sec"><h2>Decided</h2><p style="margin:0" data-content>Held the digest for the next break.</p><p class="meta" style="margin:2px 0 0"><a href="turns.html">Why</a></p></section>
<section class="sec"><h2>Health</h2><p style="margin:0">Context 61% full. Cache 93%.</p><p class="meta" style="margin:2px 0 0">$0.00 spent. 357 of 1,000 requests.</p></section>
</aside></div></div>''', '.tr{grid-template-columns:1fr auto}.look{border:2px solid var(--ink)}')

# 3 turns (observe) -----------------------------------------------------
out["turns.html"] = page("atlas turns", f'''{inst("turns", "")}
<div class="field" style="margin-bottom:8px"><input type="text" placeholder="Search turns" aria-label="Search turns"><select aria-label="Show"><option>All turns</option><option>Tool calls</option><option>Decisions</option></select></div>
<p class="meta" style="margin:0 0 20px"><span class="mark live"></span>Following</p>
<div class="turn"><h2>Turn 214 <span class="meta">now</span></h2>
<p class="content" data-content>The index has 41 entries and the folder has 44 notes. Three are missing: the two from this week and the one about backoff. I will list them, then write the entries in order.</p>
<div class="call"><span class="k" data-content>Tool</span><span class="mono" data-content>ws_list notes/ → 44 files</span></div>
<div class="call"><span class="k" data-content>Tool</span><span class="mono" data-content>note_write index.md … running</span></div></div>
<div class="turn"><h2>Turn 213 <span class="meta">1 min ago</span></h2>
<p class="content" data-content>Wrote three index entries and checked the links. Two notes still lack a summary line.</p>
<div class="call"><span class="k" data-content>Decision</span><span data-content>Held the digest for the next break. By rule.</span></div></div>
<div class="turn"><h2>Turn 212 <span class="meta">2 min ago</span></h2>
<p class="content" data-content>Read the notes folder and counted what the index covers.</p></div>
<p class="fold">Showing 3 of 214. <a href="#">Show 50 more</a> · <a href="#">Show all 214</a></p>
</div>''')

# 4 queue ---------------------------------------------------------------
out["queue.html"] = page("atlas queue", f'''{inst("queue", "")}
<section class="sec"><h2>Add to the queue</h2>
<div class="field"><select aria-label="Kind"><option>Message</option><option>Task</option><option>Pull</option></select><input type="text" placeholder="What should it know or do?" aria-label="Text"><a class="btn primary" href="#">Add</a></div></section>
<section class="sec"><h2>Being answered now</h2>
<div class="row q"><span data-content>Which notes still lack a summary line?</span><span class="meta">Message · 2 min ago</span><span></span></div></section>
<section class="sec"><h2>Waiting</h2>
<div class="row q"><span data-content>Draft the week's open questions</span><span class="meta">Task · 9 min ago</span><span><button class="quiet" aria-label="Move up">↑</button><button class="quiet" aria-label="Move down">↓</button><button class="quiet">Cancel</button></span></div>
<div class="row q"><span data-content>Stand-up: say what you are on</span><span class="meta">Message · 14:30</span><span><button class="quiet" aria-label="Move up">↑</button><button class="quiet" aria-label="Move down">↓</button><button class="quiet">Cancel</button></span></div></section>
<section class="sec"><h2>On its shelf</h2>
<div class="row q"><span data-content>Look into why free models answer with one dot</span><span class="meta">Pull · 1 d ago</span><span><button class="quiet">Remove</button></span></div></section>
</div>''', '.q{grid-template-columns:minmax(0,1fr) 11rem 11rem}@media (min-width:761px){.q>:nth-child(2),.q>:nth-child(3){text-align:right}}@media (max-width:760px){.q{grid-template-columns:1fr auto}.q>:nth-child(2){grid-column:1;grid-row:2}.q>:nth-child(3){grid-column:2;grid-row:1/3}}')

# 5 configure -----------------------------------------------------------
out["configure.html"] = page("atlas configure", f'''{inst("config", "")}
<section class="sec"><h2>Model ladder</h2>
<div class="row lad"><span class="meta">1</span><span class="mono" data-content>stealth/space-bunny-alpha</span><span class="meta">Expired</span><span></span></div>
<div class="row lad"><span class="meta">2</span><span class="mono" data-content>qwen/qwen3.8-27b:free</span><span class="meta">In use</span><span><button class="quiet" aria-label="Move up">↑</button><button class="quiet" aria-label="Move down">↓</button></span></div>
<div class="row lad"><span class="meta">3</span><span class="mono" data-content>nvidia/nemotron-3-ultra-550b-a55b:free</span><span class="meta"></span><span><button class="quiet" aria-label="Move up">↑</button><button class="quiet" aria-label="Move down">↓</button></span></div>
<div class="row lad"><span class="meta">4</span><span class="mono" data-content>google/gemma-4-31b-it:free</span><span class="meta"></span><span><button class="quiet" aria-label="Move up">↑</button><button class="quiet" aria-label="Move down">↓</button></span></div></section>
<section class="sec"><h2>Free time</h2><label class="field"><input type="checkbox" style="width:24px;height:24px"> Answer idle turns in one line</label></section>
<section class="sec"><h2>Budget</h2><div class="field"><label>Requests a day <input type="text" value="1,000" style="width:7rem;flex:none"></label><label>A minute <input type="text" value="20" style="width:5rem;flex:none"></label></div></section>
<section class="sec"><h2>Schedule</h2><p style="margin:0" data-content>Stand-up at 14:30, firm</p></section>
<section class="sec"><h2>Sandbox</h2><p class="mono" style="margin:0" data-content>/home/rung/atlas/workspace</p></section>
<section class="sec" style="border-top:1px solid var(--rule);padding-top:20px"><p style="margin:0 0 12px">Takes effect on its next turn.</p><a class="btn primary" href="#">Apply 1 change</a></section>
</div>''', '.lad{grid-template-columns:2rem minmax(0,1fr) 6rem auto}@media (max-width:760px){.lad{grid-template-columns:1.5rem 1fr auto}.lad>:nth-child(3){grid-column:2;grid-row:2}.lad>:nth-child(4){grid-column:3;grid-row:1/3}}')

# 6 console -------------------------------------------------------------
lines = [
 ("14:02:11","host","turn 214 started: free → committed, rung 2 of 5, epoch 31"),
 ("14:02:11","agent","POST https://router.example/api/v1/chat/completions 200 in 3.1 s, 22,114 prompt tokens, 21,403 cached"),
 ("14:02:12","tool","ws_list notes/ → 44 files"),
 ("14:02:14","tool","env: RUNG_HOST_OPENROUTER_API_KEY=[redacted]"),
 ("14:02:15","agent","stderr: warning: result of note_write kept verbatim (12,884 bytes)"),
 ("14:02:49","host","turn 213 ended: answered, 3 calls, cost $0.00, hash 9f3a…c21"),
]
cl = "".join(f'<div class="cl" data-content><span class="meta">{t}</span><span class="meta">{s}</span><span class="mono">{x}</span></div>' for t, s, x in lines)
out["console.html"] = page("atlas console", f'''{inst("console", "")}
<div class="field" style="margin-bottom:8px"><input type="text" placeholder="Search the console" aria-label="Search the console"><select aria-label="Show"><option>All output</option><option>Host</option><option>Agent</option><option>Tools</option></select></div>
<p class="meta" style="margin:0 0 16px"><span class="mark live"></span>Following. Secrets show as [redacted].</p>
<div class="console">{cl}</div>
<p class="fold" style="margin-top:12px">Showing the last 6 of 18,420 lines. <a href="#">Show 500 earlier</a> · <a href="#">Show all</a></p>
</div>''', '.cl{display:grid;grid-template-columns:5.5rem 3.5rem minmax(0,1fr);gap:0 12px;padding:6px 0;border-top:1px solid var(--rule);overflow-wrap:anywhere}@media (max-width:760px){.cl{grid-template-columns:5rem 1fr}.cl>:nth-child(3){grid-column:1/-1}}')

for name, html in out.items():
    (HERE / name).write_text(html)
print("wrote", len(out), "pages")
