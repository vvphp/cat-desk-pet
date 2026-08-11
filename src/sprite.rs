//! Compose the generated indexed layered atlas for native softbuffer blit.
//!
//! Appearance (coat / species / face) and quantized leg/tail poses are cached
//! with a hard LRU cap — unbounded pose keys were leaking RAM until the
//! process got killed (tray disappears with it).

use std::collections::{HashMap, VecDeque};
use std::f64::consts::TAU;

/// Base LRU cap at 1× (~53KB/slot). Retina uses a higher cap — re-raster is dearer.
const MAX_CACHE_1X: usize = 32;
const MAX_CACHE_RETINA: usize = 48;

use crate::atlas::{self, AtlasRegion};
use crate::pet::{CoatColor, IdleAction, Mode, Species, TrickAction};
use crate::renderer::RenderSnapshot;

/// Logical sprite size — matches WebView `#cat` (120×110 CSS px).
pub const SPRITE_W: u32 = 120;
pub const SPRITE_H: u32 = 110;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SpriteKey {
    species: Species,
    coat: CoatColor,
    eyes: EyeStyle,
    mouth: MouthStyle,
    /// Tail rotation in degrees, quantized (≈4°).
    tail_q: i8,
    /// Front-leg Y translate in SVG units, quantized (≈1).
    leg_fl_q: i8,
    leg_fr_q: i8,
    /// Device pixel ratio in quarter-units (4=1.0 … 12=3.0) — shared by raster + blit.
    dpr_q: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Angry reserved for future mood wiring
enum EyeStyle {
    Normal,
    Wide,
    Hearts,
    Stars,
    X,
    Happy,
    Angry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Shy reserved for future mood wiring
enum MouthStyle {
    Normal,
    Smile,
    Grumpy,
    Tongue,
    Shy,
    Pursed,
}

#[derive(Clone, Copy, Debug)]
struct AnimPose {
    tail_deg: f64,
    leg_fl: f64,
    leg_fr: f64,
}

pub struct SpriteCache {
    cache: HashMap<SpriteKey, Vec<u32>>,
    order: VecDeque<SpriteKey>,
}

impl SpriteCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn pixels_for(&mut self, pet: &RenderSnapshot<'_>, scale: f64) -> &[u32] {
        let key = key_for(pet, scale);
        if self.cache.contains_key(&key) {
            if let Some(i) = self.order.iter().position(|k| *k == key) {
                let k = self.order.remove(i).expect("index from position");
                self.order.push_back(k);
            }
        } else {
            let cap = cache_cap(key.dpr_q);
            while self.cache.len() >= cap {
                if let Some(old) = self.order.pop_front() {
                    self.cache.remove(&old);
                } else {
                    break;
                }
            }
            let px = compose(key);
            self.cache.insert(key, px);
            self.order.push_back(key);
        }
        self.cache.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Quantize DPR to 0.25 steps so pixmap size and blit layout share one `d`.
fn dpr_quant(scale: f64) -> u8 {
    ((scale.clamp(1.0, 3.0) * 4.0).round() as i32).clamp(4, 12) as u8
}

fn dpr_of(q: u8) -> f64 {
    q as f64 / 4.0
}

/// Same quantized DPR used for sprite raster size and blit placement.
pub fn layout_scale(scale: f64) -> f64 {
    dpr_of(dpr_quant(scale))
}

fn cache_cap(dpr_q: u8) -> usize {
    if dpr_of(dpr_q) >= 1.5 {
        MAX_CACHE_RETINA
    } else {
        MAX_CACHE_1X
    }
}

fn sprite_px(dpr_q: u8) -> (u32, u32) {
    let d = dpr_of(dpr_q);
    (
        (SPRITE_W as f64 * d).round().max(1.0) as u32,
        (SPRITE_H as f64 * d).round().max(1.0) as u32,
    )
}

fn key_for(pet: &RenderSnapshot<'_>, scale: f64) -> SpriteKey {
    let pose = pose_for(pet);
    SpriteKey {
        species: pet.species,
        coat: pet.coat,
        eyes: eyes_for(pet),
        mouth: mouth_for(pet),
        // Coarser steps → fewer unique keys while walking.
        tail_q: quantize(pose.tail_deg, 8.0, -36.0, 36.0),
        leg_fl_q: quantize(pose.leg_fl, 2.0, -10.0, 8.0),
        leg_fr_q: quantize(pose.leg_fr, 2.0, -10.0, 8.0),
        dpr_q: dpr_quant(scale),
    }
}

fn quantize(v: f64, step: f64, lo: f64, hi: f64) -> i8 {
    let clamped = v.clamp(lo, hi);
    let q = (clamped / step).round() * step;
    q.round() as i8
}

fn pose_for(pet: &RenderSnapshot<'_>) -> AnimPose {
    match pet.mode {
        Mode::Walking
        | Mode::GoingHome
        | Mode::Clingy
        | Mode::Interested
        | Mode::Chasing
        | Mode::Playing
        | Mode::Gifting => {
            let phase = pet.walk_phase;
            let run = matches!(pet.mode, Mode::Chasing | Mode::Playing);
            let intensity = if run { 1.5 } else { 1.0 };
            let tail_amp = if run { 22.0 } else { 14.0 };
            AnimPose {
                tail_deg: phase.sin() * tail_amp,
                leg_fl: -phase.sin().max(0.0) * 3.0 * intensity,
                leg_fr: -(phase + std::f64::consts::PI).sin().max(0.0) * 3.0 * intensity,
            }
        }
        Mode::Idle => idle_pose(pet),
        Mode::Pet => AnimPose {
            tail_deg: (pet.walk_phase.max(pet.idle_t) * 4.0 + 1.0).sin() * 22.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        Mode::Sleeping | Mode::InBed => AnimPose {
            tail_deg: (pet.sleep_t * 0.9).sin() * 4.0,
            leg_fl: 2.0,
            leg_fr: 2.0,
        },
        Mode::Dragged => AnimPose {
            tail_deg: 18.0,
            leg_fl: 3.0,
            leg_fr: 3.0,
        },
        Mode::Dizzy => AnimPose {
            tail_deg: (pet.dizzy_t * 18.0).sin() * 26.0,
            leg_fl: (pet.dizzy_t * 14.0).sin() * 2.0,
            leg_fr: -(pet.dizzy_t * 14.0).sin() * 2.0,
        },
        Mode::Feeding => {
            if pet.eat_anim_t > 0.0 {
                AnimPose {
                    tail_deg: (pet.eat_anim_t * 10.0).sin() * 10.0,
                    leg_fl: 0.0,
                    leg_fr: 0.0,
                }
            } else {
                let phase = pet.walk_phase;
                AnimPose {
                    tail_deg: phase.sin() * 14.0,
                    leg_fl: -phase.sin().max(0.0) * 3.0,
                    leg_fr: -(phase + std::f64::consts::PI).sin().max(0.0) * 3.0,
                }
            }
        }
        Mode::Watching | Mode::BirdWatch | Mode::ButterflyNose => AnimPose {
            tail_deg: (pet.idle_t.max(pet.walk_phase) * 1.2).sin() * 10.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        Mode::Startled => AnimPose {
            tail_deg: 20.0,
            leg_fl: -2.0,
            leg_fr: -2.0,
        },
        Mode::Photo => AnimPose {
            tail_deg: 8.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        Mode::Trick => match pet.trick_action.unwrap_or(TrickAction::Meow) {
            TrickAction::Spin => AnimPose {
                tail_deg: (pet.walk_phase * 3.0).sin() * 28.0,
                leg_fl: -2.0,
                leg_fr: -2.0,
            },
            TrickAction::Pounce | TrickAction::HappyJump | TrickAction::SwatCursor => AnimPose {
                tail_deg: 16.0,
                leg_fl: -3.0,
                leg_fr: -3.0,
            },
            TrickAction::Grumpy => AnimPose {
                tail_deg: -12.0,
                leg_fl: 1.0,
                leg_fr: 1.0,
            },
            TrickAction::Heart | TrickAction::Kiss => AnimPose {
                tail_deg: (pet.walk_phase * 4.0).sin() * 18.0,
                leg_fl: 0.0,
                leg_fr: 0.0,
            },
            _ => AnimPose {
                tail_deg: (pet.walk_phase * 3.0).sin() * 14.0,
                leg_fl: 0.0,
                leg_fr: 0.0,
            },
        },
    }
}

fn idle_pose(pet: &RenderSnapshot<'_>) -> AnimPose {
    let t = pet.idle_t;
    let action_t = pet.idle_action_t;
    match pet.idle_action {
        IdleAction::Sit => AnimPose {
            tail_deg: (t * 1.4).sin() * 12.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        IdleAction::Yawn => AnimPose {
            tail_deg: (t * 1.25).sin() * 6.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        IdleAction::Stretch => AnimPose {
            tail_deg: (t * 2.5).sin() * 18.0,
            leg_fl: -1.0,
            leg_fr: -1.0,
        },
        IdleAction::Look => AnimPose {
            tail_deg: (t * 1.25).sin() * 8.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        IdleAction::TailCurl => AnimPose {
            tail_deg: -28.0,
            leg_fl: 0.0,
            leg_fr: 0.0,
        },
        IdleAction::MudRoll => {
            let k = (action_t / IdleAction::MudRoll.duration()).clamp(0.0, 1.0);
            AnimPose {
                tail_deg: (k * TAU).sin() * 16.0,
                leg_fl: -4.0 * (k * TAU).sin().abs(),
                leg_fr: -5.0 * (k * TAU).sin().abs(),
            }
        }
        IdleAction::BackScratch => AnimPose {
            tail_deg: (action_t * 8.0).sin() * 14.0,
            leg_fl: -(action_t * 10.0).sin().max(0.0) * 3.0,
            leg_fr: 1.0,
        },
    }
}

fn eyes_for(pet: &RenderSnapshot<'_>) -> EyeStyle {
    if pet.mode == Mode::Idle {
        return match pet.idle_action {
            IdleAction::Yawn => EyeStyle::Happy,
            IdleAction::Look | IdleAction::Stretch => EyeStyle::Wide,
            IdleAction::MudRoll => EyeStyle::Happy,
            _ => EyeStyle::Normal,
        };
    }
    match pet.mode {
        Mode::Pet => EyeStyle::Hearts,
        Mode::Clingy if pet.clingy_arrived => EyeStyle::Hearts,
        Mode::Clingy => EyeStyle::Wide,
        Mode::Sleeping | Mode::InBed | Mode::GoingHome => EyeStyle::Happy,
        Mode::Dizzy => EyeStyle::X,
        Mode::Photo => EyeStyle::Stars,
        Mode::Watching | Mode::BirdWatch | Mode::ButterflyNose | Mode::Startled => EyeStyle::Wide,
        Mode::Chasing | Mode::Playing | Mode::Interested => EyeStyle::Wide,
        Mode::Feeding if pet.eat_anim_t > 0.0 => EyeStyle::Happy,
        Mode::Gifting if pet.gift.as_ref().map(|g| g.dropped).unwrap_or(false) => EyeStyle::Happy,
        Mode::Dragged => EyeStyle::Wide,
        Mode::Trick => match pet.trick_action {
            Some(TrickAction::Heart | TrickAction::Kiss) => EyeStyle::Hearts,
            Some(TrickAction::Grumpy) => EyeStyle::X,
            Some(TrickAction::Shy) => EyeStyle::Happy,
            Some(TrickAction::Pounce | TrickAction::HappyJump | TrickAction::SwatCursor) => {
                EyeStyle::Wide
            }
            _ => EyeStyle::Happy,
        },
        _ => EyeStyle::Normal,
    }
}

fn mouth_for(pet: &RenderSnapshot<'_>) -> MouthStyle {
    if pet.mode == Mode::Idle {
        return match pet.idle_action {
            IdleAction::Yawn => MouthStyle::Tongue,
            IdleAction::Sit | IdleAction::TailCurl => MouthStyle::Normal,
            IdleAction::Stretch => MouthStyle::Smile,
            _ => MouthStyle::Normal,
        };
    }
    match pet.mode {
        Mode::Pet | Mode::Clingy => MouthStyle::Smile,
        Mode::Sleeping | Mode::InBed => MouthStyle::Normal,
        Mode::Dizzy => MouthStyle::Grumpy,
        Mode::Photo => MouthStyle::Smile,
        Mode::Feeding if pet.eat_anim_t > 0.0 => MouthStyle::Tongue,
        Mode::Gifting if pet.gift.as_ref().map(|g| g.dropped).unwrap_or(false) => MouthStyle::Smile,
        Mode::Playing | Mode::Chasing => MouthStyle::Smile,
        Mode::Startled | Mode::ButterflyNose => MouthStyle::Pursed,
        _ => MouthStyle::Normal,
    }
}

fn pattern_class(coat: CoatColor) -> Option<&'static str> {
    match coat {
        CoatColor::Orange | CoatColor::Tabby => Some("pattern-stripes"),
        CoatColor::Calico => Some("pattern-patches"),
        CoatColor::Cow => Some("pattern-cow"),
        CoatColor::Tuxedo => Some("pattern-tuxedo"),
        _ => None,
    }
}

fn eye_class(e: EyeStyle) -> &'static str {
    match e {
        EyeStyle::Normal => "eye-normal",
        EyeStyle::Wide => "eye-wide",
        EyeStyle::Hearts => "eye-hearts",
        EyeStyle::Stars => "eye-stars",
        EyeStyle::X => "eye-x",
        EyeStyle::Happy => "eye-happy",
        EyeStyle::Angry => "eye-angry",
    }
}

fn mouth_class(m: MouthStyle) -> &'static str {
    match m {
        MouthStyle::Normal => "mouth-normal",
        MouthStyle::Smile => "mouth-smile",
        MouthStyle::Grumpy => "mouth-grumpy",
        MouthStyle::Tongue => "mouth-tongue",
        MouthStyle::Shy => "mouth-shy",
        MouthStyle::Pursed => "mouth-pursed",
    }
}

#[derive(Clone, Copy)]
struct LayerTransform {
    rotation_deg: f64,
    pivot_x: f64,
    pivot_y: f64,
    translate_x: f64,
    translate_y: f64,
}

impl LayerTransform {
    const IDENTITY: Self = Self {
        rotation_deg: 0.0,
        pivot_x: 0.0,
        pivot_y: 0.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
}

fn compose(key: SpriteKey) -> Vec<u32> {
    let (width, height) = sprite_px(key.dpr_q);
    let mut out = vec![0; (width * height) as usize];
    let mut layer = |name: &str, transform: LayerTransform| {
        let region = atlas::region(name).unwrap_or_else(|| panic!("missing atlas region {name}"));
        composite_layer(&mut out, width, height, key.coat, region, transform);
    };

    layer("shadow", LayerTransform::IDENTITY);
    if key.species == Species::Cat {
        layer(
            "tail-cat",
            LayerTransform {
                rotation_deg: key.tail_q as f64,
                pivot_x: 22.0,
                pivot_y: 66.0,
                ..LayerTransform::IDENTITY
            },
        );
    }
    layer("body-shell", LayerTransform::IDENTITY);
    if let Some(pattern) = pattern_class(key.coat) {
        layer(pattern, LayerTransform::IDENTITY);
    }
    layer("belly", LayerTransform::IDENTITY);
    if key.species == Species::Cat {
        layer("cat-ears", LayerTransform::IDENTITY);
    }
    layer("muzzle", LayerTransform::IDENTITY);
    layer(eye_class(key.eyes), LayerTransform::IDENTITY);
    if key.species == Species::Cat {
        layer("cat-nose", LayerTransform::IDENTITY);
    }
    layer(mouth_class(key.mouth), LayerTransform::IDENTITY);
    layer("blush", LayerTransform::IDENTITY);
    if key.species == Species::Cat {
        layer("whiskers", LayerTransform::IDENTITY);
    }
    layer(
        "leg-fl",
        LayerTransform {
            translate_y: key.leg_fl_q as f64,
            ..LayerTransform::IDENTITY
        },
    );
    layer(
        "leg-fr",
        LayerTransform {
            translate_y: key.leg_fr_q as f64,
            ..LayerTransform::IDENTITY
        },
    );
    match key.species {
        Species::Cat => {}
        Species::Pig => {
            layer("tail-pig", LayerTransform::IDENTITY);
            layer("species-pig", LayerTransform::IDENTITY);
        }
        Species::Bear => {
            layer("tail-bear", LayerTransform::IDENTITY);
            layer("species-bear", LayerTransform::IDENTITY);
        }
    }
    out
}

fn composite_layer(
    out: &mut [u32],
    width: u32,
    height: u32,
    coat: CoatColor,
    region: &AtlasRegion,
    transform: LayerTransform,
) {
    let scale = width as f64 / SPRITE_W as f64;
    let source_left = region.source_x as f64 / atlas::ATLAS_SCALE;
    let source_top = region.source_y as f64 / atlas::ATLAS_SCALE;
    let source_right = (region.source_x + region.width) as f64 / atlas::ATLAS_SCALE;
    let source_bottom = (region.source_y + region.height) as f64 / atlas::ATLAS_SCALE;
    let angle = transform.rotation_deg.to_radians();
    let (sin, cos) = angle.sin_cos();

    let transform_point = |x: f64, y: f64| {
        let dx = x - transform.pivot_x;
        let dy = y - transform.pivot_y;
        (
            transform.pivot_x + dx * cos - dy * sin + transform.translate_x,
            transform.pivot_y + dx * sin + dy * cos + transform.translate_y,
        )
    };
    let corners = [
        transform_point(source_left, source_top),
        transform_point(source_right, source_top),
        transform_point(source_right, source_bottom),
        transform_point(source_left, source_bottom),
    ];
    let min_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let x0 = (min_x * scale).floor().max(0.0) as i32;
    let x1 = (max_x * scale).ceil().min(width as f64) as i32;
    let y0 = (min_y * scale).floor().max(0.0) as i32;
    let y1 = (max_y * scale).ceil().min(height as f64) as i32;

    for y in y0..y1 {
        for x in x0..x1 {
            let transformed_x = (x as f64 + 0.5) / scale - transform.translate_x;
            let transformed_y = (y as f64 + 0.5) / scale - transform.translate_y;
            let dx = transformed_x - transform.pivot_x;
            let dy = transformed_y - transform.pivot_y;
            let source_x = transform.pivot_x + dx * cos + dy * sin;
            let source_y = transform.pivot_y - dx * sin + dy * cos;
            let (role, coverage) = atlas::sample(region, source_x, source_y);
            if coverage == 0 {
                continue;
            }
            let [r, g, b, palette_alpha] = atlas::role_color(coat, role);
            let alpha = (coverage as u32 * palette_alpha as u32 / 255) as u8;
            if alpha == 0 {
                continue;
            }
            let index = (y as u32 * width + x as u32) as usize;
            out[index] = blend_straight(out[index], r, g, b, alpha);
        }
    }
}

fn blend_straight(dst: u32, sr: u8, sg: u8, sb: u8, sa: u8) -> u32 {
    if sa == 255 {
        return crate::render::pack(sa, sr, sg, sb);
    }
    let da = ((dst >> 24) & 0xff) as u32;
    let dr = ((dst >> 16) & 0xff) as u32;
    let dg = ((dst >> 8) & 0xff) as u32;
    let db = (dst & 0xff) as u32;
    let sa = sa as u32;
    let inv = 255 - sa;
    let out_a = sa + (da * inv + 127) / 255;
    if out_a == 0 {
        return 0;
    }
    let blend_channel = |source: u8, dest: u32| {
        let premul = source as u32 * sa + (dest * da * inv + 127) / 255;
        ((premul + out_a / 2) / out_a).min(255) as u8
    };
    crate::render::pack(
        out_a.min(255) as u8,
        blend_channel(sr, dr),
        blend_channel(sg, dg),
        blend_channel(sb, db),
    )
}

/// Blit cached sprite into a **physical** buffer (coords already × scale).
pub fn blit_sprite(
    buf: &mut [u32],
    win_w: u32,
    win_h: u32,
    sprite: &[u32],
    facing: f64,
    bob: f64,
    dest_cx: f64,
    dest_cy: f64,
    scale: f64,
) {
    let (sw, sh) = sprite_px(dpr_quant(scale));
    if sprite.len() < (sw * sh) as usize {
        return;
    }
    let flip = facing < 0.0;
    let dest_cy = dest_cy + bob;
    let left = (dest_cx - sw as f64 * 0.5).round() as i32;
    let top = (dest_cy - sh as f64 * 0.55).round() as i32;

    for sy in 0..sh as i32 {
        for sx in 0..sw as i32 {
            let src_x = if flip {
                (sw as i32 - 1 - sx) as u32
            } else {
                sx as u32
            };
            let c = sprite[(sy as u32 * sw + src_x) as usize];
            let a = ((c >> 24) & 0xFF) as u8;
            if a < 8 {
                continue;
            }
            let dx = left + sx;
            let dy = top + sy;
            if dx < 0 || dy < 0 || dx >= win_w as i32 || dy >= win_h as i32 {
                continue;
            }
            // alpha-over onto existing buffer pixel
            let i = (dy as u32 * win_w + dx as u32) as usize;
            let dst = buf[i];
            let da = ((dst >> 24) & 0xFF) as u8;
            if da < 8 {
                buf[i] = c;
                continue;
            }
            let sr = ((c >> 16) & 0xFF) as u8;
            let sg = ((c >> 8) & 0xFF) as u8;
            let sb = (c & 0xFF) as u8;
            let dr = ((dst >> 16) & 0xFF) as u8;
            let dg = ((dst >> 8) & 0xFF) as u8;
            let db = (dst & 0xFF) as u8;
            let k = a as f32 / 255.0;
            let nr = (sr as f32 * k + dr as f32 * (1.0 - k)) as u8;
            let ng = (sg as f32 * k + dg as f32 * (1.0 - k)) as u8;
            let nb = (sb as f32 * k + db as f32 * (1.0 - k)) as u8;
            let na = a.max(da);
            buf[i] = crate::render::pack(na, nr, ng, nb);
        }
    }
}

#[cfg(all(test, feature = "asset-compiler"))]
mod visual_parity_tests {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg::{Options, Tree};

    use super::*;

    fn css_color(coat: CoatColor, role: u8) -> String {
        let [r, g, b, a] = atlas::role_color(coat, role);
        if a == 0 {
            "transparent".to_owned()
        } else {
            format!("#{r:02X}{g:02X}{b:02X}")
        }
    }

    fn reference_svg(key: SpriteKey) -> String {
        let mut svg = include_str!("../assets/pet.svg").to_owned();
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
            svg = svg.replace(token, &css_color(key.coat, role));
        }
        let pattern = pattern_class(key.coat)
            .map(|class| format!(".{class}{{display:inline}}"))
            .unwrap_or_default();
        let (pig, bear, hide_cat) = match key.species {
            Species::Cat => ("none", "none", ""),
            Species::Pig => (
                "inline",
                "none",
                "#cat-ears,#cat-nose,#tail,#whisker-l,#whisker-r{display:none}",
            ),
            Species::Bear => (
                "none",
                "inline",
                "#cat-ears,#cat-nose,#tail,#whisker-l,#whisker-r{display:none}",
            ),
        };
        let style = format!(
            "<style>.species-pig{{display:{pig}}}.species-bear{{display:{bear}}}.pattern,.eye-style,.mouth-style{{display:none}}{pattern}.{}{{display:inline}}.{}{{display:inline}}{hide_cat}</style>",
            eye_class(key.eyes),
            mouth_class(key.mouth),
        );
        if let Some(open_end) = svg.find('>') {
            svg.insert_str(open_end + 1, &style);
        }
        svg = svg.replace(
            r#"<g id="tail" style="transform-origin: 22px 66px;">"#,
            &format!(
                r#"<g id="tail" transform="rotate({}, 22, 66)">"#,
                key.tail_q
            ),
        );
        svg = svg.replace(
            r#"<g id="leg-fl" style="transform-origin: 42px 95px;">"#,
            &format!(
                r#"<g id="leg-fl" transform="translate(0, {})">"#,
                key.leg_fl_q
            ),
        );
        svg = svg.replace(
            r#"<g id="leg-fr" style="transform-origin: 82px 95px;">"#,
            &format!(
                r#"<g id="leg-fr" transform="translate(0, {})">"#,
                key.leg_fr_q
            ),
        );
        svg
    }

    fn reference_pixels(key: SpriteKey) -> Vec<u32> {
        let tree = Tree::from_str(&reference_svg(key), &Options::default()).unwrap();
        let (width, height) = sprite_px(key.dpr_q);
        let mut pixmap = Pixmap::new(width, height).unwrap();
        let scale = dpr_of(key.dpr_q) as f32;
        resvg::render(
            &tree,
            Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        pixmap
            .pixels()
            .iter()
            .map(|pixel| {
                let a = pixel.alpha();
                let channel = |value: u8| {
                    if a == 0 || a == 255 {
                        value
                    } else {
                        ((value as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                    }
                };
                crate::render::pack(
                    a,
                    channel(pixel.red()),
                    channel(pixel.green()),
                    channel(pixel.blue()),
                )
            })
            .collect()
    }

    fn premul_channels(pixel: u32) -> [i32; 4] {
        let a = ((pixel >> 24) & 0xff) as i32;
        let premul = |shift| (((pixel >> shift) & 0xffu32) as i32 * a + 127) / 255;
        [a, premul(16), premul(8), premul(0)]
    }

    #[test]
    fn layered_atlas_stays_close_to_the_svg_reference() {
        let cases = [
            SpriteKey {
                species: Species::Cat,
                coat: CoatColor::Orange,
                eyes: EyeStyle::Normal,
                mouth: MouthStyle::Normal,
                tail_q: 16,
                leg_fl_q: -2,
                leg_fr_q: 0,
                dpr_q: 4,
            },
            SpriteKey {
                species: Species::Cat,
                coat: CoatColor::Calico,
                eyes: EyeStyle::Hearts,
                mouth: MouthStyle::Tongue,
                tail_q: -24,
                leg_fl_q: 2,
                leg_fr_q: -4,
                dpr_q: 8,
            },
            SpriteKey {
                species: Species::Pig,
                coat: CoatColor::Pink,
                eyes: EyeStyle::Wide,
                mouth: MouthStyle::Smile,
                tail_q: 0,
                leg_fl_q: 0,
                leg_fr_q: 2,
                dpr_q: 8,
            },
            SpriteKey {
                species: Species::Bear,
                coat: CoatColor::Black,
                eyes: EyeStyle::Stars,
                mouth: MouthStyle::Grumpy,
                tail_q: 0,
                leg_fl_q: 2,
                leg_fr_q: 2,
                dpr_q: 12,
            },
        ];
        for key in cases {
            let actual = compose(key);
            let reference = reference_pixels(key);
            let mut total_error = 0u64;
            let mut large_error = 0usize;
            for (actual, reference) in actual.iter().zip(&reference) {
                let actual = premul_channels(*actual);
                let reference = premul_channels(*reference);
                let error: i32 = actual
                    .iter()
                    .zip(reference)
                    .map(|(left, right)| (left - right).abs())
                    .sum();
                total_error += error as u64;
                large_error += usize::from(error > 96);
            }
            let mean = total_error as f64 / (actual.len() * 4) as f64;
            let large_ratio = large_error as f64 / actual.len() as f64;
            assert!(
                mean <= 6.0 && large_ratio <= 0.035,
                "{key:?}: mean premul error {mean:.3}, large-pixel ratio {large_ratio:.3}"
            );
        }
    }
}
