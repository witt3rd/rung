#!/usr/bin/env python3
"""_reach.py — surface the blast radius of a changed question, typed, for REVIEW.

The registry is a graph: each question depends_on others / affects external
anchors, and each edge is *typed* — the type decides how a change propagates.
When a question changes state (e.g. Q7 resolves), some set of things rests on
it and must be re-examined. This script computes that set and prints a review
checklist. It NEVER mutates anything — the graph surfaces the blast radius; the
human judges each edge. (Same produce-first / gate-second discipline as the rest
of the system: a changed premise does not auto-invalidate its dependents.)

Stdlib only. Nodes are files under this directory; external targets (doc
sections, decisions) are leaves we surface but cannot recurse into.

    python _reach.py q7          # what must be reviewed if q7 changed?
    python _reach.py --graph     # dump the whole typed edge list
"""
import pathlib, re, sys
from collections import defaultdict

ROOT = pathlib.Path(__file__).parent

# kind -> (propagation verb, does it recurse through the registry?)
PROP = {
    "premise":       ("MUST re-examine — rests on this as a premise", True),
    "justification": ("REVIEW — the premise moved; the decision may still hold", True),
    "spawn":         ("REVISIT — exists only because of this resolution", True),
    "gate":          ("UNBLOCK? — this gate may have lifted", True),
    "citation":      ("mechanical — update the reference", False),
    "evidence":      ("(inbound evidence — informational)", False),
    "related":       ("(see-also — no propagation)", False),
}

def parse(path):
    """Return (id, status, depends_on[(on,kind)], affects[(target,kind)])."""
    text = path.read_text()
    if not text.startswith("---"):
        return None
    fm = text.split("---", 2)[1]
    id_ = re.search(r"^id:\s*(\S+)", fm, re.M)
    st  = re.search(r"^status:\s*(\S+)", fm, re.M)
    def pairs(field, key):
        out = []
        block = re.search(rf"^{field}:\n((?:  - .*\n?)+)", fm, re.M)
        if block:
            for line in block.group(1).splitlines():
                m = re.search(rf"{key}:\s*([^,}}]+),\s*kind:\s*([^}}]+)", line)
                if m:
                    out.append((m.group(1).strip(), m.group(2).strip()))
        return out
    return (id_.group(1) if id_ else path.stem,
            st.group(1) if st else "?",
            pairs("depends_on", "on"),
            pairs("affects", "target"))

def load():
    nodes, depends, affects = {}, {}, {}
    for p in ROOT.rglob("*.md"):
        if p.name.startswith("_") or "_evidence" in p.parts:
            continue
        r = parse(p)
        if r:
            id_, st, dep, aff = r
            nodes[id_] = (st, p.relative_to(ROOT))
            depends[id_], affects[id_] = dep, aff
    # reverse internal edges: on -> [(dependent_id, kind)]
    rev = defaultdict(list)
    for id_, dep in depends.items():
        for on, kind in dep:
            rev[on].append((id_, kind))
    return nodes, rev, affects

def reach(target):
    nodes, rev, affects = load()
    print(f"\n  ┌─ blast radius of a change to  {target}")
    seen, frontier, hits = {target}, [(target, None)], []
    while frontier:
        node, _ = frontier.pop()
        for dependent, kind in rev.get(node, []):
            if dependent not in seen:
                seen.add(dependent)
                hits.append((dependent, kind, "internal"))
                if PROP.get(kind, ("", False))[1]:
                    frontier.append((dependent, kind))
    # external anchors this node's change ripples to (from its own `affects`)
    for tgt, kind in affects.get(target, []):
        hits.append((tgt, kind, "external"))

    if not hits:
        print("  └─ nothing rests on it. (a leaf — safe to change freely.)\n")
        return
    for node, kind, loc in hits:
        verb = PROP.get(kind, ("?", False))[0]
        where = f"  [{nodes[node][1]}]" if loc == "internal" and node in nodes else "  (external)"
        print(f"  │  • {node:<26} {kind:<13} {verb}{where}")
    print("  └─ REVIEW each. the graph surfaces; you judge. nothing was changed.\n")

def dump_graph():
    nodes, _, _ = load()
    print("\n  registry edges (typed):\n")
    for p in sorted(ROOT.rglob("*.md")):
        if p.name.startswith("_") or "_evidence" in p.parts:
            continue
        r = parse(p)
        if not r:
            continue
        id_, _st, dep, af = r
        for on, kind in dep:
            print(f"  {id_}  --{kind}-->  {on}")
        for tgt, kind in af:
            print(f"  {id_}  ~~{kind}~~>  {tgt}   (external)")
    print()

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print(__doc__)
    elif sys.argv[1] == "--graph":
        dump_graph()
    else:
        reach(sys.argv[1])
