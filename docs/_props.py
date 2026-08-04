#!/usr/bin/env python3
"""Numbering and reference integrity for the proposition documents.

The decimal number of a proposition is *derived*, never authored. Identity is
the slug in the anchor; hierarchy is `data-parent`; order is document order.

    <a id="classifier-not-judgmental" data-parent="conditional-names-classifier"></a>
    **2.51** The classifying sentence is not itself judgmental. ...

References are ordinary markdown links whose text is derived and whose target
is the stable slug:

    reopens the regress [6.4](#tower-floor) closes

A root may declare `data-numbering="X"`, in which case its children are
numbered `X1..Xn` — flat rather than concatenated — instead of `n.1..n.m`:

    <a id="guarantees" data-numbering="G"></a>
    <a id="g2-sealed-construction" data-parent="guarantees"></a>
    **G2** **Sealed construction.** A rung MUST NOT be constructible ...

The number is still derived: the letter is the root's, the index is document
order. Use it only where a label is already cited from outside the documents
(rung's `G1`-`G14` are named in Rust comments and in trybuild test filenames,
so renumbering them would break citations a slug cannot carry).

There may be several normative documents (see DOC_NAMES). **Numbering is per
document** — each numbers its roots from 1 independently — but **slugs are
global**: a slug is unique across the whole set, and a reference resolves
against the union. A reference into another document names the file:

    the floor is fixed [6.4](rung-ct-props.md#tower-floor)

    ./_props.py check    exit 1 on any integrity failure; changes nothing
    ./_props.py cited    exit 1 if Rust source cites a slug that is not a proposition
"""

import re
import sys
from pathlib import Path

# The normative proposition documents, in the order they are checked. A name
# that is not on disk yet is skipped, so a second document can be added here
# before it is written.
DOC_NAMES = (
    "rung-props.md",
    "rung-het-props.md",
    "rung-ct-props.md",
)


def docs():
    """The proposition documents that exist, in DOC_NAMES order."""
    here = Path(__file__).parent
    return [p for p in (here / n for n in DOC_NAMES) if p.exists()]


ANCHOR = re.compile(
    r'^<a id="([a-z0-9-]+)"'
    r'(?: data-parent="([a-z0-9-]+)")?'
    r'(?: data-numbering="([A-Z])")?></a>$'
)
# A number is derived: either decimal (1, 1.2, 1.23) or, under a root that
# declares `data-numbering`, that root's letter followed by an index (G1, G14).
PROP = re.compile(r"^\*\*([A-Z]?\d+(?:\.\d+)?)\*\*")
HEADING = re.compile(r"^## (?:\d+(?:\.\d+)? · )?(.*)$")
REF = re.compile(r"\[([^\]]*)\]\(([A-Za-z0-9._-]*)#([a-z0-9-]+)\)")
MATH = re.compile(r"\$\$.*?\$\$|\$[^$]*\$")


class Prop:
    __slots__ = ("slug", "parent", "line", "old", "num", "kids", "doc", "label")

    def __init__(self, slug, parent, line, old, doc="", label=None):
        self.slug, self.parent, self.line, self.old = slug, parent, line, old
        self.num, self.kids, self.doc, self.label = None, [], doc, label


def parse(lines, doc=""):
    """Return (props in document order, errors) for one document."""
    props, errs, seen = [], [], {}
    for i, line in enumerate(lines):
        m = ANCHOR.match(line)
        if not m:
            continue
        slug, parent, label = m.group(1), m.group(2), m.group(3)
        nxt = lines[i + 1] if i + 1 < len(lines) else ""
        p = PROP.match(nxt)
        if not p:
            errs.append(f"L{i+1}: anchor #{slug} is not followed by a **N** proposition line")
            continue
        if slug in seen:
            errs.append(f"L{i+1}: duplicate id #{slug} (first at L{seen[slug]})")
            continue
        seen[slug] = i + 1
        props.append(Prop(slug, parent, i + 1, p.group(1), doc, label))
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
        # A labelled root numbers its children `X1..Xn` — a flat scheme, so the
        # past-9 ambiguity the concatenating scheme has does not arise.
        if node.label:
            for i, kid in enumerate(node.kids, 1):
                number(kid, f"{node.label}{i}", 0)
            return
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


def rewrite(lines, props, universe=None):
    """Return new lines with numbers and link texts recomputed.

    **Nothing writes these any more.** The documents are generated from
    `rung-doctrine/src/`, and this exists so `check` can compare — an
    independent second implementation of the numbering, built from the rendered
    markdown rather than from the encoding. Two derivations agreeing on all 380
    numbers is a stronger statement than either alone, which is why this is kept
    rather than deleted along with the writer it used to serve.

    `props` are this document's propositions; `universe` is every proposition
    of every document, against which a reference's link text is resolved.
    """
    by_slug = {p.slug: p for p in (universe if universe is not None else props)}
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
            target = by_slug.get(m.group(3))
            if not target:
                return m.group(0)
            # a reference into another document keeps that document's name
            file = "" if target.doc == props[0].doc else target.doc
            return f"[{target.num}]({file}#{target.slug})"
        out[i] = REF.sub(sub, line)
    return out


def check_refs(lines, props, universe=None):
    """`props` is this document; `universe` is every document's propositions."""
    universe = universe if universe is not None else props
    by_slug = {p.slug: p for p in universe}
    here = props[0].doc if props else ""
    errs = []
    for i, line in enumerate(lines):
        if ANCHOR.match(line):
            continue
        for m in REF.finditer(line):
            target = by_slug.get(m.group(3))
            if target is None:
                errs.append(f"L{i+1}: reference to #{m.group(3)}, which is not a proposition")
                continue
            want = "" if target.doc == here else target.doc
            if m.group(2) != want:
                errs.append(
                    f"L{i+1}: reference to #{target.slug} names "
                    f"`{m.group(2) or '(this document)'}`; it is in "
                    f"`{want or '(this document)'}`"
                )
    # a bare decimal outside math and outside a link is almost certainly a
    # hand-written reference that was never converted. The trailing guard
    # rejects only a longer number (1.234, 1.2.3) or a word — not sentence
    # punctuation, which is where such a reference most often sits.
    for i, line in enumerate(lines):
        if ANCHOR.match(line):
            continue
        bare = strip_math(REF.sub(lambda m: " " * len(m.group(0)), line))
        bare = PROP.sub(lambda m: " " * len(m.group(0)), bare)
        for m in re.finditer(r"(?<![\w.#/§-])(\d{1,2}\.\d{1,2})(?![\w-]|\.\d)", bare):
            if m.group(1) in {p.num for p in props}:
                errs.append(f"L{i+1}: bare number {m.group(1)} — write it as a [n](#slug) link")
    return errs


# Slug-shaped tokens that are deliberately NOT propositions: variants a
# document names in order to refuse them. A ban has to say what it bans.
RETIRED = {"accept-with-mod", "reject-with-alternative"}

# Concepts Het replaced outright. A retired term must not reappear in a
# normative document — not in a structural position, not in prose. Each was at
# zero occurrences when the ban was added; a hit is a regression, not a legacy.
# `role`, `presentation` and `element` are NOT here: all three have legitimate
# current uses (a competence role; the presentation of a free category; ordinary
# set membership), and banning them would be false positives.
RETIRED_TERMS = {
    "register": "a theory — an algebra carrying its own signature",
    "registry": "a theory; Het replaces the concept entirely",
    "charter": "\u03c7, the belonging-law, declared IN the signature",
    "finding": "a subject; auditability is carried by the fractal property",
}

# Normative documents the vocabulary ban covers, beyond the proposition set.
ALSO_NORMATIVE = ("rung-props.md",)


def retired_terms():
    """No normative document and no source file may use a term Het retired.

    Source is in scope because a retired concept spreads through identifiers,
    doc comments and message strings, not only through prose — and a ban that
    covers the one place a term is already absent enforces nothing.
    """
    here = Path(__file__).parent
    root = here.parent
    seen = docs()
    targets = seen + [
        here / n
        for n in ALSO_NORMATIVE
        if (here / n).exists() and (here / n) not in seen
    ]
    for crate in CITING:
        targets += [
            src
            for src in sorted((root / crate).rglob("*.rs"))
            if "target" not in src.parts
        ]
    errs = []
    for path in targets:
        for i, line in enumerate(path.read_text().split("\n")):
            for term, instead in RETIRED_TERMS.items():
                if re.search(rf"\b{term}", line, re.I):
                    where = (
                        path.name
                        if path.parent == here
                        else path.relative_to(here.parent)
                    )
                    errs.append(
                        f"{where}: L{i+1}: `{term}` is retired — use {instead}"
                    )
    return errs

# Crates that cite the propositions by slug. Scoped deliberately: elsewhere in
# the workspace a hyphenated token is ordinary prose, not a citation.
CITING = ("rung", "rung-het", "rung-std")

# Slug-shaped tokens that are wire identifiers, not citations. Kept as an
# explicit list rather than a pattern: every entry is a name some protocol
# chose, and the alternative is degrading the documentation to satisfy a
# regex.
NOT_A_CITATION = {"x-api-key"}

COMMENT = re.compile(r"^\s*(?://[/!]?)(.*)$")
SLUGLIKE = re.compile(r"\b[a-z][a-z0-9]*(?:-[a-z0-9]+){2,}\b")


def load():
    """Parse every document.

    Returns (records, universe, errors), where a record is
    (path, lines, props) and `universe` is every document's propositions.
    Slugs are global, so a slug used twice is an error even across documents.
    """
    recs, universe, errs = [], [], []
    for path in docs():
        lines = path.read_text().split("\n")
        props, e = parse(lines, path.name)
        errs += [f"{path.name}: {x}" for x in e]
        recs.append((path, lines, props))
        universe += props
    seen = {}
    for p in universe:
        where = f"{p.doc}:L{p.line}"
        if p.slug in seen:
            errs.append(f"{where}: duplicate id #{p.slug} (also {seen[p.slug]})")
        else:
            seen[p.slug] = where
    return recs, universe, errs


def cited():
    """Every slug-shaped token in a Rust comment must be a proposition."""
    _, universe, _ = load()
    slugs = {p.slug for p in universe}
    # a comment that names a proposition document is citing the file, not a
    # proposition — and the file names are themselves slug-shaped
    ignore = RETIRED | NOT_A_CITATION | {Path(name).stem for name in DOC_NAMES}
    root = Path(__file__).parent.parent
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
                elif tok not in ignore:
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

    # `fmt` is retired, and is named here rather than falling through to
    # `check`. Silently checking when someone asked to renumber would report
    # success for work that did not happen — and the work no longer exists to
    # do, because these documents are written by `render`.
    if cmd == "fmt":
        print("`fmt` is retired. These documents are generated from "
              "rung-doctrine/src/;\nnumbers are derived at render time and "
              "there is nothing here to renumber.\n\n"
              "  cargo run -p rung-doctrine --bin render", file=sys.stderr)
        return 2
    if cmd not in ("check", "cited"):
        print(f"unknown command `{cmd}` — expected `check` or `cited`", file=sys.stderr)
        return 2
    if cmd == "cited":
        return cited()

    recs, universe, errs = load()
    errs += retired_terms()
    if not recs:
        print(f"no proposition document found — expected one of {', '.join(DOC_NAMES)}",
              file=sys.stderr)
        return 1

    # numbering is PER DOCUMENT: each numbers its own roots from 1
    roots_of = {}
    for path, lines, props in recs:
        roots, e2 = build(props)
        roots_of[path.name] = roots
        errs += [f"{path.name}: {x}" for x in e2]
        errs += [f"{path.name}: {x}" for x in check_refs(lines, props, universe)]
        errs += [f"{path.name}: L{p.line}: #{p.slug} is unreachable from any root"
                 for p in props if p.num is None]

    if errs:
        print("\n".join(f"  {e}" for e in errs), file=sys.stderr)
        print(f"\n{len(errs)} problem(s)", file=sys.stderr)
        return 1

    stale, total, nroots = [], 0, 0
    for path, lines, props in recs:
        total += len(props)
        nroots += len(roots_of[path.name])
        new = rewrite(lines, props, universe)
        if new == lines:
            continue
        moved = [f"  {path.name}: L{p.line}: #{p.slug} is numbered {p.old}, "
                 f"should be {p.num}" for p in props if p.old != p.num]
        stale += moved or [f"  {path.name}: link texts are stale"]

    n = len(recs)
    if stale:
        print("\n".join(stale), file=sys.stderr)
        print("\nThese documents are generated. Fix the encoding in "
              "rung-doctrine/src/ and run:\n"
              "  cargo run -p rung-doctrine --bin render", file=sys.stderr)
        return 1

    print(f"ok — {total} propositions, {nroots} roots across {n} document(s), "
          f"numbering and references current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
