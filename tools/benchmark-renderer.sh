#!/usr/bin/env bash
# Collect repeatable renderer CPU/RSS samples and macOS footprint evidence.
# Compatible with the Bash 3.2 shipped by macOS.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: tools/benchmark-renderer.sh --pid PID --scenario NAME [options]

Options:
  --backend NAME      Renderer backend label (default: native)
  --seconds N         Number of 1 Hz samples (default: 60)
  --warm N            Warm-up seconds before sampling (default: 10)
  --out-dir DIR       Parent directory for results (default: benchmark-results)
  --binary PATH       Release binary used for the binary-size record
  --workload NAME     Replayable workload version recorded in summary.txt
  -h, --help          Show this help

The script writes interval CPU-time deltas to samples.csv, plus summary.txt and
(on macOS) vmmap-summary.txt.
Run each scenario at least three times and alternate backend order when comparing.
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

is_positive_integer() {
  case "$1" in
    ''|*[!0-9]*) return 1 ;;
    *) [[ "$1" -gt 0 ]] ;;
  esac
}

is_non_negative_integer() {
  case "$1" in
    ''|*[!0-9]*) return 1 ;;
    *) return 0 ;;
  esac
}

safe_label() {
  case "$1" in
    ''|*[!A-Za-z0-9._-]*) return 1 ;;
    *) return 0 ;;
  esac
}

process_sample() {
  LC_ALL=C ps -p "$PID" -o time= -o rss= 2>/dev/null | awk '
    NF >= 2 {
      raw = $1
      rss = $2
      days = 0
      day_parts = split(raw, with_days, "-")
      if (day_parts == 2) {
        days = with_days[1] + 0
        raw = with_days[2]
      }
      count = split(raw, parts, ":")
      seconds = parts[count] + 0
      minutes = count >= 2 ? parts[count - 1] + 0 : 0
      hours = count >= 3 ? parts[count - 2] + 0 : 0
      total = days * 86400 + hours * 3600 + minutes * 60 + seconds
      printf "%.2f,%s\n", total, rss
      exit
    }
  '
}

monotonic_seconds() {
  "$HIRES_PERL" -MTime::HiRes=clock_gettime,CLOCK_MONOTONIC -e \
    'printf "%.9f\n", clock_gettime(CLOCK_MONOTONIC)'
}

metric() {
  input="$1"
  field="$2"
  mode="$3"

  case "$mode" in
    avg)
      awk -F, -v f="$field" 'NR > 1 { sum += $f; n++ } END { if (n) printf "%.2f", sum / n; else print "na" }' "$input"
      ;;
    min)
      awk -F, -v f="$field" 'NR > 1 { if (!n || $f < value) value = $f; n++ } END { if (n) printf "%.2f", value; else print "na" }' "$input"
      ;;
    max)
      awk -F, -v f="$field" 'NR > 1 { if (!n || $f > value) value = $f; n++ } END { if (n) printf "%.2f", value; else print "na" }' "$input"
      ;;
    median|p95)
      values_file="$4"
      awk -F, -v f="$field" 'NR > 1 { print $f }' "$input" | LC_ALL=C sort -n > "$values_file"
      count="$(wc -l < "$values_file" | tr -d ' ')"
      if [[ "$count" -eq 0 ]]; then
        echo "na"
      elif [[ "$mode" = "median" ]]; then
        awk -v n="$count" 'NR == int((n + 1) / 2) { a = $1 } NR == int((n + 2) / 2) { b = $1 } END { printf "%.2f", (a + b) / 2 }' "$values_file"
      else
        rank=$(( (95 * count + 99) / 100 ))
        awk -v rank="$rank" 'NR == rank { printf "%.2f", $1; exit }' "$values_file"
      fi
      ;;
    *)
      die "unknown metric: $mode"
      ;;
  esac
}

PID=""
SCENARIO=""
BACKEND="native"
SECONDS_TO_SAMPLE=60
WARM_SECONDS=10
OUT_DIR="benchmark-results"
BINARY=""
WORKLOAD="unspecified"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pid)
      [[ $# -ge 2 ]] || die "--pid requires a value"
      PID="$2"
      shift 2
      ;;
    --scenario)
      [[ $# -ge 2 ]] || die "--scenario requires a value"
      SCENARIO="$2"
      shift 2
      ;;
    --backend)
      [[ $# -ge 2 ]] || die "--backend requires a value"
      BACKEND="$2"
      shift 2
      ;;
    --seconds)
      [[ $# -ge 2 ]] || die "--seconds requires a value"
      SECONDS_TO_SAMPLE="$2"
      shift 2
      ;;
    --warm)
      [[ $# -ge 2 ]] || die "--warm requires a value"
      WARM_SECONDS="$2"
      shift 2
      ;;
    --out-dir)
      [[ $# -ge 2 ]] || die "--out-dir requires a value"
      OUT_DIR="$2"
      shift 2
      ;;
    --binary)
      [[ $# -ge 2 ]] || die "--binary requires a value"
      BINARY="$2"
      shift 2
      ;;
    --workload)
      [[ $# -ge 2 ]] || die "--workload requires a value"
      WORKLOAD="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

is_positive_integer "$PID" || die "--pid must be a positive integer"
safe_label "$SCENARIO" || die "--scenario may contain only letters, numbers, dot, underscore, and dash"
safe_label "$BACKEND" || die "--backend may contain only letters, numbers, dot, underscore, and dash"
safe_label "$WORKLOAD" || die "--workload may contain only letters, numbers, dot, underscore, and dash"
is_positive_integer "$SECONDS_TO_SAMPLE" || die "--seconds must be a positive integer"
is_non_negative_integer "$WARM_SECONDS" || die "--warm must be a non-negative integer"
kill -0 "$PID" 2>/dev/null || die "process $PID is not running"
HIRES_PERL="$(command -v perl || true)"
[[ -n "$HIRES_PERL" ]] || die "perl Time::HiRes is required for interval timing"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
case "$OUT_DIR" in
  /*) ;;
  *) OUT_DIR="$ROOT/$OUT_DIR" ;;
esac

TIMESTAMP="$(date '+%Y%m%d-%H%M%S')"
RUN_DIR="$OUT_DIR/${TIMESTAMP}-${SCENARIO}-${BACKEND}"
[[ ! -e "$RUN_DIR" ]] || die "result directory already exists: $RUN_DIR"
mkdir -p "$RUN_DIR"

SAMPLES="$RUN_DIR/samples.csv"
CPU_SORTED="$RUN_DIR/.cpu-sorted"
RSS_SORTED="$RUN_DIR/.rss-sorted"
cleanup_samples() {
  rm -f "$CPU_SORTED" "$RSS_SORTED"
}
trap cleanup_samples EXIT
trap 'cleanup_samples; exit 129' HUP
trap 'cleanup_samples; exit 130' INT
trap 'cleanup_samples; exit 143' TERM

echo "sample,elapsed_s,cpu_percent,rss_kb" > "$SAMPLES"
echo "warming process $PID for ${WARM_SECONDS}s ..."
if [[ "$WARM_SECONDS" -gt 0 ]]; then
  sleep "$WARM_SECONDS"
fi

echo "sampling ${SCENARIO}/${BACKEND} for ${SECONDS_TO_SAMPLE}s ..."
previous_row="$(process_sample)"
[[ -n "$previous_row" ]] || die "process $PID exited before sampling"
previous_cpu_seconds="${previous_row%%,*}"
previous_wall_seconds="$(monotonic_seconds)"
sample=1
while [[ "$sample" -le "$SECONDS_TO_SAMPLE" ]]; do
  sleep 1
  row="$(process_sample)"
  [[ -n "$row" ]] || die "process $PID exited during sampling"
  wall_seconds="$(monotonic_seconds)"
  cpu_seconds="${row%%,*}"
  rss_kb="${row#*,}"
  cpu_percent="$(awk \
    -v current_cpu="$cpu_seconds" \
    -v previous_cpu="$previous_cpu_seconds" \
    -v current_wall="$wall_seconds" \
    -v previous_wall="$previous_wall_seconds" '
    BEGIN {
      cpu_delta = current_cpu - previous_cpu
      wall_delta = current_wall - previous_wall
      if (cpu_delta < 0 || wall_delta <= 0) exit 1
      printf "%.2f", cpu_delta / wall_delta * 100
    }
  ')" || die "invalid process CPU or monotonic time delta"
  echo "$sample,$sample,$cpu_percent,$rss_kb" >> "$SAMPLES"
  previous_cpu_seconds="$cpu_seconds"
  previous_wall_seconds="$wall_seconds"
  sample=$((sample + 1))
done

CPU_AVG="$(metric "$SAMPLES" 3 avg)"
CPU_MIN="$(metric "$SAMPLES" 3 min)"
CPU_MAX="$(metric "$SAMPLES" 3 max)"
CPU_MEDIAN="$(metric "$SAMPLES" 3 median "$CPU_SORTED")"
CPU_P95="$(metric "$SAMPLES" 3 p95 "$CPU_SORTED")"
RSS_AVG_KB="$(metric "$SAMPLES" 4 avg)"
RSS_MIN_KB="$(metric "$SAMPLES" 4 min)"
RSS_MAX_KB="$(metric "$SAMPLES" 4 max)"
RSS_MEDIAN_KB="$(metric "$SAMPLES" 4 median "$RSS_SORTED")"
RSS_P95_KB="$(metric "$SAMPLES" 4 p95 "$RSS_SORTED")"

VM_MAP_FILE="$RUN_DIR/vmmap-summary.txt"
FOOTPRINT="na"
FOOTPRINT_PEAK="na"
if command -v vmmap >/dev/null 2>&1; then
  if vmmap -summary "$PID" > "$VM_MAP_FILE" 2>&1; then
    FOOTPRINT="$(awk -F: '/^Physical footprint:/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' "$VM_MAP_FILE")"
    FOOTPRINT_PEAK="$(awk -F: '/^Physical footprint \(peak\):/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' "$VM_MAP_FILE")"
    [[ -n "$FOOTPRINT" ]] || FOOTPRINT="na"
    [[ -n "$FOOTPRINT_PEAK" ]] || FOOTPRINT_PEAK="na"
  else
    FOOTPRINT="unavailable"
    FOOTPRINT_PEAK="unavailable"
  fi
fi

if [[ -z "$BINARY" ]]; then
  BINARY="$(LC_ALL=C ps -p "$PID" -o comm= 2>/dev/null | awk '{$1=$1; print}' || true)"
fi
BINARY_SIZE_BYTES="na"
if [[ -n "$BINARY" && -f "$BINARY" ]]; then
  BINARY_SIZE_BYTES="$(wc -c < "$BINARY" | tr -d ' ')"
fi

GIT_COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
GIT_DIRTY="$(git -C "$ROOT" status --porcelain 2>/dev/null | awk 'NF { found=1 } END { print found ? "true" : "false" }')"
PROCESS_COMMAND="$(LC_ALL=C ps -p "$PID" -o command= 2>/dev/null | awk '{$1=$1; print}' || true)"
OS_VERSION="$(uname -sr)"
if command -v sw_vers >/dev/null 2>&1; then
  OS_VERSION="$(sw_vers -productName) $(sw_vers -productVersion)"
fi
CPU_MODEL="unknown"
RAM_BYTES="unknown"
if command -v sysctl >/dev/null 2>&1; then
  CPU_MODEL="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || sysctl -n hw.model 2>/dev/null || echo unknown)"
  RAM_BYTES="$(sysctl -n hw.memsize 2>/dev/null || echo unknown)"
elif [[ -r /proc/cpuinfo ]]; then
  CPU_MODEL="$(awk -F: '/model name/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' /proc/cpuinfo)"
  [[ -n "$CPU_MODEL" ]] || CPU_MODEL="unknown"
  if [[ -r /proc/meminfo ]]; then
    RAM_BYTES="$(awk '/^MemTotal:/ { print $2 * 1024; exit }' /proc/meminfo)"
  fi
fi

cat > "$RUN_DIR/summary.txt" <<EOF
schema_version=1
timestamp=$TIMESTAMP
git_commit=$GIT_COMMIT
git_dirty=$GIT_DIRTY
os=$OS_VERSION
arch=$(uname -m)
cpu_model=$CPU_MODEL
ram_bytes=$RAM_BYTES
scenario=$SCENARIO
backend=$BACKEND
workload=$WORKLOAD
pid=$PID
process_command=$PROCESS_COMMAND
warm_seconds=$WARM_SECONDS
sample_seconds=$SECONDS_TO_SAMPLE
sample_count=$SECONDS_TO_SAMPLE
cpu_sample_method=process_cputime_delta_over_monotonic_interval
cpu_avg_percent=$CPU_AVG
cpu_min_percent=$CPU_MIN
cpu_max_percent=$CPU_MAX
cpu_median_percent=$CPU_MEDIAN
cpu_p95_percent=$CPU_P95
rss_avg_kb=$RSS_AVG_KB
rss_min_kb=$RSS_MIN_KB
rss_max_kb=$RSS_MAX_KB
rss_median_kb=$RSS_MEDIAN_KB
rss_p95_kb=$RSS_P95_KB
physical_footprint=$FOOTPRINT
physical_footprint_peak=$FOOTPRINT_PEAK
binary=$BINARY
binary_size_bytes=$BINARY_SIZE_BYTES
EOF

echo "wrote $RUN_DIR"
echo "cpu median=${CPU_MEDIAN}% p95=${CPU_P95}%  rss median=${RSS_MEDIAN_KB}KB peak=${RSS_MAX_KB}KB"
echo "physical footprint=${FOOTPRINT} peak=${FOOTPRINT_PEAK}"
