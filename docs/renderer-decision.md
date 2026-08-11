# ADR: keep the optimized native renderer as default

- Status: Accepted
- Date: 2026-08-11
- Issue: [#6](https://github.com/vvphp/cat-desk-pet/issues/6)

## Decision

Keep `NativeRenderer` as the release default. Do not enable the experimental `wgpu` backend in default features, packaging, or user configuration.

Retain `renderer-wgpu` only as an opt-in experiment while the stacked issue #6 work is reviewed. It may be removed later if there is no approved follow-up for direct bubble/particle rendering, Windows validation, and a materially different resource hypothesis.

## Why

On the tested Apple M5 system, wgpu did not reduce process CPU in any representative scenario. Median CPU increased from 0.40% to 1.10% sleeping, 1.10% to 2.15% idle, 2.90% to 3.20% walking, and 2.55% to 4.20% under bird-plus-laser stress. The idle backend was hybrid because real idle actions can create bubbles and particles that are not implemented in the GPU pipeline.

The Metal device/Surface fixed cost was also large for a tiny transparent window. Median-of-round footprint peaks were roughly 90-97 MiB for wgpu versus 14-20 MiB native. Walking RSS and binary-size deltas stayed inside their absolute gates, and stress footprint remained just below 100 MiB, but those passes do not offset the mandatory CPU failures or incomplete content path.

Whole-system energy, cold start, and Windows acceptance were not pursued after the mandatory CPU gates failed. Missing data cannot be interpreted as a wgpu benefit.

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
