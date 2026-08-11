# Experimental wgpu renderer

The `wgpu` backend is an A/B experiment for [issue #6](https://github.com/vvphp/cat-desk-pet/issues/6). It is feature-gated and does not change the default native renderer.

## Build and select a backend

```bash
# Optimized native renderer (default build)
cargo run --release -- --renderer native --mode walking

# Experimental wgpu renderer
cargo run --release --features renderer-wgpu -- \
  --renderer wgpu --mode walking
```

`CAT_DESK_PET_RENDERER=native|wgpu` is equivalent to the command-line switch. A default binary cannot enable `wgpu`; if it is requested, the application logs that the feature is absent and continues with native. If adapter, Surface, or transparent-alpha initialization fails in a feature build, the application logs the error and falls back to native.

## Direct and fallback paths

The experimental backend reports its active path to stderr:

- `wgpu path=atlas-direct`: indexed RG8 atlas + palette texture, per-layer transforms, bed/sleep marks, bird, laser, and laser trail are encoded directly on the GPU.
- `wgpu path=native-upload-fallback`: unsupported interactive content is rendered by `NativeRenderer` and uploaded to the same transparent wgpu Surface. The fallback is allocated lazily.

The fixed sleeping, walking, and bird-plus-laser benchmark scenes use `atlas-direct`. The realistic fixed idle scene can produce action bubbles or particles and is therefore a hybrid run. Feed, gifts, text bubbles, particles, non-laser toys, and butterfly currently select the fallback. This preserves behavior while keeping the benchmark honest: a run that enters fallback must not be labelled `wgpu-atlas` in comparison data.

Both paths share `RenderSnapshot` and `FrameKey`. An unchanged frame is not acquired, encoded, submitted, or presented unless the window system explicitly requests a forced redraw.

## Surface and alpha handling

- macOS requests Metal; Windows requests D3D12; other targets use wgpu's primary backends.
- The adapter preference is `LowPower`.
- Surface alpha selection is `PreMultiplied`, then `PostMultiplied`, then `Inherit`. Opaque-only surfaces are rejected.
- A `PostMultiplied` Surface uses one premultiplied offscreen target and a final unpremultiply pass. This avoids a black background or dark halo on the macOS Metal Surface observed during development.
- Depth, MSAA, compute, mipmaps, and continuous 60 FPS presentation are disabled.
- Swapchain frame latency is limited to two.

The CPU-side atlas metadata remains authoritative for hit testing; the renderer never performs a synchronous GPU readback.

## Scope before a default decision

The feature is suitable for same-machine A/B measurement, not yet for release default selection. A default switch still requires:

- repeated CPU, RSS, physical-footprint, startup, and whole-system energy/GPU evidence;
- pixel/hit-mask and real-window checks at 1x and Retina scale;
- native behavior checks for transparent, topmost, click-through, drag, tray, and multi-display operation;
- a real Windows D3D12 transparency and tray smoke test;
- either direct GPU implementations for remaining content or an explicit, measured hybrid activation rule.

See [`renderer-benchmark.md`](renderer-benchmark.md) for the measurement protocol.
