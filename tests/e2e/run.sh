#!/usr/bin/env bash
# Builds the editor with its front end embedded and runs the end-to-end
# tests; starts Xvfb when there is no display. Linux only.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
if [ -z "${SKIP_BUILD:-}" ]; then
  (cd "$ROOT/apps/deck-app" && npm run build >/dev/null)
  cargo build --manifest-path "$ROOT/Cargo.toml" -p deck-app --features tauri/custom-protocol
fi
if [ -z "${DISPLAY:-}" ]; then
  # A free display number: a stale lock file means a display that is taken.
  n=90
  while [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
  Xvfb ":$n" -screen 0 1600x1000x24 >/dev/null 2>&1 &
  XVFB=$!
  trap 'kill $XVFB 2>/dev/null || true' EXIT
  export DISPLAY=":$n"
  sleep 1
fi
export DECKER_APP="$ROOT/target/debug/deck-app"
python3 -I -m unittest discover -s "$ROOT/tests/e2e" -v "$@"
