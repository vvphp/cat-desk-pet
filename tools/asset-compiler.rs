//! Explicit, deterministic compiler for the indexed layered pet atlas.
//!
//! Run with:
//!   cargo run --bin asset-compiler --features asset-compiler
//!   cargo run --bin asset-compiler --features asset-compiler -- --check

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

const SOURCE: &str = include_str!("../assets/pet.svg");
const SCALE: u32 = 3;
const LOGICAL_W: u32 = 120;
const LOGICAL_H: u32 = 110;
const ATLAS_W: u32 = 2048;
const PAD: u32 = 2;

// Marker RGB values are deliberately unrelated to the product palette. The
// runtime uses the role byte to apply the selected coat without baking the
// species x coat x face x pose Cartesian product into the atlas.
const ROLE_MARKERS: &[[u8; 3]] = &[
    [0, 0, 0],
    [17, 239, 53],   // body
    [31, 223, 79],   // body dark
    [47, 211, 101],  // belly
    [61, 197, 127],  // inner ear
    [73, 181, 149],  // nose
    [89, 167, 173],  // eye
    [103, 151, 197], // whisker
    [127, 137, 211], // blush
    [139, 113, 229], // accent
    [157, 97, 241],  // snout
    [173, 83, 199],  // white
    [191, 71, 181],  // mouth ink
    [207, 59, 163],  // tongue
    [223, 43, 139],  // heart
    [239, 31, 113],  // star
    [251, 19, 89],   // star outline
    [229, 101, 37],  // fixed dark brown
    [211, 127, 23],  // fixed black
    [193, 149, 11],  // mouth-open pink
];

const COMPONENTS: &str = "#layer-shadow,#tail,#body-shell,.pattern,#belly-default,#cat-ears,#face-muzzle,#eye-l,#eye-r,#cat-nose,#mouth,#blush,#whisker-l,#whisker-r,#leg-fl,#leg-fr,#tail-pig,#species-pig,#tail-bear,#species-bear";

struct Layer {
    name: &'static str,
    show: &'static str,
    allowed_roles: &'static [u8],
}

const LAYERS: &[Layer] = &[
    Layer {
        name: "shadow",
        show: "#layer-shadow",
        allowed_roles: &[18],
    },
    Layer {
        name: "tail-cat",
        show: "#tail",
        allowed_roles: &[1],
    },
    Layer {
        name: "body-shell",
        show: "#body-shell",
        allowed_roles: &[1],
    },
    Layer {
        name: "pattern-stripes",
        show: ".pattern-stripes",
        allowed_roles: &[2],
    },
    Layer {
        name: "pattern-patches",
        show: ".pattern-patches",
        allowed_roles: &[2, 9],
    },
    Layer {
        name: "pattern-cow",
        show: ".pattern-cow",
        allowed_roles: &[2],
    },
    Layer {
        name: "pattern-tuxedo",
        show: ".pattern-tuxedo",
        allowed_roles: &[3],
    },
    Layer {
        name: "belly",
        show: "#belly-default",
        allowed_roles: &[3],
    },
    Layer {
        name: "cat-ears",
        show: "#cat-ears",
        allowed_roles: &[1, 4],
    },
    Layer {
        name: "muzzle",
        show: "#face-muzzle",
        allowed_roles: &[3],
    },
    Layer {
        name: "eye-normal",
        show: "#eye-l,#eye-r,.eye-normal",
        allowed_roles: &[6, 11],
    },
    Layer {
        name: "eye-wide",
        show: "#eye-l,#eye-r,.eye-wide",
        allowed_roles: &[6, 11],
    },
    Layer {
        name: "eye-hearts",
        show: "#eye-l,#eye-r,.eye-hearts",
        allowed_roles: &[11, 14],
    },
    Layer {
        name: "eye-stars",
        show: "#eye-l,#eye-r,.eye-stars",
        allowed_roles: &[11, 15, 16],
    },
    Layer {
        name: "eye-x",
        show: "#eye-l,#eye-r,.eye-x",
        allowed_roles: &[6],
    },
    Layer {
        name: "eye-happy",
        show: "#eye-l,#eye-r,.eye-happy",
        allowed_roles: &[6],
    },
    Layer {
        name: "eye-angry",
        show: "#eye-l,#eye-r,.eye-angry",
        allowed_roles: &[6, 11],
    },
    Layer {
        name: "cat-nose",
        show: "#cat-nose",
        allowed_roles: &[5],
    },
    Layer {
        name: "mouth-normal",
        show: "#mouth,.mouth-normal",
        allowed_roles: &[12],
    },
    Layer {
        name: "mouth-smile",
        show: "#mouth,.mouth-smile",
        allowed_roles: &[12],
    },
    Layer {
        name: "mouth-grumpy",
        show: "#mouth,.mouth-grumpy",
        allowed_roles: &[12],
    },
    Layer {
        name: "mouth-tongue",
        show: "#mouth,.mouth-tongue",
        allowed_roles: &[12, 13],
    },
    Layer {
        name: "mouth-shy",
        show: "#mouth,.mouth-shy",
        allowed_roles: &[12],
    },
    Layer {
        name: "mouth-pursed",
        show: "#mouth,.mouth-pursed",
        allowed_roles: &[12],
    },
    Layer {
        name: "blush",
        show: "#blush",
        allowed_roles: &[8],
    },
    Layer {
        name: "whiskers",
        show: "#whisker-l,#whisker-r",
        allowed_roles: &[7],
    },
    Layer {
        name: "leg-fl",
        show: "#leg-fl",
        allowed_roles: &[2],
    },
    Layer {
        name: "leg-fr",
        show: "#leg-fr",
        allowed_roles: &[2],
    },
    Layer {
        name: "tail-pig",
        show: "#tail-pig",
        allowed_roles: &[2],
    },
    Layer {
        name: "species-pig",
        show: "#species-pig",
        allowed_roles: &[2, 4, 10, 17],
    },
    Layer {
        name: "tail-bear",
        show: "#tail-bear",
        allowed_roles: &[1],
    },
    Layer {
        name: "species-bear",
        show: "#species-bear",
        allowed_roles: &[1, 4, 11, 18],
    },
];

struct LayerImage {
    name: &'static str,
    allowed_roles: &'static [u8],
    crop_x: u32,
    crop_y: u32,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    atlas_x: u32,
    atlas_y: u32,
}

fn marker_hex(role: u8) -> String {
    let [r, g, b] = ROLE_MARKERS[role as usize];
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn indexed_svg(layer: &Layer) -> String {
    let mut svg = SOURCE.to_owned();
    for (token, role) in [
        ("var(--accent, #F4A56B)", 9),
        ("var(--snout, #E89DAE)", 10),
        ("var(--body-dark)", 2),
        ("var(--body)", 1),
        ("var(--belly)", 3),
        ("var(--inner-ear)", 4),
        ("var(--nose)", 5),
        ("var(--eye)", 6),
        ("var(--whisker)", 7),
        ("var(--blush)", 8),
        ("var(--accent)", 9),
        ("var(--snout)", 10),
    ] {
        svg = svg.replace(token, &marker_hex(role));
    }
    for (token, role) in [
        ("#FFFFFF", 11),
        ("#3A2A20", 12),
        ("#E76A8A", 13),
        ("#E54F6B", 14),
        ("#FFD23B", 15),
        ("#B47A30", 16),
        ("#2D1A0A", 17),
        ("#1A1A1A", 18),
        ("#000000", 18),
        ("#C2546F", 19),
    ] {
        svg = svg.replace(token, &marker_hex(role));
    }

    let style = format!(
        "<style>{COMPONENTS}{{display:none}}.eye-style,.mouth-style{{display:none}}{}{{display:inline}}</style>",
        layer.show
    );
    if let Some(open_end) = svg.find('>') {
        svg.insert_str(open_end + 1, &style);
    }
    svg
}

fn nearest_role(r: u8, g: u8, b: u8, allowed_roles: &[u8]) -> u8 {
    let mut best = allowed_roles[0] as usize;
    let mut best_dist = u32::MAX;
    for &role in allowed_roles {
        let role = role as usize;
        let marker = ROLE_MARKERS[role];
        let dr = r as i32 - marker[0] as i32;
        let dg = g as i32 - marker[1] as i32;
        let db = b as i32 - marker[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best = role;
            best_dist = dist;
        }
    }
    best as u8
}

fn render_layer(layer: &Layer) -> Result<LayerImage, String> {
    if layer.allowed_roles.is_empty()
        || layer
            .allowed_roles
            .iter()
            .any(|&role| role == 0 || role as usize >= ROLE_MARKERS.len())
    {
        return Err(format!("{} has invalid allowed roles", layer.name));
    }
    let svg = indexed_svg(layer);
    let tree = Tree::from_str(&svg, &Options::default())
        .map_err(|e| format!("{}: SVG parse failed: {e}", layer.name))?;
    let width = LOGICAL_W * SCALE;
    let height = LOGICAL_H * SCALE;
    let mut pixmap = Pixmap::new(width, height).ok_or("cannot allocate layer pixmap")?;
    resvg::render(
        &tree,
        Transform::from_scale(SCALE as f32, SCALE as f32),
        &mut pixmap.as_mut(),
    );

    let mut indexed = vec![0u8; (width * height * 2) as usize];
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for (i, pixel) in pixmap.pixels().iter().enumerate() {
        let a = pixel.alpha();
        if a == 0 {
            continue;
        }
        let unpremul = |v: u8| -> u8 {
            if a == 255 {
                v
            } else {
                ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
            }
        };
        let role = nearest_role(
            unpremul(pixel.red()),
            unpremul(pixel.green()),
            unpremul(pixel.blue()),
            layer.allowed_roles,
        );
        indexed[i * 2] = role;
        indexed[i * 2 + 1] = a;
        let x = i as u32 % width;
        let y = i as u32 / width;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if min_x > max_x || min_y > max_y {
        return Err(format!("{} produced no pixels", layer.name));
    }

    min_x = min_x.saturating_sub(PAD);
    min_y = min_y.saturating_sub(PAD);
    max_x = (max_x + PAD).min(width - 1);
    max_y = (max_y + PAD).min(height - 1);
    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;
    let mut cropped = vec![0u8; (crop_w * crop_h * 2) as usize];
    for y in 0..crop_h {
        let src = (((min_y + y) * width + min_x) * 2) as usize;
        let dst = (y * crop_w * 2) as usize;
        let len = (crop_w * 2) as usize;
        cropped[dst..dst + len].copy_from_slice(&indexed[src..src + len]);
    }
    Ok(LayerImage {
        name: layer.name,
        allowed_roles: layer.allowed_roles,
        crop_x: min_x,
        crop_y: min_y,
        width: crop_w,
        height: crop_h,
        pixels: cropped,
        atlas_x: 0,
        atlas_y: 0,
    })
}

fn compile() -> Result<(Vec<u8>, String), String> {
    let mut layers = LAYERS
        .iter()
        .map(render_layer)
        .collect::<Result<Vec<_>, _>>()?;

    let mut cursor_x = PAD;
    let mut cursor_y = PAD;
    let mut row_h = 0;
    for layer in &mut layers {
        if cursor_x + layer.width + PAD > ATLAS_W {
            cursor_x = PAD;
            cursor_y += row_h + PAD;
            row_h = 0;
        }
        layer.atlas_x = cursor_x;
        layer.atlas_y = cursor_y;
        cursor_x += layer.width + PAD;
        row_h = row_h.max(layer.height);
    }
    let atlas_h = cursor_y + row_h + PAD;
    let mut atlas = vec![0u8; (ATLAS_W * atlas_h * 2) as usize];
    for layer in &layers {
        for y in 0..layer.height {
            let src = (y * layer.width * 2) as usize;
            let dst = (((layer.atlas_y + y) * ATLAS_W + layer.atlas_x) * 2) as usize;
            let len = (layer.width * 2) as usize;
            atlas[dst..dst + len].copy_from_slice(&layer.pixels[src..src + len]);
        }
    }

    let mut manifest = String::new();
    writeln!(
        manifest,
        "// @generated by tools/asset-compiler.rs; do not edit."
    )
    .unwrap();
    writeln!(manifest, "pub const ATLAS_WIDTH: u32 = {ATLAS_W};").unwrap();
    writeln!(manifest, "pub const ATLAS_HEIGHT: u32 = {atlas_h};").unwrap();
    writeln!(manifest, "pub const ATLAS_SCALE: f64 = {SCALE}.0;").unwrap();
    writeln!(manifest, "pub const REGIONS: &[AtlasRegion] = &[").unwrap();
    for layer in &layers {
        writeln!(
            manifest,
            "    AtlasRegion {{ name: {:?}, x: {}, y: {}, width: {}, height: {}, source_x: {}, source_y: {}, allowed_roles: &{:?} }},",
            layer.name,
            layer.atlas_x,
            layer.atlas_y,
            layer.width,
            layer.height,
            layer.crop_x,
            layer.crop_y,
            layer.allowed_roles,
        )
        .unwrap();
    }
    writeln!(manifest, "];").unwrap();
    Ok((atlas, manifest))
}

fn ensure_matches(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if actual != expected {
        return Err(format!(
            "{} is stale; run the asset compiler without --check",
            path.display()
        ));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let check = std::env::args().skip(1).any(|arg| arg == "--check");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = root.join("assets/generated");
    let atlas_path = output_dir.join("pet-atlas.rg8");
    let manifest_path = output_dir.join("pet-atlas.rs");
    let (atlas, manifest) = compile()?;

    if check {
        ensure_matches(&atlas_path, &atlas)?;
        ensure_matches(&manifest_path, manifest.as_bytes())?;
        println!("layered atlas is current ({} bytes)", atlas.len());
    } else {
        fs::create_dir_all(&output_dir).map_err(|e| format!("{}: {e}", output_dir.display()))?;
        fs::write(&atlas_path, &atlas).map_err(|e| format!("{}: {e}", atlas_path.display()))?;
        fs::write(&manifest_path, manifest)
            .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
        println!(
            "wrote {} and {} ({} bytes)",
            atlas_path.display(),
            manifest_path.display(),
            atlas.len()
        );
    }
    Ok(())
}
