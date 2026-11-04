#!/usr/bin/env python3
"""scripts/post_merge_review.py against a fake model server and a fake gh: findings become issues
(BLOCK/FIX only, once), the cursor advances, no key skips quietly, nothing landed is a no-op."""
import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "scripts", "post_merge_review.py")
FINDINGS = [
    {"severity": "BLOCK", "path": "a.txt", "title": "breaks the law", "why": "w", "do": "d"},
    {"severity": "NOTE", "path": "a.txt", "title": "just a note", "why": "w", "do": "d"},
]
FAKE_GH = """#!/usr/bin/env python3
import json, os, sys
st = os.environ["GH_STATE"]; a = sys.argv[1:]
log = open(st + ".log", "a"); log.write(json.dumps(a) + "\\n"); log.close()
if a[:2] == ["issue", "list"]:
    lab = a[a.index("--label") + 1]
    print(open(st + "." + lab).read() if os.path.exists(st + "." + lab) else "[]")
elif a[:2] == ["issue", "create"]:
    lab = a[a.index("--label") + 1]; t = a[a.index("--title") + 1]; b = a[a.index("--body") + 1]
    open(st + "." + lab, "w").write(json.dumps([{"number": 7, "title": t, "body": b}]))
elif a[:2] == ["issue", "edit"]:
    open(st + ".post-merge-cursor", "w").write(json.dumps([{"number": 7, "title": "c", "body": a[a.index("--body") + 1]}]))
"""


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        out = json.dumps({"choices": [{"message": {"content": json.dumps(FINDINGS)}}]}).encode()
        self.send_response(200)
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


class T(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
        self.env = {**os.environ, **env, "GH": os.path.join(self.d, "gh"), "GH_STATE": os.path.join(self.d, "st")}
        with open(self.env["GH"], "w") as f:
            f.write(FAKE_GH)
        os.chmod(self.env["GH"], 0o755)
        self.repo = os.path.join(self.d, "repo")
        os.mkdir(self.repo)
        self.git("init", "-q", "-b", "master")
        self.commit("one")
        self.server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.env["OPENROUTER_URL"] = f"http://127.0.0.1:{self.server.server_port}/"

    def tearDown(self):
        self.server.shutdown()

    def git(self, *a):
        return subprocess.run(["git", *a], cwd=self.repo, env=self.env, check=True, capture_output=True, text=True).stdout

    def commit(self, name):
        with open(os.path.join(self.repo, "a.txt"), "a") as f:
            f.write(name + "\n")
        self.git("add", "-A")
        self.git("commit", "-qm", name)

    def run_review(self, *args, key="k"):
        env = dict(self.env)
        if key:
            env["OPENROUTER_API_KEY"] = key
        return subprocess.run([sys.executable, SCRIPT, *args], cwd=self.repo, env=env, capture_output=True, text=True)

    def state(self, label):
        p = self.env["GH_STATE"] + "." + label
        return json.load(open(p)) if os.path.exists(p) else []

    def test_files_block_not_note_advances_cursor_and_is_idempotent(self):
        self.commit("two")
        r = self.run_review()
        self.assertEqual(r.returncode, 0, r.stderr)
        issues = self.state("post-merge-review")
        self.assertEqual([i["title"] for i in issues], ["[post-merge] breaks the law"])
        head = self.git("rev-parse", "HEAD").strip()
        self.assertIn(f"reviewed-through: {head}", self.state("post-merge-cursor")[0]["body"])
        r = self.run_review()  # nothing landed since the cursor
        self.assertIn("nothing landed", r.stdout)
        self.commit("three")
        r = self.run_review()  # same finding again: already open, not filed twice
        self.assertIn("already open", r.stdout)

    def test_no_key_skips(self):
        self.commit("two")
        r = self.run_review(key=None)
        self.assertEqual(r.returncode, 0)
        self.assertIn("skipped", r.stdout)
        self.assertEqual(self.state("post-merge-review"), [])

    def test_explicit_range_leaves_the_cursor_alone(self):
        self.commit("two")
        r = self.run_review("--range", "HEAD~1..HEAD")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(len(self.state("post-merge-review")), 1)
        self.assertEqual(self.state("post-merge-cursor"), [])

    def test_dry_run_files_nothing(self):
        self.commit("two")
        r = self.run_review("--dry-run")
        self.assertIn("would file", r.stdout)
        self.assertEqual(self.state("post-merge-review"), [])


if __name__ == "__main__":
    unittest.main()
