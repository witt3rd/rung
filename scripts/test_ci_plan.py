"""Tests for ci_plan.py. Run: python3 scripts/test_ci_plan.py

The graph tests use a synthetic workspace so they never go stale; the last
class checks the rules against the real workspace (needs cargo)."""
import io
import os
import shutil
import sys
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, os.path.dirname(__file__))
import ci_plan as c  # noqa: E402

ROOT = "/w"


def pkg(name, deps=(), dev=()):
    return {
        "name": name,
        "manifest_path": f"{ROOT}/{name}/Cargo.toml",
        "dependencies": [{"name": d, "kind": None} for d in deps]
        + [{"name": d, "kind": "dev"} for d in dev]
        + [{"name": "serde", "kind": None}],  # an external crate is ignored
    }


# base <- mid <- top <- app ; leaf <- app ; kit is a dev-dependency of mid.
META = {
    "workspace_root": ROOT,
    "packages": [
        pkg("base"), pkg("kit"), pkg("leaf"),
        pkg("mid", ["base"], dev=["kit"]),
        pkg("top", ["mid"]),
        pkg("app", ["top", "leaf"]),
    ],
}


def run(event, files):
    return c.plan(event, files, META)


class Selection(unittest.TestCase):
    def test_everything_but_a_pull_request_runs_everything(self):
        for ev in ("push", "merge_group", "workflow_dispatch", "schedule"):
            r = run(ev, ["docs/x.md"])  # even a docs-only change
            self.assertEqual((r["full"], r["scope"]), ("true", "--workspace"), ev)

    def test_leaf_crate_runs_only_itself_and_its_dependents(self):
        r = run("pull_request", ["leaf/src/lib.rs"])
        self.assertEqual(r["packages"], "app,leaf")
        self.assertEqual(r["scope"], "-p app -p leaf")
        self.assertEqual(r["full"], "false")

    def test_core_crate_reaches_through_dev_dependencies_and_chains(self):
        self.assertEqual(run("pull_request", ["kit/src/lib.rs"])["packages"], "app,kit,mid,top")
        self.assertEqual(run("pull_request", ["mid/tests/t.rs"])["packages"], "app,mid,top")

    def test_a_change_that_reaches_every_crate_is_full(self):
        r = run("pull_request", ["base/src/lib.rs", "kit/src/lib.rs", "leaf/x.rs"])
        self.assertEqual((r["full"], r["scope"]), ("true", "--workspace"))

    def test_global_files_run_everything(self):
        for f in ("Cargo.lock", "Cargo.toml", ".github/workflows/ci.yml",
                  ".cargo/config.toml", "scripts/ci_plan.py", "scripts/test_tmp_clean.sh",
                  "rust-toolchain.toml"):
            self.assertEqual(run("pull_request", [f, "leaf/a.rs"])["full"], "true", f)

    def test_inert_files_run_no_rust(self):
        r = run("pull_request", ["AGENTS.md", "CHANGELOG.md", "skills/x/SKILL.md",
                                 "scripts/shadow_audit.py", "ui/src/a.ts"])
        self.assertEqual((r["rust"], r["full"], r["scope"]), ("false", "false", ""))

    def test_a_crates_agents_file_does_not_rebuild_the_crate(self):
        r = run("pull_request", ["base/AGENTS.md", "leaf/AGENTS.md"])
        self.assertEqual((r["rust"], r["full"], r["scope"]), ("false", "false", ""))

    def test_unknown_path_runs_everything(self):
        self.assertEqual(run("pull_request", ["brand-new-dir/x"])["full"], "true")

    def test_a_crate_beats_a_prefix_sibling(self):
        meta = {"workspace_root": ROOT, "packages": [pkg("a"), pkg("a-b", ["a"]), pkg("z")]}
        r = c.plan("pull_request", ["a-b/src/x.rs"], meta)
        self.assertEqual(r["packages"], "a-b")

    def test_extra_inputs_name_real_crates_only(self):
        r = c.plan("pull_request", ["docs/x.md"], META)  # no such crates here: nothing seeded
        self.assertEqual(r["rust"], "false")

    def test_flags(self):
        meta = {"workspace_root": ROOT, "packages": [
            pkg("rung"), pkg("rung-testkit"), pkg("rung-agent", ["rung"], dev=["rung-testkit"]),
            pkg("other", ["rung"]), pkg("solo")]}
        agents = c.plan("pull_request", ["rung-testkit/src/lib.rs"], meta)
        self.assertEqual((agents["agents"], agents["rung"]), ("true", "false"))
        solo = c.plan("pull_request", ["solo/src/lib.rs"], meta)
        self.assertEqual((solo["agents"], solo["rung"]), ("false", "false"))


class Gui(unittest.TestCase):
    """The GUI workspace is outside the main one: matched by path, never an
    unknown path, and it builds no main-workspace crate."""

    def test_gui_files_run_only_the_gui_job(self):
        r = run("pull_request", ["gui/rung-client/src/lib.rs", "gui/Cargo.toml"])
        self.assertEqual((r["gui"], r["rust"], r["full"], r["scope"]), ("true", "false", "false", ""))

    def test_gui_files_leave_the_other_crates_alone(self):
        r = run("pull_request", ["gui/rung-ui/src/main.rs", "leaf/src/lib.rs"])
        self.assertEqual((r["gui"], r["packages"]), ("true", "app,leaf"))

    def test_other_changes_do_not_run_it(self):
        for f in ("leaf/src/lib.rs", "AGENTS.md", "ui/src/a.ts"):
            self.assertEqual(run("pull_request", [f])["gui"], "false", f)

    def test_the_binaries_its_tests_run_do(self):
        meta = {"workspace_root": ROOT, "packages": [
            pkg("core"), pkg("rung-host", ["core"]), pkg("rung-gateway", ["rung-host"]),
            pkg("solo")]}
        for f in ("rung-gateway/src/a.rs", "rung-host/src/a.rs", "core/src/a.rs"):
            self.assertEqual(c.plan("pull_request", [f], meta)["gui"], "true", f)
        self.assertEqual(c.plan("pull_request", ["solo/src/a.rs"], meta)["gui"], "false")

    def test_global_files_and_every_other_event_run_it(self):
        self.assertEqual(run("pull_request", ["Cargo.lock"])["gui"], "true")
        for ev in ("push", "merge_group", "workflow_dispatch"):
            self.assertEqual(run(ev, ["AGENTS.md"])["gui"], "true", ev)


class Cli(unittest.TestCase):
    def test_dry_run_prints_key_value_lines(self):
        import json
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            m, f = os.path.join(d, "m.json"), os.path.join(d, "f")
            json.dump(META, open(m, "w"))
            open(f, "w").write("leaf/src/lib.rs\n")
            out = io.StringIO()
            with redirect_stdout(out):
                c.main(["--event", "pull_request", "--files", f, "--metadata", m])
            self.assertIn("scope=-p app -p leaf\n", out.getvalue())
            self.assertIn("full=false\n", out.getvalue())


@unittest.skipUnless(shutil.which("cargo"), "needs cargo")
class RealWorkspace(unittest.TestCase):
    """The three example diffs the CI design is held to, on the real graph."""

    @classmethod
    def setUpClass(cls):
        cls.meta = c.load_metadata(None)

    def plan(self, *files):
        return c.plan("pull_request", list(files), self.meta)

    def test_prose_only_builds_nothing(self):
        r = self.plan("AGENTS.md", "CHANGELOG.md")
        self.assertEqual((r["rust"], r["scope"]), ("false", ""))

    def test_spec_docs_run_only_the_crates_that_read_them(self):
        r = self.plan("docs/reference/rung-host.md")
        self.assertEqual((r["full"], r["rung"], r["agents"]), ("false", "false", "false"))
        for crate in ("rung-doctrine", "rung-driver"):
            self.assertIn(crate, r["packages"].split(","))
        self.assertNotIn("rung-host", r["packages"].split(","))

    def test_a_leaf_crate(self):
        r = self.plan("rung-gateway/src/lib.rs")
        self.assertEqual(r["packages"], "rung-gateway")
        self.assertEqual((r["full"], r["rung"], r["agents"]), ("false", "false", "false"))

    def test_a_core_crate_runs_everything_that_needs_it(self):
        r = self.plan("rung/src/lib.rs")
        pk = r["packages"].split(",")
        self.assertIn("rung-std", pk)
        self.assertIn("rung-host", pk)
        self.assertNotIn("rung-testkit", pk)  # nothing it depends on changed
        r = self.plan("rung-agent-core/src/lib.rs")
        pk = r["packages"].split(",")
        for crate in ("rung-agent-core", "rung-agent", "rung-host", "rung-gateway"):
            self.assertIn(crate, pk)
        self.assertNotIn("rung-std", pk)
        self.assertEqual((r["rung"], r["agents"]), ("false", "true"))

    def test_the_gui_needs_name_real_crates_and_the_gateway_reaches_it(self):
        names = {p["name"] for p in self.meta["packages"]}
        self.assertLessEqual(c.GUI_NEEDS, names)
        for f in ("rung-gateway/src/lib.rs", "rung-host/src/lib.rs"):
            self.assertEqual(self.plan(f)["gui"], "true", f)
        self.assertEqual(self.plan("docs/reference/rung-host.md")["gui"], "false")
        self.assertEqual(self.plan("gui/rung-client/src/lib.rs")["gui"], "true")

    def test_every_extra_input_names_a_workspace_crate(self):
        names = {p["name"] for p in self.meta["packages"]}
        for _, crates in c.EXTRA_INPUTS:
            self.assertLessEqual(set(crates), names)

    def test_every_workspace_crate_has_a_directory_of_its_own(self):
        dirs, _ = c.workspace(self.meta)
        self.assertEqual(len(set(dirs.values())), len(dirs))
        self.assertNotIn("", dirs.values())


if __name__ == "__main__":
    unittest.main()
