# Cut a release

Informative. rung is not published to a registry; a release is a version, a
changelog entry and a tag that downstream consumers pin. Master is protected
(pull request, strict `check`), so the release commit lands like any change
(see the house git section of the root `AGENTS.md`).

1. **Release commit.** Bump `[workspace.package] version` in `Cargo.toml`, build
   so `Cargo.lock` follows, and move the `[Unreleased]` entries of
   `CHANGELOG.md` under the new version. Commit `release: X.Y.Z (workspace
   version matches tag)`.
2. **Land it** as a short-lived branch and pull request; wait for `check`.
3. **Tag** `vX.Y.Z` on the merged commit and push the tag. The tag must equal
   the workspace version.

CI's `release-check` job (pull requests and `v*` tag pushes) verifies and never
publishes: the tag equals the workspace version (tag pushes), `Cargo.lock` is
current (`--locked`), a crate depending on a `publish = false` crate is also
`publish = false`, and `cargo build -p rung-agent --release --locked` succeeds.
Run the middle checks by hand with `python3 docs/_release_check.py [vX.Y.Z]`.
