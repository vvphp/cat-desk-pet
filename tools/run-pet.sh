#!/usr/bin/env bash
# Detach cat-desk-pet so closing the terminal won't kill it.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/cat-desk-pet"
LOG="${CAT_DESK_PET_LOG:-/tmp/cat-desk-pet.log}"
PIDFILE="${CAT_DESK_PET_PID:-/tmp/cat-desk-pet.pid}"

if [[ ! -x "$BIN" ]]; then
  echo "building $BIN ..."
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

pkill -f "$BIN" 2>/dev/null || true
sleep 0.2

python3 - "$BIN" "$LOG" "$PIDFILE" <<'PY'
import os, sys, time
bin_path, log, pidfile = sys.argv[1], sys.argv[2], sys.argv[3]
if os.fork() > 0:
    time.sleep(0.25)
    sys.exit(0)
os.setsid()
if os.fork() > 0:
    sys.exit(0)
os.chdir("/")
out = open(log, "a")
os.dup2(out.fileno(), 1)
os.dup2(out.fileno(), 2)
open(pidfile, "w").write(str(os.getpid()))
os.execv(bin_path, [bin_path])
PY

sleep 0.4
echo "started pid=$(cat "$PIDFILE")  log=$LOG"
echo "quit via tray menu「退出」"
