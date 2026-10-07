#!/usr/bin/env bash
# Shared deck build. Run through a deck's thin make.sh, which cd's into the deck dir.
# Builds that deck's build.py; publish only with a free versioned name (never overwrites):
#   ./make.sh --publish DIR [--name FILE]
set -euo pipefail
python3 build.py "$@"
