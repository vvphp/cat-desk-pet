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
| sleeping | `--mode sleeping` | Static/low-frequency idle cost |
| idle | `--mode idle` | Normal animated idle |
| walking | `--mode walking` | Continuous sprite animation and movement |
| stress-props | `--mode walking --stress-props` | Walking, birds, laser, particles, and large dirty regions |

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

- `samples.csv`: raw 1 Hz `%CPU` and RSS samples.
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

## 2026-08-11 native versus wgpu result

Decision: **No-Go for making wgpu the default.** See [`renderer-decision.md`](renderer-decision.md) for the ADR and [`renderer-benchmark-2026-08-11.csv`](renderer-benchmark-2026-08-11.csv) for every valid round summary.

### Environment and method

- Frozen binaries: renderer source at `006631b`; later repository commits changed only benchmark labels/documentation.
- Native SHA-256: `19c06a0213e7b007ca58075ded61a2bfcabfc3351d361b774375387b1461c593`.
- wgpu SHA-256: `baa8574c71cf64b88f6a788f0fdeed0c6346e394647f5398daac92cb51a333d8`.
- macOS 26.5.1, Apple M5, arm64, 16 GiB RAM.
- DELL S2722DC, 2560 x 1440 at 75 Hz, 1x application scale.
- AC power, Low Power Mode off.
- Release profile with LTO; 10-second warm-up, 60 one-second samples, three alternating rounds per backend and scenario.
- Metal adapter requested with `LowPower`; Surface selected `Bgra8UnormSrgb` and `PostMultiplied` alpha.
- sleeping, walking, and stress-props stayed on `atlas-direct` for the complete sample. Realistic idle entered the documented native-upload fallback for action bubbles/particles and is labelled `wgpu-hybrid`.

The table reports the median of the three round-level values, not a pooled average:

| Scenario | Backend | CPU median | CPU p95 | RSS median | RSS peak | Footprint peak | Binary size |
|---|---|---:|---:|---:|---:|---:|---:|
| sleeping | native | 0.40% | 1.10% | 65.1 MiB | 71.1 MiB | 14.0 MiB | 2.40 MiB |
| sleeping | wgpu-atlas | 1.10% | 1.50% | 79.6 MiB | 79.9 MiB | 90.0 MiB | 5.41 MiB |
| idle | native | 1.10% | 2.40% | 72.6 MiB | 72.8 MiB | 15.4 MiB | 2.40 MiB |
| idle | wgpu-hybrid | 2.15% | 3.40% | 81.3 MiB | 81.6 MiB | 90.6 MiB | 5.41 MiB |
| walking | native | 2.90% | 3.90% | 68.2 MiB | 71.8 MiB | 15.2 MiB | 2.40 MiB |
| walking | wgpu-atlas | 3.20% | 5.20% | 76.2 MiB | 80.1 MiB | 89.8 MiB | 5.41 MiB |
| stress-props | native | 2.55% | 4.70% | 75.1 MiB | 76.4 MiB | 19.6 MiB | 2.40 MiB |
| stress-props | wgpu-atlas | 4.20% | 6.30% | 80.6 MiB | 81.3 MiB | 96.8 MiB | 5.41 MiB |

### Gate interpretation

- Fail: sleeping CPU regressed by 0.70 percentage points; the limit is 0.5.
- Fail: idle CPU regressed by 1.05 percentage points and required the hybrid fallback; the limit is 0.5.
- Fail: walking and stress CPU medians regressed by about 10% and 65%, respectively. There is no basis to expect the required 15% whole-system energy improvement.
- Pass: walking RSS median increased by about 8.0 MiB, within the 25 MiB limit.
- Pass, narrowly: stress footprint median-of-round-peaks was 96.8 MiB and the worst round was 97.4 MiB, below 100 MiB. It was still about five times native.
- Pass: binary delta was 3,163,776 bytes (3.02 MiB), within the 10 MiB limit.
- Not completed: whole-system energy/GPU, cold-start distribution, and real Windows D3D12 acceptance. These cannot turn the current result into a Go because multiple mandatory gates already failed.

The ignored local result roots retain the 1 Hz samples and `vmmap` output: `benchmark-results/ab-20260811-152012` for sleeping and `benchmark-results/ab-20260811-153104` for idle/walking/stress-props. They are not committed; the checked-in CSV is the reviewable per-round evidence.
