#!/usr/bin/env python3
"""Numbering and reference integrity for formalism.md.

The decimal number of a proposition is *derived*, never authored. Identity is
the slug in the anchor; hierarchy is `data-parent`; order is document order.

    <a id="classifier-not-judgmental" data-parent="conditional-names-classifier"></a>
    **2.51** The classifying sentence is not itself judgmental. ...

References are ordinary markdown links whose text is derived and whose target
is the stable slug:

    reopens the regress [6.4](#tower-floor) closes

    ./_props.py check    exit 1 on any integrity failure; changes nothing
    ./_props.py fmt      recompute every number and link text in place
    ./_props.py cited    exit 1 if Rust source cites a slug that is not a proposition
"""

import re
import sys
from pathlib import Path

DOC = Path(__file__).with_name("formalism.md")

ANCHOR = re.compile(r'^<a id="([a-z0-9-]+)"(?: data-parent="([a-z0-9-]+)")?></a>$')
PROP = re.compile(r"^\*\*(\d+(?:\.\d+)?)\*\*")
HEADING = re.compile(r"^## (?:\d+(?:\.\d+)? · )?(.*)$")
REF = re.compile(r"\[([^\]]*)\]\(#([a-z0-9-]+)\)")
MATH = re.compile(r"\$\$.*?\$\$|\$[^$]*\$")


class Prop:
    __slots__ = ("slug", "parent", "line", "old", "num", "kids")

    def __init__(self, slug, parent, line, old):
        self.slug, self.parent, self.line, self.old = slug, parent, line, old
        self.num, self.kids = None, []


def parse(lines):
    """Return (props in document order, errors)."""
    props, errs, seen = [], [], {}
    for i, line in enumerate(lines):
        m = ANCHOR.match(line)
        if not m:
            continue
        slug, parent = m.group(1), m.group(2)
        nxt = lines[i + 1] if i + 1 < len(lines) else ""
        p = PROP.match(nxt)
        if not p:
            errs.append(f"L{i+1}: anchor #{slug} is not followed by a **N** proposition line")
            continue
        if slug in seen:
            errs.append(f"L{i+1}: duplicate id #{slug} (first at L{seen[slug]})")
            continue
        seen[slug] = i + 1
        props.append(Prop(slug, parent, i + 1, p.group(1)))
    return props, errs


def build(props):
    """Link children to parents and assign derived numbers."""
    errs = []
    by_slug = {p.slug: p for p in props}
    roots = []
    for p in props:
        if p.parent is None:
            roots.append(p)
            continue
        parent = by_slug.get(p.parent)
        if parent is None:
            errs.append(f"L{p.line}: #{p.slug} names a parent that does not exist: #{p.parent}")
            continue
        if parent.line > p.line:
            errs.append(f"L{p.line}: #{p.slug} appears before its parent #{p.parent} (L{parent.line})")
        parent.kids.append(p)

    def number(node, num, depth):
        node.num = num
        if len(node.kids) > 9:
            errs.append(
                f"L{node.line}: #{node.slug} has {len(node.kids)} children; the "
                f"concatenating scheme is ambiguous past 9 — split it"
            )
        for i, kid in enumerate(node.kids, 1):
            sep = "." if depth == 0 else ""
            number(kid, f"{num}{sep}{i}", depth + 1)

    for i, root in enumerate(roots, 1):
        number(root, str(i), 0)
    return roots, errs


def strip_math(line):
    return MATH.sub(lambda m: " " * len(m.group(0)), line)


def rewrite(lines, props):
    """Return new lines with numbers and link texts recomputed."""
    by_slug = {p.slug: p for p in props}
    out = list(lines)

    for p in props:
        out[p.line] = PROP.sub(f"**{p.num}**", out[p.line], count=1)

    # a numbered `## N · Title` displays the top-level proposition that follows
    roots = [p for p in props if p.parent is None]
    nxt = {}
    for r in roots:
        for i in range(r.line - 2, -1, -1):
            if out[i].startswith("## "):
                nxt[i] = r.num
                break
            if out[i].startswith("**") or ANCHOR.match(out[i]):
                break
    for i, num in nxt.items():
        title = HEADING.match(out[i]).group(1)
        out[i] = f"## {num} · {title}"

    for i, line in enumerate(out):
        def sub(m):
            target = by_slug.get(m.group(2))
            return f"[{target.num}](#{target.slug})" if target else m.group(0)
        out[i] = REF.sub(sub, line)
    return out


def check_refs(lines, props):
    slugs = {p.slug for p in props}
    errs = []
    for i, line in enumerate(lines):
        if ANCHOR.match(line):
            continue
        for m in REF.finditer(line):
            if m.group(2) not in slugs:
                errs.append(f"L{i+1}: reference to #{m.group(2)}, which is not a proposition")
    # a bare decimal outside math and outside a link is almost certainly a
    # hand-written reference that was never converted
    for i, line in enumerate(lines):
        if PROP.match(line) or ANCHOR.match(line):
            continue
        bare = strip_math(REF.sub(lambda m: " " * len(m.group(0)), line))
        for m in re.finditer(r"(?<![\w.#/-])(\d{1,2}\.\d{1,2})(?![\w.])", bare):
            if m.group(1) in {p.num for p in props}:
                errs.append(f"L{i+1}: bare number {m.group(1)} — write it as a [n](#slug) link")
    return errs


# Slug-shaped tokens that are deliberately NOT propositions: variants the
# formalism names in order to refuse them. A ban has to say what it bans.
RETIRED = {"accept-with-mod", "reject-with-alternative"}

# Crates that cite the formalism by slug. Scoped deliberately: elsewhere in the
# workspace a hyphenated token is ordinary prose, not a citation.
CITING = ("rung-het",)

COMMENT = re.compile(r"^\s*(?://[/!]?)(.*)$")
SLUGLIKE = re.compile(r"\b[a-z][a-z0-9]*(?:-[a-z0-9]+){2,}\b")


def cited():
    """Every slug-shaped token in a Rust comment must be a proposition."""
    slugs = {p.slug for p in parse(DOC.read_text().split("\n"))[0]}
    root = DOC.parent.parent.parent
    errs, n = [], 0
    for crate in CITING:
      for src in sorted((root / crate).rglob("*.rs")):
        if "target" in src.parts:
            continue
        for i, line in enumerate(src.read_text(errors="ignore").split("\n")):
            m = COMMENT.match(line)
            if not m:
                continue
            for tok in SLUGLIKE.findall(m.group(1)):
                if tok in slugs:
                    n += 1
                elif tok not in RETIRED:
                    rel = src.relative_to(root)
                    errs.append(f"{rel}:{i+1}: cites `{tok}`, which is not a proposition")
    if errs:
        print("\n".join(f"  {e}" for e in errs), file=sys.stderr)
        print(f"\n{len(errs)} problem(s)", file=sys.stderr)
        return 1
    print(f"ok — {n} proposition citations in Rust source, all resolve")
    return 0


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "cited":
        return cited()
    lines = DOC.read_text().split("\n")

    props, errs = parse(lines)
    roots, e2 = build(props)
    errs += e2
    errs += check_refs(lines, props)

    orphans = [p for p in props if p.num is None]
    errs += [f"L{p.line}: #{p.slug} is unreachable from any root" for p in orphans]

    if errs:
        print("\n".join(f"  {e}" for e in errs), file=sys.stderr)
        print(f"\n{len(errs)} problem(s)", file=sys.stderr)
        return 1

    new = rewrite(lines, props)

    if cmd == "fmt":
        if new != lines:
            DOC.write_text("\n".join(new))
            moved = sum(1 for p in props if p.old != p.num)
            print(f"renumbered {moved} proposition(s); {len(props)} total")
        else:
            print(f"{len(props)} propositions, {len(roots)} roots — already current")
        return 0

    if new != lines:
        stale = [f"  L{p.line}: #{p.slug} is numbered {p.old}, should be {p.num}"
                 for p in props if p.old != p.num]
        print("\n".join(stale) or "  link texts are stale", file=sys.stderr)
        print("\nrun ./_props.py fmt", file=sys.stderr)
        return 1

    print(f"ok — {len(props)} propositions, {len(roots)} roots, numbering and references current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
