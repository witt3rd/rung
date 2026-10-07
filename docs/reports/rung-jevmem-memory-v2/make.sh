#!/usr/bin/env bash
# Build the rung memory PDF; publish with: ./make.sh --publish "<dir>" --name <file>
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
python3 "$here/build.py" "$@"
