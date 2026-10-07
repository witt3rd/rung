#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
exec ../_build/make.sh "$@"
