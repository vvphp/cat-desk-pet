//! CJK-capable text raster for speech bubbles / particles (system font + fontdue).

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

static FONT: OnceLock<Option<Font>> = OnceLock::new();

fn load_font() -> Option<Font> {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/PingFang.ttc",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if let Ok(font) = Font::from_bytes(bytes.as_slice(), FontSettings::default()) {
            return Some(font);
        }
    }
    None
}

fn font() -> Option<&'static Font> {
    FONT.get_or_init(load_font).as_ref()
}

/// Measure text width/height at `px` size.
pub fn measure(text: &str, px: f32) -> (f32, f32) {
    let Some(font) = font() else {
        return ((text.chars().count() as f32) * px * 0.6, px);
    };
    let mut w = 0.0f32;
    let mut h = px;
    for ch in text.chars() {
        let m = font.metrics(ch, px);
        w += m.advance_width;
        h = h.max(m.height as f32 + (m.ymin as f32).abs());
    }
    (w, h.max(px))
}

/// Blit `text` into an ARGB buffer (coverage → alpha, tinted rgb).
pub fn blit_text(
    buf: &mut [u32],
    bw: u32,
    bh: u32,
    x: f64,
    y: f64,
    text: &str,
    px: f32,
    r: u8,
    g: u8,
    b: u8,
    a_mul: f32,
) {
    let Some(font) = font() else {
        let mut cx = x;
        for _ in text.chars() {
            fill_block(
                buf,
                bw,
                bh,
                cx,
                y,
                px as f64 * 0.45,
                px as f64 * 0.7,
                r,
                g,
                b,
                a_mul,
            );
            cx += px as f64 * 0.55;
        }
        return;
    };

    let mut pen_x = x as f32;
    let baseline = y as f32 + px * 0.85;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, px);
        let gx = pen_x + metrics.xmin as f32;
        let gy = baseline - metrics.height as f32 - metrics.ymin as f32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let cov = bitmap[row * metrics.width + col] as f32 / 255.0;
                if cov < 0.02 {
                    continue;
                }
                let px_x = (gx + col as f32).round() as i32;
                let px_y = (gy + row as f32).round() as i32;
                if px_x < 0 || px_y < 0 || px_x >= bw as i32 || px_y >= bh as i32 {
                    continue;
                }
                let a = (cov * a_mul * 255.0).clamp(0.0, 255.0) as u8;
                blend(buf, bw, px_x as u32, px_y as u32, r, g, b, a);
            }
        }
        pen_x += metrics.advance_width;
    }
}

fn fill_block(
    buf: &mut [u32],
    bw: u32,
    bh: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: u8,
    g: u8,
    b: u8,
    a_mul: f32,
) {
    let a = (a_mul * 255.0).clamp(0.0, 255.0) as u8;
    let x0 = x.floor().max(0.0) as i32;
    let y0 = y.floor().max(0.0) as i32;
    let x1 = (x + w).ceil().min(bw as f64) as i32;
    let y1 = (y + h).ceil().min(bh as f64) as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            blend(buf, bw, px as u32, py as u32, r, g, b, a);
        }
    }
}

fn blend(buf: &mut [u32], bw: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    let i = (y as usize) * (bw as usize) + (x as usize);
    if i >= buf.len() || a == 0 {
        return;
    }
    let dst = buf[i];
    let da = (dst >> 24) & 0xff;
    let dr = (dst >> 16) & 0xff;
    let dg = (dst >> 8) & 0xff;
    let db = dst & 0xff;
    let aa = a as u32;
    let inv = 255 - aa;
    let out_a = aa + (da * inv) / 255;
    let out_r = (r as u32 * aa + dr * inv) / 255;
    let out_g = (g as u32 * aa + dg * inv) / 255;
    let out_b = (b as u32 * aa + db * inv) / 255;
    buf[i] = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
}
