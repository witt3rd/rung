# Remote fm/* and spire/* branches (2026-11-01)

Read at master `96a8fa5` + PR #357. 11 remain (was 14). **Nothing was deleted;
the owner confirms by name.** `git cherry` shows every branch as unlanded because
the landings were squash merges (different patch ids), so "landed" was decided
by content: the branch's files and the merged PR on master.

| branch | last commit | landed on master | open PR | verdict |
|---|---|---|---|---|
| fm/rung-docs-s1 | 2026-11-01 | no, 3 commits | [#360](https://github.com/witt3rd/rung/pull/360) | KEEP (open PR) |
| fm/rung-fleet-r2 | 2026-11-01 | no, 2 commits | [#355](https://github.com/witt3rd/rung/pull/355) | KEEP (open PR) |
| fm/rung-fleet-r2-receipts | 2026-11-01 | no, 3 commits | [#356](https://github.com/witt3rd/rung/pull/356) | KEEP (open PR) |
| fm/rung-idle-mode-display | 2026-11-01 | no, 1 commit | [#361](https://github.com/witt3rd/rung/pull/361) | KEEP (open PR) |
| spire/b1a816c679f0 | 2026-11-01 | no, 2 commits | [#339](https://github.com/witt3rd/rung/pull/339) | KEEP (open PR) |
| spire/bf17754cce70 | 2026-10-26 | no, 9 commits | [#338](https://github.com/witt3rd/rung/pull/338) | KEEP (open PR) |
| fm/rung-nocaps-p7 | 2026-10-12 | yes, content: P7 no caps landed as #326 (`605b678`); `no_caps_prompt.rs` on master; PR #324 closed | none | DELETE |
| fm/rung-nocaps-p7b | 2026-10-12 | yes, content: same P7 plus the window-sizing fix, landed as #326 (`desk_window.rs`, `no_caps_prompt.rs` on master); no PR | none | DELETE |
| fm/rung-s2-archive-364a833 | 2026-10-11 | yes, content: S2 intent on the queue item landed as #304 (`2f5f6dd`); `intent_desk.rs` byte-identical on master | none | DELETE |
| fm/rung-s2-archive-55d4cab | 2026-10-11 | yes, content: same S2 landing (#304), `intent_desk.rs` identical | none | DELETE |
| spire/698612cbd7e0 | 2026-10-11 | **no**: `examples/continuum/continuum-assistant.yaml`, `examples/continuum/README.md` and `rung-host/tests/continuum_assistant.rs` are absent from master; PR [#291](https://github.com/witt3rd/rung/pull/291) closed unmerged | none | KEEP (unlanded work: the Continuum research-assistant host config and its mock-engine test; owner decides land or drop) |

Proposed DELETE list (four names): `fm/rung-nocaps-p7`, `fm/rung-nocaps-p7b`,
`fm/rung-s2-archive-364a833`, `fm/rung-s2-archive-55d4cab`. Note the two
`s2-archive` names say archive; their content is on master, so the branches add
nothing. `fm/rung-trunk-mandate` was removed by the merge of #357.
The open-PR branches shrink as the other mate merges them.
