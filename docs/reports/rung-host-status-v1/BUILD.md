# Rebuild rung-host-status-v1

Record: `../rung-host-status-v1.pdf` (the committed PDF is the reviewed copy).

This report is a markdown memo, not a deck, so it does not use `../_build/`. The house `publishing` tool (pinned by `report.toml`) is the one build path:

    publishing build rung-host-status-v1     # rewrites ../rung-host-status-v1.pdf from memo.md
    publishing check                         # fails if a committed PDF is stale
