#!/usr/bin/env bash
# Builds the editor and the demo deck, then takes the manual's screenshots
# into docs/src/images/screens/. Linux only; uses Xvfb when no display is set.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/docs/src/images/screens"
WORK=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT

if [ -z "${SKIP_BUILD:-}" ]; then
  (cd "$ROOT/apps/deck-app" && npm run build >/dev/null)
  cargo build --manifest-path "$ROOT/Cargo.toml" -p deck-app --features tauri/custom-protocol
fi
APP="$ROOT/target/debug/deck-app"
python3 -I "$ROOT/docs/screenshots/make_demo.py" "$WORK/demo.deck"

if [ -z "${DISPLAY:-}" ]; then
  # A free display number: a stale lock file means a display that is taken.
  n=90
  while [ -e "/tmp/.X$n-lock" ]; do n=$((n + 1)); done
  Xvfb ":$n" -screen 0 1600x1000x24 >/dev/null 2>&1 &
  XVFB=$!
  export DISPLAY=":$n"
  sleep 1
fi

python3 -I "$ROOT/docs/screenshots/shoot.py" "$APP" "$WORK/demo.deck" "$OUT"
