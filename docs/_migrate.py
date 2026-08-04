#!/usr/bin/env python3
"""Read a props document and emit its `Doctrine` declaration.

**Migration tooling.** It runs once per document, to lift 380 propositions out
of prose and into a form that can be type-checked. After a document is encoded
the markdown becomes a render target and this script has no further job.

It makes no decisions it cannot justify from the text: a proposition's `kind` is
not something markdown records, so every proposition is emitted as `Rationale`
and the triage — which of these are claims, which declare signature — is done
afterwards, deliberately, by someone reading them. Guessing here would
manufacture 380 classifications nobody made.
"""

import re
import sys
from pathlib import Path

HERE = Path(__file__).parent
ANCHOR = re.compile(
    r'^<a id="([a-z0-9-]+)"'
    r'(?: data-parent="([a-z0-9-]+)")?'
    r'(?: data-numbering="([A-Z])")?></a>$'
)
PROP = re.compile(r"^\*\*([A-Z]?\d+(?:\.\d+)?)\*\* ?")
REF = re.compile(r"\[([^\]]*)\]\(([A-Za-z0-9._-]*)#([a-z0-9-]+)\)")


def rust_str(s):
    """A Rust raw string with enough hashes to contain the body."""
    n = 1
    while ('"' + "#" * n) in s:
        n += 1
    h = "#" * n
    return f'r{h}"{s}"{h}'


def migrate(path):
    """Split a document into elements whose concatenation is the document.

    The invariant every element obeys: its text is `"\n".join(its lines) +
    "\n"`. Concatenating the elements in order therefore reproduces the file
    exactly, which is what makes the round trip checkable rather than
    approximate.
    """
    text = path.read_text()
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines = lines[:-1]          # the file's final newline, re-added per element

    elements = []
    buf = []
    i = 0
    while i < len(lines):
        m = ANCHOR.match(lines[i])
        p = PROP.match(lines[i + 1]) if m and i + 1 < len(lines) else None
        if not (m and p):
            buf.append(lines[i])
            i += 1
            continue
        if buf:
            elements.append(("verbatim", buf))
            buf = []
        body = [lines[i + 1][p.end():]]
        j = i + 2
        # A proposition ends at the next anchor — or at a section boundary. A
        # `---` rule and a `## heading` belong to the document, not to the
        # proposition that happens to precede them; absorbing them would inflate
        # coverage with prose the encoding has not actually structured.
        while (
            j < len(lines)
            and not ANCHOR.match(lines[j])
            and lines[j] != "---"
            and not lines[j].startswith("## ")
        ):
            body.append(lines[j])
            j += 1
        elements.append(("prop", m.group(1), m.group(2), m.group(3), body))
        i = j
    if buf:
        elements.append(("verbatim", buf))

    # References become {#slug}: the link text is a number, and a number is
    # derived. Storing the text would reintroduce the thing being removed.
    def deref(prose):
        return REF.sub(lambda m: "{#" + m.group(3) + "}", prose)

    out = []
    for e in elements:
        if e[0] == "verbatim":
            out.append(f"        Element::Verbatim({rust_str(chr(10).join(e[1]) + chr(10))}),")
        else:
            _, slug, parent, letter, body = e
            prose = "\n".join(body) + "\n"
            parent_r = f'Some("{parent}")' if parent else "None"
            letter_r = f"Some('{letter}')" if letter else "None"
            out.append("        Element::Prop(Prop {")
            out.append(f'            slug: "{slug}",')
            out.append(f"            parent: {parent_r},")
            out.append("            kind: Kind::Rationale,")
            out.append(f"            numbering: {letter_r},")
            out.append(f"            prose: {rust_str(deref(prose))},")
            out.append("        }),")

    n_props = sum(1 for e in elements if e[0] == "prop")
    n_verb = sum(1 for e in elements if e[0] == "verbatim")
    body = "\n".join(out)
    return n_props, n_verb, f"""//! The categorical account, encoded.
//!
//! **Generated once** from `docs/{path.name}` by `docs/_migrate.py`, and the
//! source of truth from then on. The markdown is rendered from this; where the
//! two disagree, this is right and the markdown is stale.
//!
//! Every proposition arrives as [`Kind::Rationale`], which is not a claim that
//! they are all arguments — it is the absence of a claim. Markdown does not
//! record what kind a proposition is, so the migration does not invent one. The
//! triage into signature, decidable and judgmental is a reading, done
//! deliberately, and it is the work this encoding exists to make possible.

use crate::{{Doctrine, Element, Kind, Prop}};

/// The categorical account of what a `ladder` declaration is.
pub fn doctrine() -> Doctrine {{
    Doctrine {{
        file: "{path.name}",
        elements: vec![
{body}
        ],
    }}
}}
"""


if __name__ == "__main__":
    name = sys.argv[1] if len(sys.argv) > 1 else "rung-ct-props.md"
    src = HERE / name
    n_props, n_verb, text = migrate(src)
    dest = HERE.parent / "rung-doctrine" / "src" / (name.replace("-props.md", "").replace("-", "_") + ".rs")
    dest.write_text(text)
    print(f"{src.name}: {n_props} propositions, {n_verb} verbatim block(s) -> {dest.relative_to(HERE.parent)}")
