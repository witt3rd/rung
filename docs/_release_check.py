#!/usr/bin/env python3
"""Release hardening checks. Never publishes anything.

  _release_check.py [TAG]   TAG (e.g. v0.2.0) is compared to the workspace version.

Checks: tag == workspace version (when TAG given); every workspace crate that
depends (by path) on a publish=false crate is itself publish=false.
Cargo.lock currency and the release build are checked by the CI job.
"""
import json, subprocess, sys

meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"]))
pkgs = {p["name"]: p for p in meta["packages"]}
bad = []

if len(sys.argv) > 1:
    tag = sys.argv[1]
    import tomllib
    with open("Cargo.toml", "rb") as f:
        ver = tomllib.load(f)["workspace"]["package"]["version"]
    if tag != f"v{ver}":
        bad.append(f"tag {tag} != workspace version v{ver}")

unpublished = {n for n, p in pkgs.items() if p["publish"] == []}
for n, p in pkgs.items():
    if n in unpublished:
        continue
    for d in p["dependencies"]:
        if d["name"] in unpublished and d.get("path"):
            bad.append(f"{n} depends on publish=false {d['name']} but is not publish=false")

if bad:
    print("\n".join(bad), file=sys.stderr)
    sys.exit(1)
print("release check ok; publish=false:", ", ".join(sorted(unpublished)))
