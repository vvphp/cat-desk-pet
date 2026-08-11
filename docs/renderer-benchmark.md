# Renderer benchmark protocol

This document defines the Phase 0 measurement contract for [issue #6](https://github.com/vvphp/cat-desk-pet/issues/6). It is a protocol and report template, not evidence that a `wgpu` backend is faster. The default renderer remains the current native CPU path until both backends have been measured on the same machine and workload.

## Decision being measured

Compare these implementations without changing pet behavior, visual output, input handling, or window semantics:

- `native`: the current CPU raster + platform presentation path.
- `wgpu-atlas`: the feature-gated direct layered-atlas path behind the same render snapshot contract. Runs that log `native-upload-fallback` must be labelled separately and are not valid `wgpu-atlas` evidence.

Adopt `wgpu-atlas` as the default only when repeated release-build measurements show a material user benefit without regressing transparency, click-through, startup, memory, package size, or supported platforms. Otherwise keep `native`; a hybrid backend is acceptable only when its activation rule is explicit and measurable.

## Required environment record

Every comparison report must include:

- Git commit and dirty/clean state.
- OS version, CPU model, architecture, RAM, display resolution, and DPR/scale factor.
- Power source and macOS Low Power Mode state.
- Release command and complete runtime arguments.
- Renderer backend, scenario, warm-up duration, sample duration, and round order.
- Binary size and, for packaged tests, application bundle size.

The benchmark helper records the stable process and repository fields automatically, including CPU model and installed RAM where the OS exposes them. Display/DPR, power state, launch latency, and whole-system energy must be added to the report because they are not reliably attributable from an unprivileged process sampler.

## Scenarios

Use the same fixed placement and display for all runs.

| Scenario | Launch arguments | What it covers |
|---|---|---|
| sleeping | `--mode sleeping` | Fixed sleeping pose with live cursor input ignored |
| idle | `--mode idle` | Fixed `Sit -> Yawn -> Stretch -> Look -> TailCurl` replay, including the same timed bubble |
| walking | `--mode walking` | Fixed edge-to-edge walking path with no random targets |
| stress-props | `--mode walking --stress-props` | Fixed walking path, repeated left-to-right bird, and clock-driven laser curve |

These arguments select workload `forced-v2`. Every forced scenario ignores live cursor updates and removes random target/action selection, so native and wgpu receive the same replayable application state for a given timestep sequence. The A/B wrapper records `workload=forced-v2` in each `summary.txt`; results without that field, including the earlier 2026-08-11 run below, must not be mixed with this workload.

For backend comparisons, run at least three rounds per scenario. Alternate order (`native → wgpu`, then `wgpu → native`) to reduce thermal and cache bias. Use a warm-up of at least 10 seconds and sample for at least 60 seconds at 1 Hz.

## CPU and memory capture

Build and launch the release binaries in one terminal. Keep separate copies so rebuilding one feature set cannot silently replace the other:

```bash
cargo build --release
cp target/release/cat-desk-pet /tmp/cat-desk-pet-native

cargo build --release --features renderer-wgpu
cp target/release/cat-desk-pet /tmp/cat-desk-pet-wgpu

/tmp/cat-desk-pet-native --renderer native --mode walking
# or:
/tmp/cat-desk-pet-wgpu --renderer wgpu --mode walking
```

Capture its PID and sample it from another terminal:

```bash
tools/benchmark-renderer.sh \
  --pid "$(pgrep -n cat-desk-pet)" \
  --scenario walking \
  --backend native \
  --warm 10 \
  --seconds 60 \
  --workload forced-v2 \
  --binary target/release/cat-desk-pet
```

For a direct wgpu sample, first confirm stderr contains both `renderer=wgpu` and `wgpu path=atlas-direct`, then pass `--backend wgpu-atlas` and the frozen wgpu binary path. Record a run that enters fallback as `wgpu-hybrid` instead. The realistic fixed idle workload currently uses the hybrid label because some idle actions create bubbles or particles.

On macOS, the complete alternating 4-scenario x 2-backend x 3-round matrix can be run with:

```bash
tools/benchmark-renderer-ab.sh \
  /tmp/cat-desk-pet-native \
  /tmp/cat-desk-pet-wgpu
```

Run the wrapper from a real terminal. It verifies the reported renderer path before sampling and stops only the exact child process it created.

Results are written under the ignored `benchmark-results/` directory:

- `samples.csv`: raw 1 Hz process CPU-time deltas and RSS samples. CPU percent is `100 * process CPU-time delta / monotonic wall-time delta`; it does not reuse `ps %cpu` history. On macOS, the helper reads nanosecond process time from `proc_pid_rusage`; the coarser `ps TIME` parser is only a non-macOS fallback.
- `summary.txt`: commit/environment metadata plus average, minimum, maximum, median, and nearest-rank p95 values.
- `vmmap-summary.txt`: macOS physical-footprint evidence when `vmmap` is available.

Keep raw result directories for review artifacts, but do not commit machine-specific runs unless the issue or PR explicitly calls for a checked-in evidence set.

## Energy, GPU, and startup capture

`ps` cannot determine whole-system GPU or energy cost. For the final native-vs-wgpu decision:

1. Use the same macOS Activity Monitor Energy tab or the same Instruments Energy Log/GPU template for both backends.
2. Capture an idle system baseline, then the same scenario duration and round ordering used for CPU samples.
3. Record the tool version, capture duration, whole-system versus process scope, and median/peak values exposed by that tool.
4. Measure cold launch from process start to first visible frame for at least ten launches per backend; report median and p95.

Do not combine `ps` process CPU with an unrelated whole-system energy interval and present it as one run.

## Historical reference only

These existing measurements predate the issue #6 protocol and are not a backend comparison. They are retained only as regression context:

| Source | Scenario | CPU | RSS | Physical footprint peak |
|---|---|---:|---:|---:|
| PR #5 Retina restore, 2026-07-19 | walking, warm short sample | ~8.6% avg | ~88 MB | 20.9 MB |
| PR #5 Retina restore, 2026-07-19 | walking + stress props | not recorded | ~98 MB | 42.5 MB |
| issue #3 1x/lower-FPS baseline, 2026-07-19 | sleeping | 0.91% avg | 74.5 MB | 14.9 MB |
| issue #3 1x/lower-FPS baseline, 2026-07-19 | idle | 1.43% avg | 73.0 MB | 15.1 MB |
| issue #3 1x/lower-FPS baseline, 2026-07-19 | walking | 2.81% avg | 73.3 MB | 15.7 MB |

See [`docs/perf.md`](perf.md) for the original context and caveats.

## Comparison report template

```markdown
### Environment

- Commit / clean state:
- OS / CPU / architecture / RAM:
- Display / DPR:
- Power state:
- Build and runtime arguments:
- Round order:

### Results (median of rounds; include per-round raw directories)

| Scenario | Backend | CPU median | CPU p95 | RSS median | RSS peak | Footprint peak | Energy/GPU | Cold start p50/p95 | Binary/app size |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| sleeping | native | | | | | | | | |
| sleeping | wgpu-atlas | | | | | | | | |
| idle | native | | | | | | | | |
| idle | wgpu-atlas | | | | | | | | |
| walking | native | | | | | | | | |
| walking | wgpu-atlas | | | | | | | | |
| stress-props | native | | | | | | | | |
| stress-props | wgpu-atlas | | | | | | | | |

### Correctness gates

- [ ] Pixel-diff tolerance and hit-mask alignment pass at 1x and Retina DPR.
- [ ] Transparent, topmost, click-through, drag, tray, and multi-display behavior pass.
- [ ] Sleeping/static mode does not continuously present unchanged frames.
- [ ] No new steady-state allocation growth or unbounded atlas/cache growth.
- [ ] macOS and Windows release builds pass.

### Decision

- Default backend:
- Quantified benefit:
- Regressions and accepted trade-offs:
- Hybrid activation rule, if any:
- Rollback trigger:
```

## Go / no-go rule

The PR proposing a new default backend must link its raw evidence and fill the template above. Averages alone are insufficient: compare median and p95 CPU, RSS median/peak, footprint peak, energy/GPU evidence, cold-start p50/p95, and package size. Any correctness-gate failure is a no-go regardless of a faster microbenchmark.

## 2026-08-11 native versus wgpu result (forced-v2 rerun)

Decision: **No-Go for making wgpu the default.** See [`renderer-decision.md`](renderer-decision.md) for the ADR and [`renderer-benchmark-2026-08-11.csv`](renderer-benchmark-2026-08-11.csv) for every valid round summary.

### Environment and method

- Frozen binaries: renderer source at commit `1c2b678`, clean when built, with workload `forced-v2`; the later commit only updates this report.
- Native SHA-256: `44c872cc45079edfa293550d787ed3f80079bf2675e27ef5278892029bb89929`.
- wgpu SHA-256: `9b6e5ae035b231dc1054aa17072b556dd0526b5d47a70cae4f88420aec349a4c`.
- macOS 26.5.1, Apple M5, arm64, 16 GiB RAM.
- DELL S2722DC, 2560 x 1440 at 75 Hz, 1x application scale.
- AC power, Low Power Mode off.
- Release profile with LTO; 10-second warm-up and 60 one-second samples. Sleeping and idle used three alternating rounds per backend.
- CPU is calculated from nanosecond `proc_pid_rusage` deltas divided by monotonic wall-time deltas. It does not use the historical/decaying `ps %cpu` value. A 100% CPU probe measured 99.70%-99.80% before this run.
- Metal adapter requested with `LowPower`; Surface selected `Bgra8UnormSrgb` and `PostMultiplied` alpha.
- sleeping stayed on `atlas-direct`. Idle entered the documented native-upload fallback for the fixed Yawn bubble and is labelled `wgpu-hybrid`.
- Walking and stress-props were not rerun after the CPU sampler correction and are not used as acceptance evidence.

The table reports the median of the three round-level values, not a pooled average:

| Scenario | Backend | CPU median | CPU p95 | RSS median | RSS peak | Footprint peak | Binary size |
|---|---|---:|---:|---:|---:|---:|---:|
| sleeping | native | 0.72% | 1.10% | 71.2 MiB | 71.3 MiB | 14.1 MiB | 2.40 MiB |
| sleeping | wgpu-atlas | 1.08% | 1.47% | 74.0 MiB | 79.9 MiB | 90.1 MiB | 5.41 MiB |
| idle | native | 0.94% | 1.31% | 72.8 MiB | 72.9 MiB | 15.6 MiB | 2.40 MiB |
| idle | wgpu-hybrid | 2.00% | 2.88% | 81.6 MiB | 81.8 MiB | 91.0 MiB | 5.41 MiB |

### Gate interpretation

- Pass: sleeping CPU regressed by 0.36 percentage points, within the 0.5-point limit.
- Fail: fixed idle CPU regressed by 1.06 percentage points and required the hybrid fallback; the limit is 0.5.
- Trade-off: sleeping and idle wgpu footprint peaks were about 90 MiB, roughly six times native. They remain below the 100 MiB absolute limit but are not justified by the measured CPU result.
- Pass: binary delta was 3,163,776 bytes (3.02 MiB), within the 10 MiB limit.
- Not completed: corrected-sampler walking/stress-props, whole-system energy/GPU, cold-start distribution, and real Windows D3D12 acceptance. These cannot turn the current result into a Go because the mandatory idle CPU gate already failed.

The ignored local result root `benchmark-results/ab-20260811-174401` retains the 1 Hz samples and `vmmap` output. It is not committed; the checked-in CSV contains only complete, matched evidence. Rows from the old `ps %cpu` sampler and uncorrected workloads were removed rather than mixed with this evidence.
