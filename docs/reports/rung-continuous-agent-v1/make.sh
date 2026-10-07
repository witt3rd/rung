#!/usr/bin/env bash
# Build the PDF; publish only with a free versioned name (never overwrites).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
python3 "$here/build.py" "$@"
