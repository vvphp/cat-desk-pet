#!/usr/bin/env bash
# Run the issue #6 native/wgpu matrix on macOS with an AppKit-capable PTY.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: tools/benchmark-renderer-ab.sh NATIVE_BINARY WGPU_BINARY [SECONDS] [WARM]

Runs sleeping, idle, walking, and stress-props three times per backend.
Round order alternates native/wgpu to reduce thermal and cache bias.
Defaults: 60 one-second samples after a 10-second warm-up.
EOF
}

if [[ $# -lt 2 || $# -gt 4 ]]; then
  usage >&2
  exit 2
fi

NATIVE_BINARY="$1"
WGPU_BINARY="$2"
SAMPLE_SECONDS="${3:-60}"
WARM_SECONDS="${4:-10}"

[[ -x "$NATIVE_BINARY" ]] || { echo "not executable: $NATIVE_BINARY" >&2; exit 2; }
[[ -x "$WGPU_BINARY" ]] || { echo "not executable: $WGPU_BINARY" >&2; exit 2; }
[[ "$SAMPLE_SECONDS" =~ ^[1-9][0-9]*$ ]] || { echo "invalid sample seconds" >&2; exit 2; }
[[ "$WARM_SECONDS" =~ ^[0-9]+$ ]] || { echo "invalid warm seconds" >&2; exit 2; }
[[ "$(uname -s)" = "Darwin" ]] || { echo "this PTY runner currently supports macOS" >&2; exit 2; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAMP="$(date '+%Y%m%d-%H%M%S')"
OUT_DIR="$ROOT/benchmark-results/ab-$STAMP"
mkdir -p "$OUT_DIR"
ORDER_FILE="$OUT_DIR/round-order.txt"

APP_PID=""
SCRIPT_PID=""

stop_app() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    wait_count=0
    while kill -0 "$APP_PID" 2>/dev/null && [[ "$wait_count" -lt 20 ]]; do
      sleep 0.1
      wait_count=$((wait_count + 1))
    done
  fi
  if [[ -n "$SCRIPT_PID" ]] && kill -0 "$SCRIPT_PID" 2>/dev/null; then
    kill -TERM "$SCRIPT_PID" 2>/dev/null || true
  fi
  if [[ -n "$SCRIPT_PID" ]]; then
    wait "$SCRIPT_PID" 2>/dev/null || true
  fi
  APP_PID=""
  SCRIPT_PID=""
}
trap stop_app EXIT HUP INT TERM

run_one() {
  backend="$1"
  scenario="$2"
  round="$3"
  mode="$scenario"
  binary="$NATIVE_BINARY"
  renderer="native"
  expected="renderer=native"
  extra=""

  if [[ "$backend" = "wgpu-atlas" ]]; then
    binary="$WGPU_BINARY"
    renderer="wgpu"
    expected="wgpu path=atlas-direct"
  fi
  if [[ "$scenario" = "stress-props" ]]; then
    mode="walking"
    extra="--stress-props"
  fi

  app_log="$OUT_DIR/app-${scenario}-${backend}-r${round}.log"
  script_stdout="$OUT_DIR/script-${scenario}-${backend}-r${round}.log"
  printf '%s round=%s scenario=%s backend=%s\n' "$(date '+%F %T')" "$round" "$scenario" "$backend" | tee -a "$ORDER_FILE"

  # AppKit/winit requires a foreground-like terminal in this environment.
  # `script` supplies the PTY while the sampler runs in this parent process.
  script -Fq "$app_log" "$binary" --renderer "$renderer" --mode "$mode" $extra >"$script_stdout" 2>&1 &
  SCRIPT_PID=$!

  attempt=0
  while [[ "$attempt" -lt 50 ]]; do
    APP_PID="$(pgrep -P "$SCRIPT_PID" | awk 'NR == 1 { print; exit }')"
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
      break
    fi
    APP_PID=""
    sleep 0.1
    attempt=$((attempt + 1))
  done
  [[ -n "$APP_PID" ]] || { echo "app failed to start; see $app_log" >&2; exit 1; }

  attempt=0
  while [[ "$attempt" -lt 50 ]] && ! grep -q "$expected" "$app_log" 2>/dev/null; do
    sleep 0.1
    attempt=$((attempt + 1))
  done
  if ! grep -q "$expected" "$app_log" 2>/dev/null; then
    echo "renderer confirmation missing ($expected); see $app_log" >&2
    exit 1
  fi

  "$ROOT/tools/benchmark-renderer.sh" \
    --pid "$APP_PID" \
    --scenario "$scenario" \
    --backend "$backend-r$round" \
    --warm "$WARM_SECONDS" \
    --seconds "$SAMPLE_SECONDS" \
    --out-dir "$OUT_DIR" \
    --binary "$binary"
  stop_app
}

for scenario in sleeping idle walking stress-props; do
  run_one native "$scenario" 1
  run_one wgpu-atlas "$scenario" 1
  run_one wgpu-atlas "$scenario" 2
  run_one native "$scenario" 2
  run_one native "$scenario" 3
  run_one wgpu-atlas "$scenario" 3
done

echo "A/B matrix complete: $OUT_DIR"
