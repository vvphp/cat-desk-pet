//! Backend-neutral render input and the current native CPU renderer.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::pet::{
    CoatColor, Feed, Flyer, Gift, IdleAction, LaserTrailPt, Mode, Particle, Pet, Species,
    SpeechBubble, Toy, TrickAction,
};
use crate::render;
use crate::sprite::SpriteCache;

/// Shared logical hit padding for passthrough capture and click/context menu.
pub const PET_HIT_PAD: f64 = 4.0;
/// Leave-capture ellipse scale (enter uses 1.0) to avoid edge flicker.
pub const PET_HIT_LEAVE_SCALE: f64 = 1.4;

/// Immutable, render-only view of the behavior state.
///
/// Large variable-length collections are borrowed so creating a snapshot does
/// not allocate. A backend cannot mutate the behavior state through this type.
pub struct RenderSnapshot<'a> {
    pub mode: Mode,
    pub x: f64,
    pub y: f64,
    pub facing: f64,
    pub walk_phase: f64,
    pub idle_t: f64,
    pub sleep_t: f64,
    pub idle_action: IdleAction,
    pub idle_action_t: f64,
    pub dizzy_t: f64,
    pub eat_anim_t: f64,
    pub home_x: f64,
    pub home_y: f64,
    pub feed: Option<&'a Feed>,
    pub toy: Option<&'a Toy>,
    pub flyer: Option<&'a Flyer>,
    pub gift: Option<&'a Gift>,
    pub laser_trail: &'a std::collections::VecDeque<LaserTrailPt>,
    pub species: Species,
    pub coat: CoatColor,
    pub flash: f64,
    pub clingy_arrived: bool,
    pub bubble: Option<&'a SpeechBubble>,
    pub particles: &'a [Particle],
    pub trick_action: Option<TrickAction>,
}

impl<'a> From<&'a Pet> for RenderSnapshot<'a> {
    fn from(pet: &'a Pet) -> Self {
        Self {
            mode: pet.mode,
            x: pet.x,
            y: pet.y,
            facing: pet.facing,
            walk_phase: pet.walk_phase,
            idle_t: pet.idle_t,
            sleep_t: pet.sleep_t,
            idle_action: pet.idle_action,
            idle_action_t: pet.idle_action_t,
            dizzy_t: pet.dizzy_t,
            eat_anim_t: pet.eat_anim_t,
            home_x: pet.home_x,
            home_y: pet.home_y,
            feed: pet.feed.as_ref(),
            toy: pet.toy.as_ref(),
            flyer: pet.flyer.as_ref(),
            gift: pet.gift.as_ref(),
            laser_trail: &pet.laser_trail,
            species: pet.species,
            coat: pet.coat,
            flash: pet.flash,
            clingy_arrived: pet.clingy_arrived,
            bubble: pet.bubble.as_ref(),
            particles: &pet.particles,
            trick_action: pet.trick_action,
        }
    }
}

impl RenderSnapshot<'_> {
    pub fn pet_hit_mask(&self, scale: f64) -> HitMask {
        HitMask {
            center_x: self.x,
            center_y: self.y,
            radius_x: (39.0 + PET_HIT_PAD) * scale,
            radius_y: (33.0 + PET_HIT_PAD) * scale,
        }
    }
}

/// Logical desktop-space pet-body hit mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitMask {
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
}

impl HitMask {
    pub fn contains_desktop(&self, x: f64, y: f64) -> bool {
        let dx = x - self.center_x;
        let dy = y - self.center_y;
        (dx * dx) / (self.radius_x * self.radius_x) + (dy * dy) / (self.radius_y * self.radius_y)
            <= 1.0
    }

    pub fn contains_local(&self, x: f64, y: f64, origin_x: f64, origin_y: f64) -> bool {
        self.contains_desktop(origin_x + x, origin_y + y)
    }

    pub fn contains_physical(
        &self,
        x: f64,
        y: f64,
        origin_x: f64,
        origin_y: f64,
        dpr: f64,
    ) -> bool {
        let dpr = dpr.max(0.01);
        self.contains_local(x / dpr, y / dpr, origin_x, origin_y)
    }
}

/// Physical target and logical window origin for one render operation.
#[derive(Clone, Copy, Debug)]
pub struct FrameViewport {
    pub width: u32,
    pub height: u32,
    pub origin_x: f64,
    pub origin_y: f64,
    pub scale: f64,
}

impl FrameViewport {
    fn pixel_len(self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }
}

/// 128-bit semantic key for all state that can affect rendered pixels.
///
/// Two independently domain-separated standard hashes make accidental dirty
/// frame collisions negligible without retaining a second full framebuffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameKey {
    lo: u64,
    hi: u64,
}

impl FrameKey {
    pub fn new(snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> Self {
        Self {
            lo: hash_frame(snapshot, viewport, 0x43),
            hi: hash_frame(snapshot, viewport, 0xA7),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOutcome {
    pub frame_key: FrameKey,
    /// False means the previous pixel buffer already represents `frame_key`.
    pub rasterized: bool,
}

/// Minimal backend boundary. Platform presentation remains outside the backend
/// until the experimental `wgpu` surface path is introduced.
pub trait Renderer {
    fn render(&mut self, snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> RenderOutcome;

    fn pixels(&self) -> &[u32];

    fn invalidate(&mut self);
}

/// Existing SVG/procedural CPU raster path, now behind `Renderer`.
pub struct NativeRenderer {
    sprites: SpriteCache,
    pixels: Vec<u32>,
    last_frame_key: Option<FrameKey>,
}

impl NativeRenderer {
    pub fn new() -> Self {
        Self {
            sprites: SpriteCache::new(),
            pixels: Vec::new(),
            last_frame_key: None,
        }
    }
}

impl Renderer for NativeRenderer {
    fn render(&mut self, snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> RenderOutcome {
        let frame_key = FrameKey::new(snapshot, viewport);
        let pixel_len = viewport.pixel_len();
        if self.last_frame_key == Some(frame_key) && self.pixels.len() == pixel_len {
            return RenderOutcome {
                frame_key,
                rasterized: false,
            };
        }

        if self.pixels.len() != pixel_len {
            self.pixels.resize(pixel_len, 0);
        }
        render::draw_pet(
            &mut self.pixels,
            viewport.width,
            viewport.height,
            snapshot,
            viewport.origin_x,
            viewport.origin_y,
            &mut self.sprites,
            viewport.scale,
        );
        self.last_frame_key = Some(frame_key);
        RenderOutcome {
            frame_key,
            rasterized: true,
        }
    }

    fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    fn invalidate(&mut self) {
        self.last_frame_key = None;
    }
}

impl Default for NativeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_f64(value: f64, state: &mut impl Hasher) {
    value.to_bits().hash(state);
}

fn hash_frame(snapshot: &RenderSnapshot<'_>, viewport: FrameViewport, domain: u8) -> u64 {
    let mut state = DefaultHasher::new();
    domain.hash(&mut state);
    viewport.width.hash(&mut state);
    viewport.height.hash(&mut state);
    hash_f64(viewport.origin_x, &mut state);
    hash_f64(viewport.origin_y, &mut state);
    hash_f64(viewport.scale, &mut state);

    (snapshot.mode as u8).hash(&mut state);
    hash_f64(snapshot.x, &mut state);
    hash_f64(snapshot.y, &mut state);
    hash_f64(snapshot.facing, &mut state);
    hash_f64(snapshot.walk_phase, &mut state);
    hash_f64(snapshot.idle_t, &mut state);
    hash_f64(snapshot.sleep_t, &mut state);
    (snapshot.idle_action as u8).hash(&mut state);
    hash_f64(snapshot.idle_action_t, &mut state);
    hash_f64(snapshot.dizzy_t, &mut state);
    hash_f64(snapshot.eat_anim_t, &mut state);
    hash_f64(snapshot.home_x, &mut state);
    hash_f64(snapshot.home_y, &mut state);
    snapshot.species.hash(&mut state);
    snapshot.coat.hash(&mut state);
    hash_f64(snapshot.flash, &mut state);
    snapshot.clingy_arrived.hash(&mut state);
    snapshot
        .trick_action
        .map(|value| value as u8)
        .hash(&mut state);

    snapshot.feed.is_some().hash(&mut state);
    if let Some(feed) = snapshot.feed {
        hash_f64(feed.x, &mut state);
        hash_f64(feed.y, &mut state);
        feed.eat_t.is_some().hash(&mut state);
    }

    snapshot.toy.is_some().hash(&mut state);
    if let Some(toy) = snapshot.toy {
        (toy.kind as u8).hash(&mut state);
        hash_f64(toy.x, &mut state);
        hash_f64(toy.y, &mut state);
        hash_f64(toy.age, &mut state);
        hash_f64(toy.spin, &mut state);
    }

    snapshot.flyer.is_some().hash(&mut state);
    if let Some(flyer) = snapshot.flyer {
        (flyer.kind as u8).hash(&mut state);
        hash_f64(flyer.x, &mut state);
        hash_f64(flyer.y, &mut state);
        hash_f64(flyer.vx, &mut state);
        hash_f64(flyer.age, &mut state);
    }

    snapshot.gift.is_some().hash(&mut state);
    if let Some(gift) = snapshot.gift {
        (gift.kind as u8).hash(&mut state);
        hash_f64(gift.x, &mut state);
        hash_f64(gift.y, &mut state);
        gift.dropped.hash(&mut state);
        hash_f64(gift.fade, &mut state);
    }

    snapshot.laser_trail.len().hash(&mut state);
    for point in snapshot.laser_trail {
        hash_f64(point.x, &mut state);
        hash_f64(point.y, &mut state);
    }

    snapshot.particles.len().hash(&mut state);
    for particle in snapshot.particles {
        (particle.kind as u8).hash(&mut state);
        hash_f64(particle.x, &mut state);
        hash_f64(particle.y, &mut state);
        hash_f64(particle.age, &mut state);
        hash_f64(particle.life, &mut state);
        particle.label.hash(&mut state);
    }

    snapshot.bubble.is_some().hash(&mut state);
    if let Some(bubble) = snapshot.bubble {
        bubble.text.hash(&mut state);
        hash_f64(bubble.age, &mut state);
        hash_f64(bubble.dur, &mut state);
    }

    state.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> FrameViewport {
        FrameViewport {
            width: 180,
            height: 180,
            origin_x: 410.0,
            origin_y: 270.0,
            scale: 1.0,
        }
    }

    #[test]
    fn frame_key_tracks_render_state_not_behavior_only_state() {
        let mut pet = Pet::new(1000.0, 700.0);
        let first = FrameKey::new(&RenderSnapshot::from(&pet), viewport());

        pet.ambient_t += 1.0;
        let behavior_only = FrameKey::new(&RenderSnapshot::from(&pet), viewport());
        assert_eq!(first, behavior_only);

        pet.x += 1.0;
        let moved = FrameKey::new(&RenderSnapshot::from(&pet), viewport());
        assert_ne!(first, moved);
    }

    #[test]
    fn native_renderer_skips_an_identical_frame_and_can_be_invalidated() {
        let pet = Pet::new(1000.0, 700.0);
        let snapshot = RenderSnapshot::from(&pet);
        let mut renderer = NativeRenderer::new();

        assert!(renderer.render(&snapshot, viewport()).rasterized);
        assert!(!renderer.render(&snapshot, viewport()).rasterized);
        renderer.invalidate();
        assert!(renderer.render(&snapshot, viewport()).rasterized);
    }

    #[test]
    fn hit_mask_maps_desktop_local_and_retina_coordinates_consistently() {
        let pet = Pet::new(1000.0, 700.0);
        let snapshot = RenderSnapshot::from(&pet);
        let mask = snapshot.pet_hit_mask(1.0);
        let origin_x = 410.0;
        let origin_y = 270.0;
        let local_x = pet.x - origin_x;
        let local_y = pet.y - origin_y;

        assert!(mask.contains_desktop(pet.x, pet.y));
        assert!(mask.contains_local(local_x, local_y, origin_x, origin_y));
        assert!(mask.contains_physical(local_x * 2.0, local_y * 2.0, origin_x, origin_y, 2.0,));
        assert!(!mask.contains_desktop(pet.x + 80.0, pet.y));
    }
}
