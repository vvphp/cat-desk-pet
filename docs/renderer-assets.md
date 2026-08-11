# Layered atlas contract

Issue #6 uses one deterministic indexed atlas as the shared pet-body asset for
the native renderer and the experimental `wgpu` renderer. The committed output
is generated explicitly; an ordinary `cargo build` never parses or rasterizes
SVG.

## Generate and verify

```bash
cargo run --bin asset-compiler --features asset-compiler
cargo run --bin asset-compiler --features asset-compiler -- --check
```

Inputs and outputs:

- source: `assets/pet.svg`
- indexed RG8 atlas: `assets/generated/pet-atlas.rg8`
- generated region manifest: `assets/generated/pet-atlas.rs`

`--check` recompiles the assets in memory and fails if either committed output
is stale. Running the compiler twice with identical input must produce identical
bytes.

## Format

The atlas is RG8 rather than pre-colored RGBA:

- R: semantic palette role (`body`, `body-dark`, `belly`, eyes, fixed details,
  and so on)
- G: alpha coverage

The runtime maps roles to the selected coat palette. Species, pattern, eyes,
mouth, tail, and legs are independent atlas regions. Tail rotation and leg
translation remain transforms, so the compiler does not generate the forbidden
`species x coat x eyes x mouth x pose` Cartesian product.

Each region declares its allowed semantic roles. Antialiased overlaps are
quantized only within that region-specific set, preventing blended eye/body
edges from becoming unrelated roles. Tests scan every non-transparent atlas
pixel and reject roles outside its region allowlist.

The source is rasterized once at 3x. Native composition samples the same indexed
regions into its DPR-quantized cache. The `wgpu` backend can upload the RG8 bytes
directly and apply the same palette and transforms in its shader.

## Runtime guarantees

- `resvg` is optional and only enabled by the `asset-compiler` feature
- the default binary embeds the generated atlas and manifest, not the SVG parser
- native retains its bounded LRU for composed pose frames
- hit testing stays CPU-side and does not read back the atlas or GPU surface
- representative native frames are compared with a direct SVG reference by
  `cargo test --features asset-compiler`
