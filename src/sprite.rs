//! Rasterize the original WebView `CAT_SVG` via resvg for softbuffer blit.
//!
//! Appearance (coat / species / face) and quantized leg/tail poses are cached
//! with a hard LRU cap — unbounded pose keys were leaking RAM until the
//! process got killed (tray disappears with it).

use std::collections::{HashMap, VecDeque};
use std::f64::consts::TAU;

/// Max cached rasters (~53KB each at 120×110). Walking creates many pose keys.
const MAX_CACHE: usize = 48;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

use crate::pet::{CoatColor, IdleAction, Mode, Pet, Species, TrickAction};

const PET_SVG: &str = include_str!("../assets/pet.svg");

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

    pub fn pixels_for(&mut self, pet: &Pet) -> &[u32] {
        let key = key_for(pet);
        if self.cache.contains_key(&key) {
            if let Some(i) = self.order.iter().position(|k| *k == key) {
                let k = self.order.remove(i).expect("index from position");
                self.order.push_back(k);
            }
        } else {
            while self.cache.len() >= MAX_CACHE {
                if let Some(old) = self.order.pop_front() {
                    self.cache.remove(&old);
                } else {
                    break;
                }
            }
            let px = rasterize(key).unwrap_or_else(|| vec![0; (SPRITE_W * SPRITE_H) as usize]);
            self.cache.insert(key, px);
            self.order.push_back(key);
        }
        self.cache.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

fn key_for(pet: &Pet) -> SpriteKey {
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
    }
}

fn quantize(v: f64, step: f64, lo: f64, hi: f64) -> i8 {
    let clamped = v.clamp(lo, hi);
    let q = (clamped / step).round() * step;
    q.round() as i8
}

fn pose_for(pet: &Pet) -> AnimPose {
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

fn idle_pose(pet: &Pet) -> AnimPose {
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

fn eyes_for(pet: &Pet) -> EyeStyle {
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

fn mouth_for(pet: &Pet) -> MouthStyle {
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

struct Palette {
    body: &'static str,
    body_dark: &'static str,
    belly: &'static str,
    inner_ear: &'static str,
    nose: &'static str,
    eye: &'static str,
    whisker: &'static str,
    blush: &'static str,
    accent: &'static str,
    snout: &'static str,
}

fn palette(coat: CoatColor) -> Palette {
    // Matches src/styles.css coat variables.
    match coat {
        CoatColor::Orange => Palette {
            body: "#F4A56B",
            body_dark: "#C66A2C",
            belly: "#FFF1DC",
            inner_ear: "#FFB8C1",
            nose: "#E36B7A",
            eye: "#2D1A0A",
            whisker: "#5A3A24",
            blush: "#FFB3BA",
            accent: "#C66A2C",
            snout: "#E89DAE",
        },
        CoatColor::Calico => Palette {
            body: "#FFF1DC",
            body_dark: "#2C2828",
            belly: "#FFFFFF",
            inner_ear: "#FFB8C1",
            nose: "#E36B7A",
            eye: "#2D1A0A",
            whisker: "#6A5A4A",
            blush: "#FFB3BA",
            accent: "#F4A56B",
            snout: "#E89DAE",
        },
        CoatColor::Cow => Palette {
            body: "#FFFFFF",
            body_dark: "#2C2828",
            belly: "#FFFFFF",
            inner_ear: "#FFB8C1",
            nose: "#FF85A1",
            eye: "#2D1A0A",
            whisker: "#6A5A4A",
            blush: "#FFB3BA",
            accent: "#2C2828",
            snout: "#E89DAE",
        },
        CoatColor::Tabby => Palette {
            body: "#A89E91",
            body_dark: "#5C544A",
            belly: "#DAD2C6",
            inner_ear: "#E8A8B0",
            nose: "#3C3530",
            eye: "#1A1A1A",
            whisker: "#FFFFFF",
            blush: "#FFB3BA",
            accent: "#5C544A",
            snout: "#E89DAE",
        },
        CoatColor::Tuxedo => Palette {
            body: "#2A2828",
            body_dark: "#000000",
            belly: "#FFFFFF",
            inner_ear: "#FFB8C1",
            nose: "#FF85A1",
            eye: "#FFD23B",
            whisker: "#FFFFFF",
            blush: "#FFB3BA",
            accent: "#FFFFFF",
            snout: "#E89DAE",
        },
        CoatColor::Pink => Palette {
            body: "#F5B5C0",
            body_dark: "#C2778A",
            belly: "#FFE6EE",
            inner_ear: "#E8929E",
            nose: "#C2546F",
            eye: "#2D1A0A",
            whisker: "transparent",
            blush: "#FFB3BA",
            accent: "#C2778A",
            snout: "#E89DAE",
        },
        CoatColor::Cream => Palette {
            body: "#FFE0C2",
            body_dark: "#D8B591",
            belly: "#FFF4E0",
            inner_ear: "#E8B998",
            nose: "#B27858",
            eye: "#2D1A0A",
            whisker: "transparent",
            blush: "#FFB3BA",
            accent: "#D8B591",
            snout: "#E8C6A6",
        },
        CoatColor::Brown => Palette {
            body: "#A87248",
            body_dark: "#5E3D1F",
            belly: "#D2A878",
            inner_ear: "#6A4525",
            nose: "#1A0F0F",
            eye: "#2D1A0A",
            whisker: "transparent",
            blush: "#FFB3BA",
            accent: "#5E3D1F",
            snout: "#E89DAE",
        },
        CoatColor::Black => Palette {
            body: "#3C2D2D",
            body_dark: "#1A0F0F",
            belly: "#5E4C4C",
            inner_ear: "#2A1818",
            nose: "#000000",
            eye: "#FFD23B",
            whisker: "transparent",
            blush: "#FFB3BA",
            accent: "#1A0F0F",
            snout: "#E89DAE",
        },
        CoatColor::Polar => Palette {
            body: "#F2EDE2",
            body_dark: "#C8C0B0",
            belly: "#FFFFFF",
            inner_ear: "#D8D0C0",
            nose: "#1A1A1A",
            eye: "#2D1A0A",
            whisker: "transparent",
            blush: "#FFB3BA",
            accent: "#C8C0B0",
            snout: "#E89DAE",
        },
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

fn build_svg(key: SpriteKey) -> String {
    let p = palette(key.coat);
    let mut svg = PET_SVG.to_string();
    // resvg doesn't resolve CSS variables — substitute concrete colors.
    for (name, val) in [
        ("var(--body-dark)", p.body_dark),
        ("var(--body)", p.body),
        ("var(--belly)", p.belly),
        ("var(--inner-ear)", p.inner_ear),
        ("var(--nose)", p.nose),
        ("var(--eye)", p.eye),
        ("var(--whisker)", p.whisker),
        ("var(--blush)", p.blush),
        ("var(--accent, #F4A56B)", p.accent),
        ("var(--accent)", p.accent),
        ("var(--snout, #E89DAE)", p.snout),
        ("var(--snout)", p.snout),
    ] {
        svg = svg.replace(name, val);
    }

    let pattern = pattern_class(key.coat)
        .map(|c| format!(".{c} {{ display: inline; }}"))
        .unwrap_or_default();

    let (show_pig, show_bear, hide_cat_bits) = match key.species {
        Species::Cat => ("none", "none", ""),
        Species::Pig => (
            "inline",
            "none",
            "#cat-ears, #cat-nose, #tail, #whisker-l, #whisker-r { display: none; }",
        ),
        Species::Bear => (
            "none",
            "inline",
            "#cat-ears, #cat-nose, #tail, #whisker-l, #whisker-r { display: none; }",
        ),
    };

    let style = format!(
        r#"
<style type="text/css">
  .species-pig {{ display: {show_pig}; }}
  .species-bear {{ display: {show_bear}; }}
  .pattern {{ display: none; }}
  {pattern}
  .eye-style, .mouth-style {{ display: none; }}
  .{eye} {{ display: inline; }}
  .{mouth} {{ display: inline; }}
  {hide_cat_bits}
</style>
"#,
        eye = eye_class(key.eyes),
        mouth = mouth_class(key.mouth),
    );

    if let Some(i) = svg.find('>') {
        svg.insert_str(i + 1, &style);
    }

    // Pose: SVG transform (usvg ignores CSS transform-origin on style=).
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

fn rasterize(key: SpriteKey) -> Option<Vec<u32>> {
    let svg = build_svg(key);
    let opt = Options::default();
    let tree = Tree::from_str(&svg, &opt).ok()?;
    let mut pixmap = Pixmap::new(SPRITE_W, SPRITE_H)?;
    // SVG viewBox is 120×110 — 1:1 with WebView cat size.
    let ts = Transform::from_scale(
        SPRITE_W as f32 / 120.0,
        SPRITE_H as f32 / 110.0,
    );
    resvg::render(&tree, ts, &mut pixmap.as_mut());

    let mut out = Vec::with_capacity((SPRITE_W * SPRITE_H) as usize);
    for px in pixmap.pixels() {
        // tiny-skia PremultipliedColorU8 → straight-ish ARGB for softbuffer
        let a = px.alpha();
        let r = px.red();
        let g = px.green();
        let b = px.blue();
        // un-premultiply if needed for clearer colors on transparent windows
        let (r, g, b) = if a > 0 && a < 255 {
            let fa = a as f32;
            (
                ((r as f32) * 255.0 / fa).min(255.0) as u8,
                ((g as f32) * 255.0 / fa).min(255.0) as u8,
                ((b as f32) * 255.0 / fa).min(255.0) as u8,
            )
        } else {
            (r, g, b)
        };
        out.push(crate::render::pack(a, r, g, b));
    }
    Some(out)
}

/// Blit cached sprite into the logical WIN buffer, with facing flip + bob.
pub fn blit_sprite(
    buf: &mut [u32],
    win_w: u32,
    win_h: u32,
    sprite: &[u32],
    facing: f64,
    bob: f64,
    dest_cx: f64,
    dest_cy: f64,
) {
    if sprite.len() < (SPRITE_W * SPRITE_H) as usize {
        return;
    }
    let flip = facing < 0.0;
    let dest_cy = dest_cy + bob;
    let left = (dest_cx - SPRITE_W as f64 * 0.5).round() as i32;
    let top = (dest_cy - SPRITE_H as f64 * 0.55).round() as i32;

    for sy in 0..SPRITE_H as i32 {
        for sx in 0..SPRITE_W as i32 {
            let src_x = if flip {
                (SPRITE_W as i32 - 1 - sx) as u32
            } else {
                sx as u32
            };
            let c = sprite[(sy as u32 * SPRITE_W + src_x) as usize];
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
