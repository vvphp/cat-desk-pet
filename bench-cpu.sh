#!/usr/bin/env bash
# Sample %CPU for a process over N seconds (1 Hz). Prints avg/min/max.
set -euo pipefail

PID="${1:?pid}"
SECS="${2:-30}"
WARM="${3:-5}"

if ! kill -0 "$PID" 2>/dev/null; then
  echo "process $PID not running" >&2
  exit 1
fi

sleep "$WARM"

sum=0
count=0
min=999
max=0
declare -a samples=()

for ((i = 1; i <= SECS; i++)); do
  cpu=$(ps -p "$PID" -o %cpu= 2>/dev/null | tr -d ' ' || echo "")
  if [[ -z "$cpu" ]]; then
    echo "process exited during sample" >&2
    exit 1
  fi
  samples+=("$cpu")
  sum=$(awk -v s="$sum" -v c="$cpu" 'BEGIN { printf "%.4f", s + c }')
  count=$((count + 1))
  min=$(awk -v a="$min" -v b="$cpu" 'BEGIN { print (b < a) ? b : a }')
  max=$(awk -v a="$max" -v b="$cpu" 'BEGIN { print (b > a) ? b : a }')
  sleep 1
done

avg=$(awk -v s="$sum" -v n="$count" 'BEGIN { printf "%.2f", s / n }')
rss_kb=$(ps -p "$PID" -o rss= | tr -d ' ')
rss_mb=$(awk -v r="$rss_kb" 'BEGIN { printf "%.1f", r / 1024 }')

echo "avg_cpu=${avg} min_cpu=${min} max_cpu=${max} samples=${count} rss_mb=${rss_mb} pid=${PID}"
