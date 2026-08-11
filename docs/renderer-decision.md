# ADR: keep the optimized native renderer as default

- Status: Accepted
- Date: 2026-08-11
- Issue: [#6](https://github.com/vvphp/cat-desk-pet/issues/6)

## Decision

Keep `NativeRenderer` as the release default. Do not enable the experimental `wgpu` backend in default features, packaging, or user configuration.

Retain `renderer-wgpu` only as an opt-in experiment while the stacked issue #6 work is reviewed. It may be removed later if there is no approved follow-up for direct bubble/particle rendering, Windows validation, and a materially different resource hypothesis.

## Why

On the tested Apple M5 system, the replayable `forced-v2` workload passed the sleeping CPU gate but failed the mandatory idle gate. Using independent nanosecond process CPU-time intervals, median-of-round CPU increased from 0.72% to 1.08% sleeping and from 0.94% to 2.00% idle; idle therefore regressed by 1.06 percentage points against a 0.5-point limit. The idle backend was hybrid because the fixed Yawn bubble is not implemented in the GPU pipeline.

The Metal device/Surface fixed cost was also large for a tiny transparent window. Median-of-round footprint peaks were 90.1 MiB sleeping and 91.0 MiB idle for wgpu versus 14.1 MiB and 15.6 MiB native. The binary-size delta stayed inside its absolute gate, but that does not offset the mandatory idle CPU failure or incomplete content path.

Walking and stress were not rerun after correcting the CPU sampler. Whole-system energy, cold start, and Windows acceptance were not pursued after the mandatory idle CPU gate failed. Missing data cannot be interpreted as a wgpu benefit.

Full data and method are in [`renderer-benchmark.md`](renderer-benchmark.md).

## Consequences

- The release path remains Rust state machine + offline layered atlas + native CALayer/softbuffer presentation.
- Runtime SVG parsing/rasterization stays eliminated; that Phase 2 benefit is independent of wgpu.
- The renderer boundary and dirty-frame scheduling stay useful and preserve a future backend seam.
- The optional wgpu build adds no dependency or binary cost to default builds.
- wgpu users must explicitly build `--features renderer-wgpu` and select `--renderer wgpu`.
- Unsupported content uses the measured native-upload fallback; it must not be represented as direct GPU rendering.

## Reconsideration criteria

Reopen the default decision only if a follow-up changes the tested hypothesis, for example by eliminating the PostMultiplied offscreen pass, directly rendering all frequent idle content, or targeting content complexity that native composition cannot sustain. A new proposal must rerun the same alternating protocol and still satisfy every issue #6 correctness, energy, CPU, memory, startup, binary, macOS, and Windows gate.

## Rollback

No rollback is needed because the default never changed. If the experimental feature causes maintenance or CI problems, delete the optional dependencies, `src/wgpu_renderer.rs`, and its shaders without changing behavior, assets, or user data.
