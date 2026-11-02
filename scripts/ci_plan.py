#!/usr/bin/env python3
"""Decide what CI must build and test for a change.

Pull requests run only the crates a change can affect: every workspace crate
that owns a changed file, plus all of its reverse dependencies (normal, build
and dev). Anything that is not a pull request (push to master, merge_group,
tags, manual runs) runs everything, so the speculative merge is always tested
in full. Anything the rules below do not recognise also runs everything:
the safe answer to an unknown path is "all of it".

Usage (what CI does):

    scripts/ci_plan.py --event pull_request --base <sha> --head <sha>

Dry run on a list of paths, one per line, `-` for stdin:

    printf 'docs/rung-host.md\\n' | scripts/ci_plan.py --event pull_request --files -

Prints `key=value` lines (append them to $GITHUB_OUTPUT):

    full      true when everything runs
    rust      true when any Rust job has work (false: only the always-on
              jobs run, the Rust jobs are skipped on purpose)
    scope     cargo package selection: `--workspace` or `-p a -p b ...`
    rung      true when the `rung` crate is in scope (its examples run)
    agents    true when the example agents (rung-agent, rung-host,
              rung-testkit) are in scope
    gui       true when the GUI workspace (`gui/`, outside the main
              workspace and the required `check`) has work: a file under
              `gui/` changed, or a crate its tests run (the gateway and the
              host binaries) is in scope
    packages  the affected crates, comma separated (empty when full)
    reason    one line saying why

Repository layout knowledge lives in the three tables below, nowhere else.
"""
import argparse
import fnmatch
import json
import subprocess
import sys

# Paths that change what every crate builds with. Any match runs everything.
GLOBAL = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".cargo/*",
    ".config/*",
    ".github/*",
    "scripts/ci_plan.py",
    "scripts/test_ci_plan.py",
    "scripts/test_tmp_clean.sh",
]

# Files outside any crate that a crate's tests, doctests or examples read.
# Keep in step with the tests: a test that reads a new root-level path adds a
# row here (a path with no row and no crate is treated as GLOBAL below).
EXTRA_INPUTS = [
    ("docs/*", ["rung-doctrine", "rung-driver"]),  # rendered props, triage, docket
    (".het/*", ["rung-doctrine", "rung-driver"]),  # carrier state read by tests
    ("examples/*", ["rung-host", "rung-agent"]),  # mock config, example agents
]

# Paths with no Rust input at all (checked by always-on jobs, or by nothing).
INERT = [
    "AGENTS.md",
    "CLAUDE.md",
    "CHANGELOG.md",
    "LICENSE*",
    ".gitignore",
    "deny.toml",  # the deny job always runs
    "skills/*",
    "data/*",
    "ui/*",
    ".archive/*",
    "scripts/*",  # Python probes and their tests; ci_plan/test_tmp_clean are GLOBAL
]

AGENT_CRATES = {"rung-agent", "rung-host", "rung-testkit"}

# The GUI workspace is its own cargo root, so `cargo metadata` of the main
# workspace does not list it: its files are matched by path. Its tests run the
# real binaries of these main-workspace crates.
GUI = ["gui/*"]
GUI_NEEDS = {"rung-gateway", "rung-host"}


def match(path, patterns):
    return any(fnmatch.fnmatchcase(path, p) for p in patterns)


def load_metadata(path):
    if path:
        with open(path) as f:
            return json.load(f)
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)


def workspace(meta):
    """Return ({crate: dir relative to the root}, {crate: set of crates it depends on})."""
    root = meta["workspace_root"].rstrip("/") + "/"
    dirs, deps = {}, {}
    pkgs = meta["packages"]
    names = {p["name"] for p in pkgs}
    for p in pkgs:
        manifest = p["manifest_path"]
        assert manifest.startswith(root), manifest
        d = manifest[len(root):].rsplit("/", 1)[0] if "/" in manifest[len(root):] else ""
        dirs[p["name"]] = d
        deps[p["name"]] = {d_["name"] for d_ in p["dependencies"] if d_["name"] in names}
    return dirs, deps


def reverse_closure(seed, deps):
    out = set(seed)
    grew = True
    while grew:
        grew = False
        for crate, ds in deps.items():
            if crate not in out and ds & out:
                out.add(crate)
                grew = True
    return out


def owner(path, dirs):
    """The crate whose directory holds `path` (longest directory wins)."""
    best = None
    for crate, d in dirs.items():
        if d and (path == d or path.startswith(d + "/")):
            if best is None or len(d) > len(dirs[best]):
                best = crate
    return best


def plan(event, files, meta):
    dirs, deps = workspace(meta)
    everything = {
        "full": "true", "rust": "true", "scope": "--workspace",
        "rung": "true", "agents": "true", "gui": "true", "packages": "",
    }
    if event != "pull_request":
        return {**everything, "reason": f"{event}: the full suite always runs"}
    seed = set()
    gui = False
    for f in files:
        f = f.strip()
        if not f:
            continue
        if match(f, GLOBAL):
            return {**everything, "reason": f"{f} affects every crate"}
        if match(f, GUI):
            gui = True
            continue
        crate = owner(f, dirs)
        if crate:
            seed.add(crate)
        elif match(f, [p for p, _ in EXTRA_INPUTS]):
            for pat, crates in EXTRA_INPUTS:
                if match(f, [pat]):
                    seed.update(c for c in crates if c in dirs)
        elif match(f, INERT):
            pass
        else:
            return {**everything, "reason": f"{f} is not a known path: run everything"}
    affected = reverse_closure(seed, deps)
    ordered = sorted(affected)
    if len(affected) == len(dirs):
        return {**everything, "reason": "the change reaches every crate"}
    return {
        "full": "false",
        "rust": "true" if affected else "false",
        "scope": " ".join(f"-p {c}" for c in ordered),
        "rung": "true" if "rung" in affected else "false",
        "agents": "true" if affected & AGENT_CRATES else "false",
        "gui": "true" if gui or affected & GUI_NEEDS else "false",
        "packages": ",".join(ordered),
        "reason": (
            f"changed {', '.join(sorted(seed))}; with reverse dependencies: {', '.join(ordered)}"
            if affected else "no Rust input changed"
        ),
    }


def changed_files(base, head):
    return subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", f"{base}...{head}"],
        check=True, capture_output=True, text=True,
    ).stdout.splitlines()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--event", required=True, help="github.event_name")
    ap.add_argument("--base")
    ap.add_argument("--head")
    ap.add_argument("--files", help="file of changed paths, `-` for stdin (dry run)")
    ap.add_argument("--metadata", help="`cargo metadata --no-deps` JSON (default: run cargo)")
    a = ap.parse_args(argv)
    if a.files:
        src = sys.stdin if a.files == "-" else open(a.files)
        files = src.read().splitlines()
    elif a.event == "pull_request":
        if not (a.base and a.head):
            ap.error("pull_request needs --base and --head, or --files")
        files = changed_files(a.base, a.head)
    else:
        files = []
    result = plan(a.event, files, load_metadata(a.metadata))
    for k in ("full", "rust", "scope", "rung", "agents", "gui", "packages", "reason"):
        print(f"{k}={result[k]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
