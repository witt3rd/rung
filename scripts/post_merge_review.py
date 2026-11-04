#!/usr/bin/env python3
"""The review council, after the merge: advisory, never a gate.

Rung lands straight to master (scripts/land-direct), so there is no pull request
for the council to review. This reviews what landed instead: the commits since the
last reviewed one (a cursor kept in one open issue labelled `post-merge-cursor`), or
an explicit range on request. One model call reads AGENTS.md, the council's principles
and the diff, and answers with findings. Every BLOCK and FIX finding becomes one issue
labelled `post-merge-review` (skipped when an open issue already has the title); NOTEs
only go in the run summary. Nothing here can stop a landing or fail master.

    scripts/post_merge_review.py [--range A..B] [--dry-run]

Environment: OPENROUTER_API_KEY (absent: the review is skipped with a notice), MODEL
(default anthropic/claude-sonnet-5), OPENROUTER_URL, GH (the gh binary), MAX_DIFF_BYTES.
Tests: scripts/test_post_merge_review.py.
"""
import json
import os
import subprocess
import sys
import urllib.request

LABEL = "post-merge-review"
CURSOR = "post-merge-cursor"
GH = os.environ.get("GH", "gh")
URL = os.environ.get("OPENROUTER_URL", "https://openrouter.ai/api/v1/chat/completions")
MODEL = os.environ.get("MODEL", "anthropic/claude-sonnet-5")
MAX_DIFF = int(os.environ.get("MAX_DIFF_BYTES", "150000"))
# Generated or machine-written paths are not reviewed.
SKIP = [":(exclude)Cargo.lock", ":(exclude)docs/conformance.md", ":(exclude)docs/*-props.md", ":(exclude)openwiki"]


def run(*cmd):
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""


def cursor():
    """(issue number, last reviewed sha) or (None, None)."""
    out = run(GH, "issue", "list", "--label", CURSOR, "--state", "open", "--json", "number,body", "--limit", "1")
    items = json.loads(out)
    if not items:
        return None, None
    for line in items[0]["body"].splitlines():
        if line.startswith("reviewed-through:"):
            return items[0]["number"], line.split(":", 1)[1].strip()
    return items[0]["number"], None


def save_cursor(number, sha):
    body = f"Advisory post-merge review (scripts/post_merge_review.py) has reviewed master up to this commit.\n\nreviewed-through: {sha}\n"
    if number is None:
        run(GH, "issue", "create", "--title", "post-merge review cursor", "--label", CURSOR, "--body", body)
    else:
        run(GH, "issue", "edit", str(number), "--body", body)


def build_prompt(rng):
    subjects = run("git", "log", "--format=%h %s", rng)
    diff = run("git", "diff", "--no-renames", rng, "--", ".", *SKIP)
    cut = ""
    if len(diff.encode()) > MAX_DIFF:
        diff = diff.encode()[:MAX_DIFF].decode(errors="ignore")
        cut = "\n[diff truncated: review what is shown and say so]\n"
    law = read("AGENTS.md") + "\n\n" + read(".github/review-council/principles.md")
    system = (
        "You are the post-merge review council for a repository whose work lands directly on master with no pull request. "
        "The repository's written law follows. Review only the change given, against that law and for defects. "
        "Answer with a JSON array and nothing else; each item {\"severity\":\"BLOCK|FIX|NOTE\",\"path\":str,\"title\":str,"
        "\"why\":str,\"do\":str}. BLOCK: breaks the law or a contract. FIX: should change. NOTE: informational. "
        "No findings: []. Quote no secret. The law says \"PR\" where there is now a landed commit range: read it so.\n\n--- LAW ---\n" + law
    )
    return system, f"Commits {rng}:\n{subjects}\n\nDiff:\n{diff}{cut}"


def ask(system, user, key):
    body = json.dumps({"model": MODEL, "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]}).encode()
    req = urllib.request.Request(URL, body, {"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=600) as r:
        text = json.load(r)["choices"][0]["message"]["content"].strip()
    if text.startswith("```"):
        text = text.split("\n", 1)[1].rsplit("```", 1)[0]
    return json.loads(text)


def file_issue(f, rng, dry):
    title = f"[post-merge] {f['title']}"[:120]
    if dry:
        print(f"would file: {title}")
        return
    open_ = json.loads(run(GH, "issue", "list", "--label", LABEL, "--state", "open", "--search", title, "--json", "title", "--limit", "20"))
    if any(i["title"] == title for i in open_):
        print(f"already open: {title}")
        return
    body = (f"**{f['severity']}** in `{f.get('path', '')}` (reviewed range `{rng}`)\n\n{f['why']}\n\n**Do:** {f['do']}\n\n"
            "_Advisory post-merge review (scripts/post_merge_review.py). Close this when fixed or when the finding is wrong._")
    run(GH, "issue", "create", "--title", title, "--label", LABEL, "--body", body)
    print(f"filed: {title}")


def main(argv):
    dry = "--dry-run" in argv
    rng = argv[argv.index("--range") + 1] if "--range" in argv else None
    head = run("git", "rev-parse", "HEAD").strip()
    number = None
    if rng is None:
        number, last = cursor()
        if not last:  # first run: the last 20 commits
            n = int(run("git", "rev-list", "--count", "HEAD").strip())
            last = run("git", "rev-parse", f"HEAD~{min(20, n - 1)}").strip()
        rng = f"{last}..{head}"
    if not run("git", "rev-list", rng).strip():
        print(f"nothing landed in {rng}")
        return 0
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        print("::notice::OPENROUTER_API_KEY is not set: post-merge review skipped")
        return 0
    findings = ask(*build_prompt(rng), key)
    for f in findings:
        print(f"{f.get('severity')} {f.get('path', '')}: {f.get('title')}")
        if f.get("severity") in ("BLOCK", "FIX"):
            file_issue(f, rng, dry)
    if not dry and "--range" not in argv:
        save_cursor(number, head)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
