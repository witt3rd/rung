#!/usr/bin/env python3
"""Link and path guard: a document must not point at something that is not there.

Checks, over every tracked `.md` (not rung-host/live/runs, not the generated openwiki/, not node_modules):
  1. relative links `[t](path)` resolve to a tracked file or directory;
  2. `#anchor` fragments resolve to a heading (GitHub slug) or `<a id>`/`<a name>`;
  3. backtick repo paths (`dir/file.ext`, first segment a tracked top-level
     entry; root- or document-relative) exist; historical text (docs/records,
     docs/reports, docs/.archive) is exempt;
  4. documentation paths cited in tracked .rs/.py/.yml files exist;
  5. no hand-kept test counts of the form `(N tests)` in living documents
     (they drift on every test added; link to the test file instead).
Known problems still to fix are listed in docs/_links.allow, one
`path: message-prefix  # reason` per line; the list only shrinks.
Run locally:  docs/_links.py      Self-test:  scripts/test_links.py
"""
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
SKIP_DOCS = ("rung-host/live/runs/", "openwiki/")  # openwiki/ is generated
SKIP_PARTS = ("node_modules/",)
HISTORICAL = ("docs/records/", "docs/reports/", "docs/.archive/", ".archive/")
CITING = (".rs", ".py", ".yml", ".yaml")

LINK = re.compile(r"(?<!\!)\[[^\]\n]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
IMAGE = re.compile(r"!\[[^\]\n]*\]\(([^)\s]+)\)")
TICK = re.compile(r"`([^`\n]+)`")
PATHISH = re.compile(r"^[A-Za-z0-9_.][A-Za-z0-9_./-]*$")
EXT = re.compile(r"\.[A-Za-z0-9]{1,8}$")
DOCREF = re.compile(r"docs/[A-Za-z0-9_./-]+\.md")
COUNT = re.compile(r"\(\d+ tests?\b[,)]")
ANCHOR_TAG = re.compile(r"""<a\s+(?:id|name)=["']([^"']+)["']""")
FENCE = re.compile(r"^\s*(```|~~~)")


def tracked(root):
    out = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode()
    return [f for f in out.split("\0") if f and (root / f).exists()]


def skipped(f):
    return any(f.startswith(p) or p in f for p in SKIP_DOCS + SKIP_PARTS)


def slug(heading):
    h = re.sub(r"<[^>]+>", "", heading).strip().lower()
    h = re.sub(r"[`*_\[\]()]", lambda m: "_" if m.group() == "_" else "", h)
    h = re.sub(r"[^\w\- ]", "", h)
    return h.replace(" ", "-")


def anchors(text):
    found, seen, fenced = set(), {}, False
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
        if fenced:
            continue
        m = re.match(r"#{1,6}\s+(.*?)\s*#*\s*$", line)
        if m:
            s = slug(m.group(1))
            n = seen.get(s, 0)
            seen[s] = n + 1
            found.add(s if n == 0 else f"{s}-{n}")
        found.update(ANCHOR_TAG.findall(line))
    return found


def strip_fences(text):
    out, fenced = [], False
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
            out.append("")
        else:
            out.append("" if fenced else line)
    return out


def check(root, files=None):
    """Return a list of `path:line: message` problems."""
    files = tracked(root) if files is None else files
    fileset = set(files)
    dirs = {str(p) for f in fileset for p in Path(f).parents if str(p) != "."}
    tops = {Path(f).parts[0] for f in fileset}
    live = [f for f in files if not skipped(f)]
    probs, cache = [], {}

    def exists(rel):
        rel = str(Path(rel))
        return rel in fileset or rel in dirs

    def anchor_set(rel):
        if rel not in cache:
            cache[rel] = anchors((root / rel).read_text(errors="replace"))
        return cache[rel]

    for f in live:
        if f.endswith(".md"):
            text = (root / f).read_text(errors="replace")
            lines = strip_fences(text)
            base = Path(f).parent
            hist = f.startswith(HISTORICAL)
            for n, line in enumerate(lines, 1):
                prose = re.sub(r"`[^`\n]*`", "", line)
                for target in LINK.findall(prose) + IMAGE.findall(prose):
                    if re.match(r"^([a-z][a-z0-9+.-]*:|//)", target):
                        continue
                    path, _, frag = target.partition("#")
                    path = unquote(path)
                    if path:
                        rel = str((base / path.lstrip("/")) if not path.startswith("/") else path.lstrip("/"))
                        rel = str(Path(rel.replace("\\", "/")))
                        parts = []
                        for seg in rel.split("/"):
                            if seg == "..":
                                if parts:
                                    parts.pop()
                            elif seg not in ("", "."):
                                parts.append(seg)
                        rel = "/".join(parts)
                        if not exists(rel):
                            probs.append(f"{f}:{n}: broken link: {target}")
                            continue
                    else:
                        rel = f
                    if frag and rel.endswith(".md") and rel in fileset:
                        if unquote(frag) not in anchor_set(rel):
                            probs.append(f"{f}:{n}: broken anchor: {target}")
                if hist:
                    continue
                if COUNT.search(line):
                    probs.append(f"{f}:{n}: hand-kept test count (delete it): {COUNT.search(line).group()}")
                for tok in TICK.findall(line):
                    tok = tok.strip().rstrip(",.;:")
                    if "/" not in tok or not PATHISH.match(tok) or "//" in tok:
                        continue
                    tok = tok.rstrip("/")
                    if tok.split("/")[0] not in tops or not (EXT.search(tok) or tok in dirs):
                        continue
                    if not exists(tok) and not exists(str(base / tok)):
                        probs.append(f"{f}:{n}: backtick path does not exist: {tok}")
        if f.endswith(CITING) and not f.startswith("scripts/test_"):
            for n, line in enumerate((root / f).read_text(errors="replace").splitlines(), 1):
                for ref in DOCREF.findall(line):
                    if not exists(ref):
                        probs.append(f"{f}:{n}: cites missing document: {ref}")
    return probs


def allowed(root):
    out = []
    p = root / "docs/_links.allow"
    if p.exists():
        for line in p.read_text().splitlines():
            entry = line.split("#", 1)[0].strip()
            if entry:
                out.append(entry)
    return out


def unallowed(root, probs):
    allow = [a.split(": ", 1) for a in allowed(root)]
    return [p for p in probs
            if not any(p.startswith(path + ":") and p.split(": ", 1)[1].startswith(msg) for path, msg in allow)]


def main():
    probs = unallowed(ROOT, check(ROOT))
    for p in probs:
        print(p)
    if probs:
        print(f"links: {len(probs)} problem(s)", file=sys.stderr)
        return 1
    print("links: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
