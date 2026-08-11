//! Runtime view of the generated indexed pet atlas.

use crate::pet::CoatColor;

#[derive(Clone, Copy, Debug)]
pub struct AtlasRegion {
    pub name: &'static str,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Crop origin in 3x source pixels.
    pub source_x: u32,
    pub source_y: u32,
    /// Semantic palette roles that this independently rasterized region may contain.
    pub allowed_roles: &'static [u8],
}

#[allow(dead_code)] // ATLAS_HEIGHT is only used by tests and the optional GPU backend.
mod generated {
    use super::AtlasRegion;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/generated/pet-atlas.rs"
    ));
}

#[allow(unused_imports)] // Phase 3 uploads the full texture.
pub use generated::ATLAS_HEIGHT;
pub use generated::{ATLAS_SCALE, ATLAS_WIDTH};

const PIXELS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/generated/pet-atlas.rg8"
));

/// Raw indexed pixels are consumed directly by the Phase 3 GPU backend.
#[allow(dead_code)]
pub fn pixels() -> &'static [u8] {
    PIXELS
}

pub fn region(name: &str) -> Option<&'static AtlasRegion> {
    generated::REGIONS.iter().find(|region| region.name == name)
}

pub fn sample(region: &AtlasRegion, source_x: f64, source_y: f64) -> (u8, u8) {
    let px = (source_x * ATLAS_SCALE).floor() as i32;
    let py = (source_y * ATLAS_SCALE).floor() as i32;
    let local_x = px - region.source_x as i32;
    let local_y = py - region.source_y as i32;
    if local_x < 0
        || local_y < 0
        || local_x >= region.width as i32
        || local_y >= region.height as i32
    {
        return (0, 0);
    }
    let atlas_x = region.x + local_x as u32;
    let atlas_y = region.y + local_y as u32;
    let index = ((atlas_y * ATLAS_WIDTH + atlas_x) * 2) as usize;
    (PIXELS[index], PIXELS[index + 1])
}

#[derive(Clone, Copy)]
struct CoatPalette {
    body: [u8; 4],
    body_dark: [u8; 4],
    belly: [u8; 4],
    inner_ear: [u8; 4],
    nose: [u8; 4],
    eye: [u8; 4],
    whisker: [u8; 4],
    blush: [u8; 4],
    accent: [u8; 4],
    snout: [u8; 4],
}

const fn rgba(rgb: u32) -> [u8; 4] {
    [
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        255,
    ]
}

const fn transparent() -> [u8; 4] {
    [0, 0, 0, 0]
}

fn palette(coat: CoatColor) -> CoatPalette {
    match coat {
        CoatColor::Orange => CoatPalette {
            body: rgba(0xF4A56B),
            body_dark: rgba(0xC66A2C),
            belly: rgba(0xFFF1DC),
            inner_ear: rgba(0xFFB8C1),
            nose: rgba(0xE36B7A),
            eye: rgba(0x2D1A0A),
            whisker: rgba(0x5A3A24),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xC66A2C),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Calico => CoatPalette {
            body: rgba(0xFFF1DC),
            body_dark: rgba(0x2C2828),
            belly: rgba(0xFFFFFF),
            inner_ear: rgba(0xFFB8C1),
            nose: rgba(0xE36B7A),
            eye: rgba(0x2D1A0A),
            whisker: rgba(0x6A5A4A),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xF4A56B),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Cow => CoatPalette {
            body: rgba(0xFFFFFF),
            body_dark: rgba(0x2C2828),
            belly: rgba(0xFFFFFF),
            inner_ear: rgba(0xFFB8C1),
            nose: rgba(0xFF85A1),
            eye: rgba(0x2D1A0A),
            whisker: rgba(0x6A5A4A),
            blush: rgba(0xFFB3BA),
            accent: rgba(0x2C2828),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Tabby => CoatPalette {
            body: rgba(0xA89E91),
            body_dark: rgba(0x5C544A),
            belly: rgba(0xDAD2C6),
            inner_ear: rgba(0xE8A8B0),
            nose: rgba(0x3C3530),
            eye: rgba(0x1A1A1A),
            whisker: rgba(0xFFFFFF),
            blush: rgba(0xFFB3BA),
            accent: rgba(0x5C544A),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Tuxedo => CoatPalette {
            body: rgba(0x2A2828),
            body_dark: rgba(0x000000),
            belly: rgba(0xFFFFFF),
            inner_ear: rgba(0xFFB8C1),
            nose: rgba(0xFF85A1),
            eye: rgba(0xFFD23B),
            whisker: rgba(0xFFFFFF),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xFFFFFF),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Pink => CoatPalette {
            body: rgba(0xF5B5C0),
            body_dark: rgba(0xC2778A),
            belly: rgba(0xFFE6EE),
            inner_ear: rgba(0xE8929E),
            nose: rgba(0xC2546F),
            eye: rgba(0x2D1A0A),
            whisker: transparent(),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xC2778A),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Cream => CoatPalette {
            body: rgba(0xFFE0C2),
            body_dark: rgba(0xD8B591),
            belly: rgba(0xFFF4E0),
            inner_ear: rgba(0xE8B998),
            nose: rgba(0xB27858),
            eye: rgba(0x2D1A0A),
            whisker: transparent(),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xD8B591),
            snout: rgba(0xE8C6A6),
        },
        CoatColor::Brown => CoatPalette {
            body: rgba(0xA87248),
            body_dark: rgba(0x5E3D1F),
            belly: rgba(0xD2A878),
            inner_ear: rgba(0x6A4525),
            nose: rgba(0x1A0F0F),
            eye: rgba(0x2D1A0A),
            whisker: transparent(),
            blush: rgba(0xFFB3BA),
            accent: rgba(0x5E3D1F),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Black => CoatPalette {
            body: rgba(0x3C2D2D),
            body_dark: rgba(0x1A0F0F),
            belly: rgba(0x5E4C4C),
            inner_ear: rgba(0x2A1818),
            nose: rgba(0x000000),
            eye: rgba(0xFFD23B),
            whisker: transparent(),
            blush: rgba(0xFFB3BA),
            accent: rgba(0x1A0F0F),
            snout: rgba(0xE89DAE),
        },
        CoatColor::Polar => CoatPalette {
            body: rgba(0xF2EDE2),
            body_dark: rgba(0xC8C0B0),
            belly: rgba(0xFFFFFF),
            inner_ear: rgba(0xD8D0C0),
            nose: rgba(0x1A1A1A),
            eye: rgba(0x2D1A0A),
            whisker: transparent(),
            blush: rgba(0xFFB3BA),
            accent: rgba(0xC8C0B0),
            snout: rgba(0xE89DAE),
        },
    }
}

pub fn role_color(coat: CoatColor, role: u8) -> [u8; 4] {
    let palette = palette(coat);
    match role {
        1 => palette.body,
        2 => palette.body_dark,
        3 => palette.belly,
        4 => palette.inner_ear,
        5 => palette.nose,
        6 => palette.eye,
        7 => palette.whisker,
        8 => palette.blush,
        9 => palette.accent,
        10 => palette.snout,
        11 => rgba(0xFFFFFF),
        12 => rgba(0x3A2A20),
        13 => rgba(0xE76A8A),
        14 => rgba(0xE54F6B),
        15 => rgba(0xFFD23B),
        16 => rgba(0xB47A30),
        17 => rgba(0x2D1A0A),
        18 => rgba(0x1A1A1A),
        19 => rgba(0xC2546F),
        _ => transparent(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn generated_atlas_manifest_is_valid() {
        assert_eq!(
            PIXELS.len(),
            (ATLAS_WIDTH * ATLAS_HEIGHT * 2) as usize,
            "RG8 byte length"
        );
        let mut names = HashSet::new();
        for region in generated::REGIONS {
            assert!(names.insert(region.name), "duplicate {}", region.name);
            assert!(
                region.width > 0 && region.height > 0,
                "empty {}",
                region.name
            );
            assert!(
                region.x + region.width <= ATLAS_WIDTH,
                "x bounds {}",
                region.name
            );
            assert!(
                region.y + region.height <= ATLAS_HEIGHT,
                "y bounds {}",
                region.name
            );
            assert!(!region.allowed_roles.is_empty(), "roles {}", region.name);
            for &role in region.allowed_roles {
                assert!((1..=19).contains(&role), "role {role} in {}", region.name);
            }
            for y in 0..region.height {
                for x in 0..region.width {
                    let index = (((region.y + y) * ATLAS_WIDTH + region.x + x) * 2) as usize;
                    let role = PIXELS[index];
                    let coverage = PIXELS[index + 1];
                    if coverage != 0 {
                        assert!(
                            region.allowed_roles.contains(&role),
                            "illegal role {role} in {} at ({x}, {y})",
                            region.name
                        );
                    }
                }
            }
        }
        for (index, left) in generated::REGIONS.iter().enumerate() {
            for right in &generated::REGIONS[index + 1..] {
                let overlaps = left.x < right.x + right.width
                    && left.x + left.width > right.x
                    && left.y < right.y + right.height
                    && left.y + left.height > right.y;
                assert!(!overlaps, "{} overlaps {}", left.name, right.name);
            }
        }
    }

    #[test]
    fn palette_roles_include_transparent_whiskers() {
        assert_eq!(role_color(CoatColor::Orange, 1), [0xF4, 0xA5, 0x6B, 255]);
        assert_eq!(role_color(CoatColor::Pink, 7)[3], 0);
        assert_eq!(role_color(CoatColor::Tuxedo, 6), [0xFF, 0xD2, 0x3B, 255]);
    }

    #[test]
    fn eye_normal_is_limited_to_eye_and_white_roles() {
        let eye = region("eye-normal").expect("eye-normal region");
        assert_eq!(eye.allowed_roles, &[6, 11]);
    }
}
