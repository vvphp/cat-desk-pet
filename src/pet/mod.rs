//! Pet behaviour state machine.
//! + cursor: interested / watching / chasing

use std::collections::VecDeque;

mod bubble;
mod mode;
mod particle;
mod rng;

pub use bubble::SpeechBubble;
pub use mode::{Mode, TrickAction};
pub use particle::{Particle, ParticleKind};


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiftKind {
    Leaf,
    Flower,
    Mouse,
    Candy,
}

impl GiftKind {
    fn pick() -> Self {
        const POOL: &[GiftKind] = &[
            GiftKind::Leaf,
            GiftKind::Flower,
            GiftKind::Mouse,
            GiftKind::Candy,
        ];
        POOL[(fastrand_u64() as usize) % POOL.len()]
    }
}

#[derive(Clone, Debug)]
pub struct Gift {
    pub kind: GiftKind,
    pub x: f64,
    pub y: f64,
    pub dropped: bool,
    /// Seconds since drop (proud wait + linger).
    pub drop_age: f64,
    /// 1.0 opaque → 0.0 faded out.
    pub fade: f64,
    /// True after pet walked away; gift lingers then fades.
    pub lingering: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlyerKind {
    Bird,
    Butterfly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlyerPhase {
    FlyBy,
    FlyIn,
    Landed,
    FlyOff,
}

#[derive(Clone, Debug)]
pub struct Flyer {
    pub kind: FlyerKind,
    pub phase: FlyerPhase,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub age: f64,
    pub land_t: f64,
    pub nose: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToyKind {
    Yarn,
    Ball,
    Paper,
    Mouse,
    Laser,
    Wand,
}

impl ToyKind {
    fn is_cursor_driven(self) -> bool {
        matches!(self, ToyKind::Laser | ToyKind::Wand)
    }

    fn hit_limit(self) -> u32 {
        match self {
            ToyKind::Yarn => 4,
            ToyKind::Ball => 5,
            ToyKind::Paper => 3,
            ToyKind::Mouse => 3,
            ToyKind::Laser | ToyKind::Wand => u32::MAX,
        }
    }
}

/// Aligned with WebView `state.species`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Species {
    Cat,
    Pig,
    Bear,
}

impl Species {
    pub fn all() -> &'static [Species] {
        &[Species::Cat, Species::Pig, Species::Bear]
    }

    pub fn label(self) -> &'static str {
        match self {
            Species::Cat => "🐈 猫",
            Species::Pig => "🐷 猪",
            Species::Bear => "🐻 熊",
        }
    }

    pub fn speed(self) -> f64 {
        match self {
            Species::Cat => 1.0,
            Species::Pig => 0.78,
            Species::Bear => 0.72,
        }
    }

    pub fn default_coat(self) -> CoatColor {
        match self {
            Species::Cat => CoatColor::Orange,
            Species::Pig => CoatColor::Pink,
            Species::Bear => CoatColor::Brown,
        }
    }

    #[allow(dead_code)]
    pub fn coats(self) -> &'static [CoatColor] {
        match self {
            Species::Cat => &[
                CoatColor::Orange,
                CoatColor::Calico,
                CoatColor::Cow,
                CoatColor::Tabby,
                CoatColor::Tuxedo,
            ],
            Species::Pig => &[CoatColor::Pink, CoatColor::Cream],
            Species::Bear => &[CoatColor::Brown, CoatColor::Black, CoatColor::Polar],
        }
    }
}

/// Coat / fur colors (aligned with WebView `data-color`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoatColor {
    // cat
    Orange,
    Calico,
    Cow,
    Tabby,
    Tuxedo,
    // pig
    Pink,
    Cream,
    // bear
    Brown,
    Black,
    Polar,
}

impl CoatColor {
    pub fn label(self) -> &'static str {
        match self {
            CoatColor::Orange => "🟠 橘猫",
            CoatColor::Calico => "🐱 三花",
            CoatColor::Cow => "🐮 奶牛",
            CoatColor::Tabby => "🩶 灰虎斑",
            CoatColor::Tuxedo => "🎩 黑白",
            CoatColor::Pink => "🌸 粉猪",
            CoatColor::Cream => "🥛 奶白猪",
            CoatColor::Brown => "🟫 棕熊",
            CoatColor::Black => "⬛ 黑熊",
            CoatColor::Polar => "⬜ 北极熊",
        }
    }

    pub fn all() -> &'static [CoatColor] {
        &[
            CoatColor::Orange,
            CoatColor::Calico,
            CoatColor::Cow,
            CoatColor::Tabby,
            CoatColor::Tuxedo,
            CoatColor::Pink,
            CoatColor::Cream,
            CoatColor::Brown,
            CoatColor::Black,
            CoatColor::Polar,
        ]
    }

    pub fn species(self) -> Species {
        match self {
            CoatColor::Orange
            | CoatColor::Calico
            | CoatColor::Cow
            | CoatColor::Tabby
            | CoatColor::Tuxedo => Species::Cat,
            CoatColor::Pink | CoatColor::Cream => Species::Pig,
            CoatColor::Brown | CoatColor::Black | CoatColor::Polar => Species::Bear,
        }
    }

    /// (fur, fur_dark, ink/eye, belly highlight, accent patch)
    pub fn palette(self) -> ((u8, u8, u8), (u8, u8, u8), (u8, u8, u8), (u8, u8, u8), (u8, u8, u8)) {
        match self {
            CoatColor::Orange => (
                (0xF4, 0xA5, 0x6B),
                (0xC6, 0x6A, 0x2C),
                (0x2D, 0x1A, 0x0A),
                (0xFF, 0xF1, 0xDC),
                (0xC6, 0x6A, 0x2C),
            ),
            CoatColor::Calico => (
                (0xFF, 0xF1, 0xDC),
                (0x2C, 0x28, 0x28),
                (0x2D, 0x1A, 0x0A),
                (0xFF, 0xFF, 0xFF),
                (0xF4, 0xA5, 0x6B),
            ),
            CoatColor::Cow => (
                (0xFF, 0xFF, 0xFF),
                (0x2C, 0x28, 0x28),
                (0x2D, 0x1A, 0x0A),
                (0xFF, 0xFF, 0xFF),
                (0x2C, 0x28, 0x28),
            ),
            CoatColor::Tabby => (
                (0xA8, 0x9E, 0x91),
                (0x5C, 0x54, 0x4A),
                (0x1A, 0x1A, 0x1A),
                (0xDA, 0xD2, 0xC6),
                (0x5C, 0x54, 0x4A),
            ),
            CoatColor::Tuxedo => (
                (0x2A, 0x28, 0x28),
                (0x00, 0x00, 0x00),
                (0xFF, 0xD2, 0x3B), // yellow eyes
                (0xFF, 0xFF, 0xFF),
                (0xFF, 0xFF, 0xFF),
            ),
            CoatColor::Pink => (
                (0xF7, 0xA8, 0xB8),
                (0xE0, 0x78, 0x90),
                (0x4A, 0x20, 0x28),
                (0xFF, 0xD8, 0xE0),
                (0xE0, 0x78, 0x90),
            ),
            CoatColor::Cream => (
                (0xF5, 0xE6, 0xC8),
                (0xD4, 0xB8, 0x8A),
                (0x5A, 0x3A, 0x28),
                (0xFF, 0xF8, 0xEE),
                (0xD4, 0xB8, 0x8A),
            ),
            CoatColor::Brown => (
                (0x8B, 0x5A, 0x2B),
                (0x5C, 0x3A, 0x1A),
                (0x1A, 0x10, 0x08),
                (0xC4, 0x8A, 0x5A),
                (0x5C, 0x3A, 0x1A),
            ),
            CoatColor::Black => (
                (0x2A, 0x28, 0x2C),
                (0x10, 0x10, 0x12),
                (0xFF, 0xD2, 0x3B),
                (0x4A, 0x48, 0x4C),
                (0x10, 0x10, 0x12),
            ),
            CoatColor::Polar => (
                (0xF2, 0xF4, 0xF8),
                (0xC8, 0xD0, 0xDC),
                (0x1A, 0x1A, 0x22),
                (0xFF, 0xFF, 0xFF),
                (0xC8, 0xD0, 0xDC),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Feed {
    pub x: f64,
    pub y: f64,
    pub eat_t: Option<f64>,
    pub age: f64,
}

#[derive(Clone, Debug)]
pub struct Toy {
    pub kind: ToyKind,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub hits: u32,
    pub age: f64,
    pub swat_t: f64,
    /// Wand feather sway angle (degrees), visual only.
    pub spin: f64,
    /// Fake-mouse wander target.
    pub rat_x: f64,
    pub rat_y: f64,
    pub rat_next: f64,
}

/// Laser trail sample in desktop logical coords.
#[derive(Clone, Copy, Debug)]
pub struct LaserTrailPt {
    pub x: f64,
    pub y: f64,
    pub t: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForceScene {
    Walking,
    Idle,
    Sleeping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleAction {
    Sit,
    Yawn,
    Stretch,
    Look,
    TailCurl,
    MudRoll,
    BackScratch,
}

impl IdleAction {
    pub fn duration(self) -> f64 {
        match self {
            IdleAction::Sit => 2.2,
            IdleAction::Yawn => 0.9,
            IdleAction::Stretch => 0.9,
            IdleAction::Look => 1.8,
            IdleAction::TailCurl => 2.4,
            IdleAction::MudRoll => 3.2,
            IdleAction::BackScratch => 3.2,
        }
    }

    fn pick(species: Species, near_edge: bool) -> Self {
        let mut pool: Vec<IdleAction> = vec![
            IdleAction::Sit,
            IdleAction::Sit,
            IdleAction::Yawn,
            IdleAction::Stretch,
            IdleAction::Look,
            IdleAction::TailCurl,
        ];
        if species == Species::Pig {
            pool.push(IdleAction::MudRoll);
            pool.push(IdleAction::MudRoll);
        }
        if species == Species::Bear && near_edge {
            pool.push(IdleAction::BackScratch);
            pool.push(IdleAction::BackScratch);
        }
        pool[(fastrand_u64() as usize) % pool.len()]
    }
}

struct CursorSample {
    x: f64,
    y: f64,
    t: f64, // seconds since pet clock
}

pub struct Pet {
    pub mode: Mode,
    pub x: f64,
    pub y: f64,
    pub facing: f64,
    pub walk_phase: f64,
    pub idle_t: f64,
    pub sleep_t: f64,
    pub idle_action: IdleAction,
    pub idle_action_t: f64,
    pub look_flipped: bool,
    pub dizzy_t: f64,
    pub interested_jitter: f64,
    pub chase_t: f64,
    pub eat_anim_t: f64,
    pub floor_y: f64,
    /// Corner "home" spot for 回窝 (bed waits here during `GoingHome`).
    pub home_x: f64,
    pub home_y: f64,
    /// Latest cursor (logical desktop), if known.
    pub cursor: Option<(f64, f64)>,
    pub cursor_move_amt: f64,
    pub feed: Option<Feed>,
    pub toy: Option<Toy>,
    pub flyer: Option<Flyer>,
    pub gift: Option<Gift>,
    pub laser_trail: VecDeque<LaserTrailPt>,
    pub species: Species,
    pub coat: CoatColor,
    pub photo_t: f64,
    pub flash: f64, // 0..1 white flash opacity
    pub ambient_t: f64,
    gift_cooldown: f64,
    mode_elapsed: f64,
    mode_until: f64,
    target_x: f64,
    target_y: f64,
    screen_w: f64,
    screen_h: f64,
    pub dragging: bool,
    drag_dist: f64,
    drag_last: Option<(f64, f64)>,
    pub force_scene: Option<ForceScene>,
    clock: f64,
    cursor_trail: VecDeque<CursorSample>,
    last_sig_move_t: f64,
    last_clingy_t: f64,
    pub clingy_arrived: bool,
    /// Speech bubble above the pet (WebView `#bubble`).
    pub bubble: Option<SpeechBubble>,
    pub particles: Vec<Particle>,
    /// Soft mood 0..100 (trick weights + meow flavor).
    pub mood: f64,
    pub trick_action: Option<TrickAction>,
    last_ambient_meow_t: f64,
    last_tap_t: f64,
    last_zzz_bubble_t: f64,
    last_footprint_t: f64,
    last_dream_t: f64,
    /// One-shot flags so eat/gift bubbles don't spam.
    ate_bubble_shown: bool,
    kiss_particle_shown: bool,
}

impl Pet {
    /// Footprint / clamp margin — matches WebView cat (~120 CSS px).
    pub const SIZE: f64 = 120.0;

    pub fn new(screen_w: f64, screen_h: f64) -> Self {
        let x = screen_w * 0.5;
        let floor_y = screen_h * 0.72;
        Self {
            mode: Mode::Walking,
            x,
            y: floor_y,
            facing: 1.0,
            walk_phase: 0.0,
            idle_t: 0.0,
            sleep_t: 0.0,
            idle_action: IdleAction::Sit,
            idle_action_t: 0.0,
            look_flipped: false,
            dizzy_t: 0.0,
            interested_jitter: 0.0,
            chase_t: 0.0,
            eat_anim_t: 0.0,
            floor_y,
            home_x: (screen_w - Self::SIZE - 80.0).max(40.0),
            home_y: floor_y,
            cursor: None,
            cursor_move_amt: 0.0,
            feed: None,
            toy: None,
            flyer: None,
            gift: None,
            laser_trail: VecDeque::with_capacity(16),
            species: Species::Cat,
            coat: CoatColor::Orange,
            photo_t: 0.0,
            flash: 0.0,
            ambient_t: 0.0,
            gift_cooldown: 40.0, // allow tray/manual soon; ambient waits longer
            mode_elapsed: 0.0,
            mode_until: 0.0,
            target_x: x + 180.0,
            target_y: floor_y,
            screen_w,
            screen_h,
            dragging: false,
            drag_dist: 0.0,
            drag_last: None,
            force_scene: None,
            clock: 0.0,
            cursor_trail: VecDeque::with_capacity(48),
            last_sig_move_t: 0.0,
            last_clingy_t: -120.0,
            clingy_arrived: false,
            bubble: None,
            particles: Vec::new(),
            mood: 55.0,
            trick_action: None,
            last_ambient_meow_t: 0.0,
            last_tap_t: -10.0,
            last_zzz_bubble_t: -10.0,
            last_footprint_t: -10.0,
            last_dream_t: -10.0,
            ate_bubble_shown: false,
            kiss_particle_shown: false,
        }
    }

    pub fn set_screen(&mut self, w: f64, h: f64) {
        self.screen_w = w.max(320.0);
        self.screen_h = h.max(240.0);
        self.floor_y = self.screen_h * 0.72;
        self.home_x = (self.screen_w - Self::SIZE - 80.0).max(40.0);
        self.home_y = self.floor_y;
        self.clamp_pos();
    }

    /// Desktop-logical axis-aligned bounds that must stay visible (pet + nearby props).
    ///
    /// Far flyers / laser trails are clipped instead of growing the OS window toward
    /// full-monitor size (that path allocated multi‑hundred MB Retina present buffers).
    pub fn visible_bounds(&self) -> (f64, f64, f64, f64) {
        // Match the idle pet window (~180²) footprint around the pet.
        const BASE: f64 = 180.0;
        /// Hard cap on either edge — keeps present buffers bounded on Retina.
        const MAX_EDGE: f64 = 480.0;
        /// Only expand for props within this distance of the pet center (+ pad).
        const NEAR: f64 = 200.0;

        let half = BASE * 0.5;
        let mut min_x = self.x - half;
        let mut max_x = self.x + half;
        // Extra headroom for speech bubbles above the pet.
        let mut min_y = self.y - half - 56.0;
        let mut max_y = self.y + half;
        let cx = self.x;
        let cy = self.y;

        let mut include = |x: f64, y: f64, pad: f64| {
            if (x - cx).abs() > NEAR + pad || (y - cy).abs() > NEAR + pad {
                return;
            }
            min_x = min_x.min(x - pad);
            max_x = max_x.max(x + pad);
            min_y = min_y.min(y - pad);
            max_y = max_y.max(y + pad);
        };

        if let Some(b) = &self.bubble {
            let tw = (b.text.chars().count() as f64 * 9.0).clamp(48.0, 160.0);
            include(self.x, self.y - 70.0, tw * 0.5 + 12.0);
        }
        for p in &self.particles {
            include(p.x, p.y, 20.0);
        }

        if let Some(t) = &self.toy {
            include(t.x, t.y, 48.0);
            if t.kind == ToyKind::Laser {
                for p in &self.laser_trail {
                    include(p.x, p.y, 24.0);
                }
            }
        }
        if let Some(f) = &self.flyer {
            include(f.x, f.y, 40.0);
        }
        if let Some(feed) = &self.feed {
            include(feed.x, feed.y, 36.0);
        }
        if let Some(g) = &self.gift {
            include(g.x, g.y, 36.0);
        }
        if matches!(self.mode, Mode::GoingHome | Mode::InBed) {
            include(self.home_x, self.home_y, 70.0);
        }

        // Cap oversized unions around the *union* center (not pet-only) so a
        // near prop on one side isn't cropped while empty space remains on the other.
        let clamp_edge = |min_v: f64, max_v: f64, limit: f64| -> (f64, f64) {
            let span = max_v - min_v;
            if span <= MAX_EDGE {
                return (min_v, max_v);
            }
            let mid = (min_v + max_v) * 0.5;
            let half = MAX_EDGE * 0.5;
            let mut a = mid - half;
            let mut b = mid + half;
            if a < 0.0 {
                a = 0.0;
                b = MAX_EDGE.min(limit);
            } else if b > limit {
                b = limit;
                a = (b - MAX_EDGE).max(0.0);
            }
            (a, b)
        };
        (min_x, max_x) = clamp_edge(min_x, max_x, self.screen_w);
        (min_y, max_y) = clamp_edge(min_y, max_y, self.screen_h);

        // Clamp to screen so the OS window never exceeds the desktop.
        min_x = min_x.max(0.0);
        min_y = min_y.max(0.0);
        max_x = max_x.min(self.screen_w);
        max_y = max_y.min(self.screen_h);
        if max_x - min_x < BASE {
            max_x = (min_x + BASE).min(self.screen_w);
            min_x = (max_x - BASE).max(0.0);
        }
        if max_y - min_y < BASE {
            max_y = (min_y + BASE).min(self.screen_h);
            min_y = (max_y - BASE).max(0.0);
        }
        // Re-apply edge cap after BASE expand (narrow screens already smaller).
        (min_x, max_x) = clamp_edge(min_x, max_x, self.screen_w);
        (min_y, max_y) = clamp_edge(min_y, max_y, self.screen_h);
        (min_x, min_y, max_x, max_y)
    }

    pub fn note_cursor(&mut self, pos: Option<(f64, f64)>) {
        self.cursor = pos;
        let Some((x, y)) = pos else {
            self.prune_cursor_trail();
            return;
        };
        let moved = match self.cursor_trail.back() {
            Some(last) => (last.x - x).abs() >= 0.5 || (last.y - y).abs() >= 0.5,
            None => true,
        };
        if moved {
            self.cursor_trail.push_back(CursorSample {
                x,
                y,
                t: self.clock,
            });
        }
        // Always prune / recompute — stationary cursor must still decay amt
        // so Interested / Watching can end.
        self.prune_cursor_trail();
    }

    fn prune_cursor_trail(&mut self) {
        while let Some(front) = self.cursor_trail.front() {
            if self.clock - front.t > 1.5 {
                self.cursor_trail.pop_front();
            } else {
                break;
            }
        }
        let mut amt = 0.0;
        let mut prev: Option<&CursorSample> = None;
        for s in &self.cursor_trail {
            if let Some(p) = prev {
                amt += ((s.x - p.x).powi(2) + (s.y - p.y).powi(2)).sqrt();
            }
            prev = Some(s);
        }
        self.cursor_move_amt = amt;
        if amt > 100.0 {
            self.last_sig_move_t = self.clock;
        }
    }

    pub fn wake(&mut self) {
        if self.mode.is_asleep() {
            self.transition(Mode::Idle);
            // After idle-start bubble (if any) — waking message wins.
            self.show_bubble("睡饱啦~", 1.5);
        }
    }

    pub fn go_sleep(&mut self) {
        self.dragging = false;
        self.interrupt_gift();
        self.transition(Mode::Sleeping);
    }

    pub fn go_to_bed(&mut self) {
        self.dragging = false;
        self.interrupt_gift();
        self.toy = None;
        self.feed = None;
        self.flyer = None;
        self.force_scene = None;
        // Refresh home to current floor / screen corner (like WebView).
        self.home_x = (self.screen_w - Self::SIZE - 80.0).max(40.0);
        self.home_y = self.floor_y;
        let dx = self.home_x - self.x;
        let dy = self.home_y - self.y;
        if (dx * dx + dy * dy).sqrt() < 12.0 {
            self.x = self.home_x;
            self.y = self.home_y;
            self.transition(Mode::InBed);
        } else {
            self.transition(Mode::GoingHome);
            self.mode_until = 12.0;
        }
    }

    pub fn start_gifting(&mut self) {
        if self.gift.is_some() {
            return;
        }
        self.force_scene = None;
        if self.mode.is_asleep() {
            self.wake();
        }
        self.toy = None;
        self.feed = None;
        let dir = if self.facing >= 0.0 { 1.0 } else { -1.0 };
        let (gx, gy) = match self.cursor {
            Some((cx, cy)) => (cx, cy),
            None => (self.screen_w * 0.5, self.floor_y),
        };
        let jitter = (fastrand_u64() % 100) as f64 - 50.0;
        self.target_x = (gx + jitter).clamp(40.0, self.screen_w - 40.0);
        self.gift = Some(Gift {
            kind: GiftKind::pick(),
            x: self.x + dir * 22.0,
            y: self.y - 6.0,
            dropped: false,
            drop_age: 0.0,
            fade: 1.0,
            lingering: false,
        });
        self.gift_cooldown = 180.0 + (fastrand_u64() % 120) as f64;
        // Approach cursor Y band via target_y — do not teleport / permanently
        // rewrite floor_y (post-gift walking must keep a stable ground).
        self.target_y = (gy + 40.0).clamp(self.screen_h * 0.55, self.screen_h * 0.85);
        self.transition(Mode::Gifting);
        self.mode_until = 15.0;
    }

    fn interrupt_gift(&mut self) {
        let Some(g) = self.gift.as_mut() else {
            return;
        };
        if g.dropped {
            // Already on the ground — linger and fade.
            g.lingering = true;
            if g.drop_age < 0.01 {
                g.drop_age = 0.01;
            }
        } else {
            self.gift = None;
        }
    }

    pub fn spawn_feed(&mut self) {
        if self.feed.is_some() {
            return;
        }
        self.force_scene = None;
        self.interrupt_gift();
        if self.mode.is_asleep() {
            self.wake();
        }
        let dir = if self.facing >= 0.0 { 1.0 } else { -1.0 };
        let fx = (self.x + dir * 55.0).clamp(40.0, self.screen_w - 40.0);
        let fy = (self.y - 8.0).clamp(40.0, self.screen_h - 40.0);
        self.feed = Some(Feed {
            x: fx,
            y: fy,
            eat_t: None,
            age: 0.0,
        });
        self.toy = None;
        self.mode = Mode::Feeding;
        self.mode_elapsed = 0.0;
        self.mode_until = 12.0;
        self.eat_anim_t = 0.0;
    }

    pub fn spawn_toy(&mut self, kind: ToyKind) {
        self.force_scene = None;
        self.interrupt_gift();
        if self.mode.is_asleep() {
            self.wake();
        }
        let dir = if self.facing >= 0.0 { 1.0 } else { -1.0 };
        let (tx, ty) = if kind.is_cursor_driven() {
            self.cursor.unwrap_or((self.x + dir * 80.0, self.y - 30.0))
        } else {
            let tx = (self.x + dir * 70.0 + ((fastrand_u64() % 40) as f64 - 20.0))
                .clamp(40.0, self.screen_w - 40.0);
            let ty = (self.y + ((fastrand_u64() % 30) as f64 - 15.0))
                .clamp(40.0, self.screen_h - 40.0);
            (tx, ty)
        };
        self.laser_trail.clear();
        self.toy = Some(Toy {
            kind,
            x: tx,
            y: ty,
            vx: 0.0,
            vy: 0.0,
            hits: 0,
            age: 0.0,
            swat_t: 0.0,
            spin: 0.0,
            rat_x: tx,
            rat_y: ty,
            rat_next: 0.0,
        });
        self.feed = None;

        // Pigs/bears leave the wand out but don't chase it (WebView parity).
        if kind == ToyKind::Wand && self.species != Species::Cat {
            return;
        }

        self.mode = Mode::Playing;
        self.mode_elapsed = 0.0;
        self.mode_until = if kind.is_cursor_driven() { 18.0 } else { 20.0 };
    }

    pub fn cancel_toy(&mut self) {
        self.toy = None;
        self.laser_trail.clear();
        if self.mode == Mode::Playing {
            self.pick_new_target();
            self.transition(Mode::Walking);
        }
    }

    pub fn set_coat(&mut self, coat: CoatColor) {
        self.species = coat.species();
        self.coat = coat;
    }

    pub fn set_species(&mut self, species: Species) {
        if self.species == species {
            return;
        }
        self.species = species;
        self.coat = species.default_coat();
    }

    fn speed_mul(&self) -> f64 {
        self.species.speed()
    }

    pub fn spawn_bird_flyby(&mut self) {
        if self.flyer.is_some() {
            return;
        }
        self.force_scene = None;
        let go_right = fastrand_chance(0.5);
        // Pass near the cat so the small window can see it.
        let y = self.y - 40.0;
        let x = if go_right {
            self.x - 90.0
        } else {
            self.x + 90.0
        };
        let vx = if go_right { 160.0 } else { -160.0 };
        self.flyer = Some(Flyer {
            kind: FlyerKind::Bird,
            phase: FlyerPhase::FlyBy,
            x,
            y,
            vx,
            age: 0.0,
            land_t: 0.0,
            nose: false,
        });
        if matches!(self.mode, Mode::Walking | Mode::Idle) {
            self.mode = Mode::BirdWatch;
            self.mode_elapsed = 0.0;
            self.mode_until = 4.0;
        }
    }

    /// Deterministic bird + laser workload used by renderer A/B benchmarks.
    pub fn force_stress_scene(&mut self) {
        self.spawn_bird_flyby();
        self.spawn_toy(ToyKind::Laser);
        self.force_scene = Some(ForceScene::Walking);
        self.mode = Mode::Walking;
    }

    pub fn spawn_nose_butterfly(&mut self) {
        if self.flyer.is_some() {
            return;
        }
        self.force_scene = None;
        if self.mode.is_asleep() {
            self.wake();
        }
        let from_right = fastrand_chance(0.5);
        self.flyer = Some(Flyer {
            kind: FlyerKind::Butterfly,
            phase: FlyerPhase::FlyIn,
            x: if from_right {
                self.screen_w + 50.0
            } else {
                -50.0
            },
            y: self.screen_h * 0.35,
            vx: 0.0,
            age: 0.0,
            land_t: 0.0,
            nose: true,
        });
    }

    pub fn take_photo(&mut self) {
        self.force_scene = None;
        if self.mode.is_asleep() {
            self.wake();
        }
        self.feed = None;
        self.toy = None;
        // keep flyer if any — photo can interrupt
        self.mode = Mode::Photo;
        self.mode_elapsed = 0.0;
        self.mode_until = 1.2;
        self.photo_t = 0.0;
        self.flash = 1.0;
        self.show_bubble("茄子~ 📸", 1.3);
    }

    pub fn begin_drag(&mut self) {
        self.dragging = true;
        self.drag_dist = 0.0;
        self.drag_last = Some((self.x, self.y));
        self.feed = None;
        self.toy = None;
        self.interrupt_gift();
        if let Some(f) = self.flyer.as_mut() {
            if f.nose && f.phase == FlyerPhase::Landed {
                f.phase = FlyerPhase::FlyOff;
                f.vx = if fastrand_chance(0.5) { -220.0 } else { 220.0 };
            }
        }
        self.mode = Mode::Dragged;
        self.mode_elapsed = 0.0;
    }

    /// Soft press on the pet (before drag vs pet is decided).
    pub fn on_press(&mut self) {
        self.wake();
        self.force_scene = None;
    }

    pub fn show_bubble(&mut self, text: impl Into<String>, dur: f64) {
        self.bubble = Some(SpeechBubble {
            text: text.into(),
            age: 0.0,
            dur,
        });
    }

    pub fn clear_bubble(&mut self) {
        self.bubble = None;
    }

    fn spawn_particle(&mut self, kind: ParticleKind, x: f64, y: f64, life: f64) {
        self.particles.push(Particle::new(kind, x, y, life));
    }

    fn bump_mood(&mut self, delta: f64) {
        self.mood = (self.mood + delta).clamp(0.0, 100.0);
    }

    /// Short click / double-tap → trick (WebView mouseup path).
    pub fn on_short_click(&mut self) {
        if matches!(
            self.mode,
            Mode::GoingHome | Mode::Feeding | Mode::Dragged | Mode::Photo | Mode::Trick
        ) {
            return;
        }
        if self.clock - self.last_tap_t < 0.35 {
            self.last_tap_t = 0.0;
            self.start_trick(TrickAction::Kiss);
            return;
        }
        self.last_tap_t = self.clock;
        self.bump_mood(2.0);
        let action = self.pick_trick_action();
        self.start_trick(action);
    }

    fn pick_trick_action(&self) -> TrickAction {
        // Neutral mood weights (mood≈50): lean meow/heart like WebView mid mood.
        const WEIGHTS: &[(TrickAction, f64)] = &[
            (TrickAction::Meow, 0.20),
            (TrickAction::Heart, 0.16),
            (TrickAction::Spin, 0.12),
            (TrickAction::Pounce, 0.09),
            (TrickAction::HappyJump, 0.08),
            (TrickAction::Grumpy, 0.08),
            (TrickAction::Wave, 0.09),
            (TrickAction::Shy, 0.09),
            (TrickAction::SwatCursor, 0.09),
        ];
        let m = (self.mood / 100.0).clamp(0.0, 1.0);
        // Blend toward happier weights when mood high.
        let happy_boost = [0.0, 0.12, 0.04, 0.0, 0.08, -0.12, 0.04, -0.06, 0.02];
        let mut total = 0.0;
        let mut ws = [0.0; 9];
        for (i, (_, w0)) in WEIGHTS.iter().enumerate() {
            ws[i] = (w0 + happy_boost[i] * (m - 0.5) * 2.0).max(0.01);
            total += ws[i];
        }
        let mut r = (fastrand_u64() as f64 / u64::MAX as f64) * total;
        for (i, (action, _)) in WEIGHTS.iter().enumerate() {
            r -= ws[i];
            if r <= 0.0 {
                return *action;
            }
        }
        TrickAction::Meow
    }

    pub fn start_trick(&mut self, action: TrickAction) {
        if self.dragging {
            return;
        }
        self.force_scene = None;
        if self.mode.is_asleep() {
            self.wake();
        }
        self.kiss_particle_shown = false;
        self.trick_action = Some(action);
        self.mode = Mode::Trick;
        self.mode_elapsed = 0.0;
        self.mode_until = action.duration();
        let rng = fastrand_u64();
        match action {
            TrickAction::Meow => {
                let t = bubble::pick_meow(
                    self.species,
                    self.mood,
                    false,
                    self.cursor_move_amt,
                    self.clock - self.last_sig_move_t,
                    rng,
                );
                self.show_bubble(t, 1.3);
            }
            TrickAction::Heart => {
                self.show_bubble(bubble::pick_hearts(rng), 1.5);
                self.spawn_particle(ParticleKind::Heart, self.x, self.y - 30.0, 1.6);
            }
            TrickAction::Grumpy => self.show_bubble(bubble::pick_grumpy_line(rng), 1.2),
            TrickAction::Wave => self.show_bubble(bubble::pick_wave(rng), 1.3),
            TrickAction::Shy => self.show_bubble(bubble::pick_shy(rng), 1.4),
            TrickAction::Kiss => self.show_bubble(bubble::pick_kiss(rng), 1.2),
            TrickAction::Spin | TrickAction::Pounce | TrickAction::HappyJump => {}
            TrickAction::SwatCursor => {
                self.show_bubble("!", 0.7);
            }
        }
    }

    /// Enter petting after a long press without dragging (~0.5s).
    pub fn start_pet(&mut self) {
        if self.dragging || self.mode == Mode::Pet {
            return;
        }
        self.feed = None;
        self.toy = None;
        // Keep a dropped gift on the ground; interrupt only if still carrying.
        if self.gift.as_ref().map(|g| !g.dropped).unwrap_or(false) {
            self.interrupt_gift();
        }
        self.mode = Mode::Pet;
        self.mode_elapsed = 0.0;
        self.show_bubble("咕噜咕噜~", 10.0);
    }

    pub fn end_pet(&mut self) {
        if self.mode == Mode::Pet {
            self.clear_bubble();
            self.spawn_particle(ParticleKind::Heart, self.x + 8.0, self.y - 24.0, 1.5);
            self.transition(Mode::Idle);
        }
    }

    pub fn start_clingy(&mut self) {
        let Some((cx, cy)) = self.cursor else {
            return;
        };
        self.force_scene = None;
        if self.mode.is_asleep() {
            self.wake();
        }
        self.toy = None;
        self.feed = None;
        let jitter_x = (fastrand_u64() % 60) as f64 - 30.0;
        let jitter_y = 40.0 + (fastrand_u64() % 40) as f64;
        self.target_x = (cx + jitter_x).clamp(40.0, self.screen_w - 40.0);
        self.target_y = (cy + jitter_y).clamp(self.screen_h * 0.55, self.screen_h * 0.85);
        self.last_clingy_t = self.clock;
        self.transition(Mode::Clingy);
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_last = None;
        if self.drag_dist > 80.0 || self.mode_elapsed > 0.45 {
            self.mode = Mode::Dizzy;
            self.mode_elapsed = 0.0;
            self.dizzy_t = 0.0;
        } else {
            self.transition(Mode::Idle);
        }
    }

    pub fn drag_to(&mut self, x: f64, y: f64) {
        let nx = x.clamp(Self::SIZE * 0.5, self.screen_w - Self::SIZE * 0.5);
        let ny = y.clamp(Self::SIZE * 0.5, self.screen_h - Self::SIZE * 0.5);
        if let Some((lx, ly)) = self.drag_last {
            self.drag_dist += ((nx - lx).powi(2) + (ny - ly).powi(2)).sqrt();
        }
        self.drag_last = Some((nx, ny));
        self.x = nx;
        self.y = ny;
        self.floor_y = ny.clamp(self.screen_h * 0.55, self.screen_h * 0.85);
    }

    pub fn update(&mut self, dt: f64) {
        self.clock += dt;
        self.flash = (self.flash - dt * 2.2).max(0.0);
        // Mood drifts toward neutral 50.
        self.mood += (50.0 - self.mood) * dt * 0.003;

        if let Some(b) = &mut self.bubble {
            b.age += dt;
            if !b.alive() {
                self.bubble = None;
            }
        }
        particle::tick_particles(&mut self.particles, dt);

        if self.dragging {
            self.mode = Mode::Dragged;
            self.mode_elapsed += dt;
            self.phys_flyer(dt);
            return;
        }

        if let Some(scene) = self.force_scene {
            self.update_forced(scene, dt);
            self.phys_flyer(dt);
            self.phys_toy(dt);
            return;
        }

        self.maybe_ambient_event(dt);
        self.maybe_ambient_meow();
        self.maybe_clingy();
        self.maybe_react_to_cursor();
        self.phys_flyer(dt);
        self.phys_toy(dt);
        self.expire_orphan_cursor_toy();
        self.tick_gift_linger(dt);

        self.mode_elapsed += dt;
        match self.mode {
            Mode::Walking => self.tick_walking(dt),
            Mode::Idle => self.tick_idle(dt),
            Mode::Sleeping => self.tick_sleeping(dt),
            Mode::GoingHome => self.tick_going_home(dt),
            Mode::InBed => self.tick_in_bed(dt),
            Mode::Dragged => self.transition(Mode::Idle),
            Mode::Pet => {
                // Held; release from App ends it. Soft bob + tail phase while petted.
                self.walk_phase = (self.walk_phase + dt * 6.0) % std::f64::consts::TAU;
                self.y = self.floor_y + (self.mode_elapsed * 3.0).sin() * 1.2;
                if fastrand_chance(0.04) {
                    self.spawn_particle(
                        ParticleKind::Heart,
                        self.x + ((fastrand_u64() % 24) as f64 - 12.0),
                        self.y - 28.0,
                        1.5,
                    );
                }
            }
            Mode::Clingy => self.tick_clingy(dt),
            Mode::Dizzy => {
                self.dizzy_t += dt;
                if self.mode_elapsed > 0.9 {
                    self.pick_new_target();
                    self.transition(Mode::Walking);
                }
            }
            Mode::Interested => self.tick_interested(dt),
            Mode::Watching => self.tick_watching(dt),
            Mode::Chasing => self.tick_chasing(dt),
            Mode::Feeding => self.tick_feeding(dt),
            Mode::Playing => self.tick_playing(dt),
            Mode::BirdWatch => self.tick_bird_watch(dt),
            Mode::ButterflyNose => self.tick_butterfly_nose(dt),
            Mode::Gifting => self.tick_gifting(dt),
            Mode::Trick => self.tick_trick(dt),
            Mode::Startled => {
                if self.mode_elapsed > 0.7 {
                    self.transition(Mode::Idle);
                }
            }
            Mode::Photo => {
                self.photo_t += dt;
                if self.mode_elapsed > self.mode_until {
                    self.transition(Mode::Idle);
                }
            }
        }
    }

    fn maybe_ambient_meow(&mut self) {
        if !matches!(self.mode, Mode::Walking | Mode::Idle) {
            return;
        }
        let gap = 35.0 + (fastrand_u64() % 35000) as f64 / 1000.0;
        if self.clock - self.last_ambient_meow_t < gap {
            return;
        }
        // Only when fairly calm / not mid-chase energy.
        if self.cursor_move_amt > 80.0 {
            return;
        }
        self.last_ambient_meow_t = self.clock;
        self.show_bubble(bubble::pick_curious(self.species, fastrand_u64()), 1.1);
    }

    fn tick_trick(&mut self, dt: f64) {
        let Some(action) = self.trick_action else {
            self.transition(Mode::Idle);
            return;
        };
        let t = (self.mode_elapsed / self.mode_until.max(0.01)).clamp(0.0, 1.0);
        match action {
            TrickAction::Spin => {
                self.facing = if ((self.mode_elapsed * 8.0) as i32) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                self.walk_phase = (self.walk_phase + dt * 20.0) % std::f64::consts::TAU;
                self.y = self.floor_y;
            }
            TrickAction::Pounce | TrickAction::HappyJump => {
                let hop = if t < 0.45 {
                    -(t / 0.45) * 22.0
                } else {
                    -22.0 * (1.0 - (t - 0.45) / 0.55)
                };
                self.y = self.floor_y + hop;
                self.walk_phase = (self.walk_phase + dt * 14.0) % std::f64::consts::TAU;
                if t > 0.5 && action == TrickAction::Pounce {
                    self.spawn_particle(ParticleKind::Dust, self.x, self.floor_y + 18.0, 0.55);
                }
            }
            TrickAction::Kiss => {
                self.y = self.floor_y;
                if !self.kiss_particle_shown && t > 0.35 {
                    self.kiss_particle_shown = true;
                    self.spawn_particle(ParticleKind::Kiss, self.x + self.facing * 10.0, self.y - 10.0, 1.4);
                }
            }
            TrickAction::SwatCursor => {
                self.y = self.floor_y - 8.0 * (1.0 - t);
                self.walk_phase = (self.walk_phase + dt * 16.0) % std::f64::consts::TAU;
            }
            _ => {
                self.y = self.floor_y;
                self.walk_phase = (self.walk_phase + dt * 8.0) % std::f64::consts::TAU;
            }
        }
        if self.mode_elapsed >= self.mode_until {
            self.trick_action = None;
            self.transition(Mode::Idle);
        }
    }

    fn maybe_clingy(&mut self) {
        if !matches!(self.mode, Mode::Walking | Mode::Idle) {
            return;
        }
        // WebView: ~90s idle cursor + 120s cooldown.
        if self.clock - self.last_sig_move_t < 90.0 {
            return;
        }
        if self.clock - self.last_clingy_t < 120.0 {
            return;
        }
        if self.cursor.is_none() {
            return;
        }
        self.start_clingy();
    }

    fn tick_clingy(&mut self, dt: f64) {
        if self.mode_elapsed > self.mode_until {
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        }
        // Path on floor_y (ground); bob is applied after — never write bob into floor.
        let dx = self.target_x - self.x;
        let dy = self.target_y - self.floor_y;
        let dist = (dx * dx + dy * dy).sqrt();
        // Walk for the first ~4s of the 9s window (mirror WebView modeUntil-5000).
        if dist > 8.0 && self.mode_elapsed < self.mode_until - 5.0 {
            let speed = 60.0 * self.species.speed();
            let step = (speed * dt).min(dist);
            self.x += dx / dist * step;
            self.floor_y = (self.floor_y + dy / dist * step)
                .clamp(self.screen_h * 0.55, self.screen_h * 0.85);
            if dx.abs() > 2.0 {
                self.facing = if dx > 0.0 { 1.0 } else { -1.0 };
            }
            self.walk_phase = (self.walk_phase + dt * 9.0) % std::f64::consts::TAU;
            self.y = self.floor_y + self.walk_phase.sin() * 2.0;
        } else {
            self.clingy_arrived = true;
            self.y = self.floor_y + (self.mode_elapsed * 1.8).sin() * 1.5;
        }
    }

    fn maybe_ambient_event(&mut self, dt: f64) {
        self.ambient_t += dt;
        self.gift_cooldown = (self.gift_cooldown - dt).max(0.0);
        if self.flyer.is_some() {
            return;
        }
        if !matches!(self.mode, Mode::Walking | Mode::Idle) {
            return;
        }
        // occasional flyby every ~25–45s of calm time
        if self.ambient_t > 28.0 && fastrand_chance(0.008) {
            self.ambient_t = 0.0;
            if fastrand_chance(0.4) {
                self.spawn_nose_butterfly();
            } else {
                self.spawn_bird_flyby();
            }
            return;
        }
        // Happy spontaneous gift: cursor alive, far enough, cooldown done.
        if self.gift.is_none()
            && self.gift_cooldown <= 0.0
            && fastrand_chance(0.004)
        {
            if let Some((cx, cy)) = self.cursor {
                let dx = cx - self.x;
                let dy = cy - self.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 150.0 {
                    self.start_gifting();
                }
            }
        }
    }

    fn maybe_react_to_cursor(&mut self) {
        if self.mode.cursor_locked() {
            return;
        }
        let Some((cx, cy)) = self.cursor else {
            return;
        };
        let dist = ((cx - self.x).powi(2) + (cy - self.y).powi(2)).sqrt();
        let active = self.cursor_move_amt > 60.0;
        let fast = self.cursor_move_amt > 220.0;

        // Cursor already on the body: stay put so hover/click doesn't chase us away.
        const ON_BODY: f64 = 64.0;

        match self.mode {
            Mode::Walking | Mode::Idle => {
                if active && dist > ON_BODY && dist < 280.0 {
                    self.transition(Mode::Interested);
                } else if fast && dist >= 280.0 && dist < 900.0 {
                    self.transition(Mode::Watching);
                }
            }
            Mode::Interested => {
                if dist < ON_BODY {
                    self.transition(Mode::Idle);
                } else if fast && dist < 320.0 && self.mode_elapsed > 0.4 {
                    // escalate: close + very active → chase
                    self.transition(Mode::Chasing);
                } else if !active || dist > 380.0 || self.mode_elapsed > self.mode_until {
                    self.pick_new_target();
                    self.transition(Mode::Walking);
                }
            }
            Mode::Watching => {
                if self.mode_elapsed > self.mode_until || (dist < 280.0 && active) {
                    // hand off — next tick may go interested
                    self.pick_new_target();
                    self.transition(Mode::Walking);
                }
            }
            Mode::Chasing => {
                if self.mode_elapsed > self.mode_until {
                    self.transition(Mode::Idle);
                }
            }
            _ => {}
        }
    }

    fn tick_interested(&mut self, dt: f64) {
        let Some((cx, cy)) = self.cursor else {
            self.transition(Mode::Walking);
            return;
        };
        let dist = ((cx - self.x).powi(2) + (cy - self.y).powi(2)).sqrt();
        // Pointer on us: freeze orbit so hover doesn't look like a shake.
        if dist < 64.0 {
            self.y = self.floor_y;
            if (cx - self.x).abs() > 18.0 {
                self.facing = (cx - self.x).signum();
            }
            return;
        }
        self.interested_jitter += dt * 1.5;
        self.walk_phase = (self.walk_phase + dt * 9.0) % (std::f64::consts::TAU);
        let desired = 110.0;
        let ox = self.interested_jitter.cos() * desired;
        let oy = (self.interested_jitter * 1.3).sin() * desired * 0.7;
        let tx = cx + ox;
        let ty = cy + oy;
        self.move_toward(tx, ty, 70.0 * self.speed_mul() * dt);
        // face cursor, not orbit tangent
        if (cx - self.x).abs() > 18.0 {
            self.facing = (cx - self.x).signum();
        }
    }

    fn tick_watching(&mut self, _dt: f64) {
        if let Some((cx, _)) = self.cursor {
            if (cx - self.x).abs() > 18.0 {
                self.facing = (cx - self.x).signum();
            }
        }
        self.y = self.floor_y;
    }

    fn tick_chasing(&mut self, dt: f64) {
        self.chase_t += dt;
        self.walk_phase = (self.walk_phase + dt * 14.0) % (std::f64::consts::TAU);
        let Some((cx, cy)) = self.cursor else {
            self.transition(Mode::Idle);
            return;
        };
        // sprint toward cursor with a little lead
        let lead = 40.0;
        let vx = cx - self.x;
        let vy = cy - self.y;
        let d = (vx * vx + vy * vy).sqrt().max(1.0);
        let tx = cx + (vx / d) * lead;
        let ty = cy + (vy / d) * lead * 0.4;
        self.move_toward(tx, ty, 160.0 * self.speed_mul() * dt);
        if (cx - self.x).abs() > 10.0 {
            self.facing = (cx - self.x).signum();
        }
        // catch: close enough → brief idle celebration then continue or stop
        if d < 48.0 && self.mode_elapsed > 0.6 {
            self.transition(Mode::Idle);
        }
    }

    fn tick_feeding(&mut self, dt: f64) {
        let Some(mut feed) = self.feed.take() else {
            self.transition(Mode::Walking);
            return;
        };
        feed.age += dt;
        if self.mode_elapsed > self.mode_until || feed.age > 25.0 {
            self.feed = None;
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        }

        let dx = feed.x - self.x;
        let dy = feed.y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 28.0 {
            if feed.eat_t.is_none() {
                feed.eat_t = Some(0.0);
                self.ate_bubble_shown = false;
            }
            if let Some(t) = feed.eat_t.as_mut() {
                *t += dt;
                self.eat_anim_t = *t;
                if *t > 0.35 && !self.ate_bubble_shown {
                    self.ate_bubble_shown = true;
                    self.show_bubble(bubble::eat_bubble(self.species), 1.5);
                    self.bump_mood(match self.species {
                        Species::Cat => 15.0,
                        Species::Pig => 20.0,
                        Species::Bear => 28.0,
                    });
                    for _ in 0..bubble::eat_heart_count(self.species) {
                        self.spawn_particle(
                            ParticleKind::Heart,
                            self.x + ((fastrand_u64() % 20) as f64 - 10.0),
                            self.y - 26.0,
                            1.6,
                        );
                    }
                }
                if *t > 1.4 {
                    self.feed = None;
                    self.show_bubble("嗝~", 0.9);
                    self.transition(Mode::Idle);
                    return;
                }
            }
            self.feed = Some(feed);
        } else {
            feed.eat_t = None;
            self.eat_anim_t = 0.0;
            self.ate_bubble_shown = false;
            self.move_toward(feed.x, feed.y, 120.0 * self.speed_mul() * dt);
            if dx.abs() > 8.0 {
                self.facing = dx.signum();
            }
            self.walk_phase = (self.walk_phase + dt * 12.0) % (std::f64::consts::TAU);
            self.feed = Some(feed);
        }
    }

    fn tick_playing(&mut self, dt: f64) {
        let Some(toy) = self.toy.as_ref() else {
            self.laser_trail.clear();
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        };
        let cursor_toy = toy.kind.is_cursor_driven();
        let max_age = if cursor_toy { 18.0 } else { 30.0 };
        let max_hits = toy.kind.hit_limit();
        if self.mode_elapsed > self.mode_until || toy.age > max_age || toy.hits >= max_hits {
            self.toy = None;
            self.laser_trail.clear();
            self.show_bubble(bubble::toy_done_bubble(fastrand_u64()), 1.2);
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        }

        let tx = toy.x;
        let ty = toy.y;
        let kind = toy.kind;
        let dx = tx - self.x;
        let dy = ty - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        if cursor_toy {
            let is_laser = kind == ToyKind::Laser;
            let close_r = if is_laser { 55.0 } else { 70.0 };
            let cooldown = if is_laser { 0.62 } else { 0.95 };
            if dist < close_r {
                if let Some(t) = self.toy.as_mut() {
                    if t.swat_t <= 0.0 {
                        t.swat_t = cooldown;
                        // Bat at air — no hit count / no impulse.
                        if is_laser || fastrand_chance(0.55) {
                            self.walk_phase = 0.0; // brief pounce bob
                            self.chase_t += 0.2;
                        }
                    } else {
                        t.swat_t = (t.swat_t - dt).max(0.0);
                    }
                }
                if dx.abs() > 4.0 {
                    self.facing = dx.signum();
                }
                // hop feel while swatting
                let hop = if self.toy.as_ref().map(|t| t.swat_t > cooldown * 0.55).unwrap_or(false)
                {
                    if is_laser { -10.0 } else { -16.0 }
                } else {
                    0.0
                };
                self.y = self.floor_y + hop;
                self.walk_phase = (self.walk_phase + dt * 16.0) % (std::f64::consts::TAU);
            } else {
                let sp = if is_laser { 200.0 } else { 155.0 };
                self.move_toward(tx, ty, sp * self.speed_mul() * dt);
                if dx.abs() > 8.0 {
                    self.facing = dx.signum();
                }
                self.walk_phase = (self.walk_phase + dt * 14.0) % (std::f64::consts::TAU);
                if let Some(t) = self.toy.as_mut() {
                    t.swat_t = (t.swat_t - dt).max(0.0);
                }
            }
            return;
        }

        if dist < 36.0 {
            // swat physical toys
            if let Some(t) = self.toy.as_mut() {
                if t.swat_t <= 0.0 {
                    t.swat_t = 0.35;
                    t.hits += 1;
                    match t.kind {
                        ToyKind::Yarn => {
                            let kick = 220.0;
                            let ang = (fastrand_u64() % 628) as f64 / 100.0;
                            t.vx = self.facing * kick * 0.85 + ang.cos() * kick * 0.25;
                            t.vy = -kick * 0.4 + ang.sin() * kick * 0.15;
                        }
                        ToyKind::Ball => {
                            let kick = 320.0;
                            let ang = (fastrand_u64() % 628) as f64 / 100.0;
                            t.vx = self.facing * kick * 0.85 + ang.cos() * kick * 0.25;
                            t.vy = -kick * 0.55 + ang.sin() * kick * 0.15;
                        }
                        ToyKind::Paper => {
                            let kick = 90.0;
                            t.vx = self.facing * kick * 0.5 + ((fastrand_u64() % 40) as f64 - 20.0);
                            t.vy = -kick * 0.35;
                            t.spin += 40.0;
                        }
                        ToyKind::Mouse => {
                            // dart away
                            let dir = self.facing;
                            t.rat_x = (t.x + dir * (120.0 + (fastrand_u64() % 100) as f64))
                                .clamp(40.0, self.screen_w - 40.0);
                            t.rat_y = (t.y + ((fastrand_u64() % 120) as f64 - 60.0))
                                .clamp(self.screen_h * 0.45, self.screen_h * 0.88);
                            t.rat_next = t.age + 0.5;
                        }
                        ToyKind::Laser | ToyKind::Wand => {}
                    }
                } else {
                    t.swat_t = (t.swat_t - dt).max(0.0);
                }
            }
            if dx.abs() > 4.0 {
                self.facing = dx.signum();
            }
        } else {
            self.move_toward(tx, ty, 140.0 * self.speed_mul() * dt);
            if dx.abs() > 8.0 {
                self.facing = dx.signum();
            }
            self.walk_phase = (self.walk_phase + dt * 13.0) % (std::f64::consts::TAU);
            if let Some(t) = self.toy.as_mut() {
                t.swat_t = (t.swat_t - dt).max(0.0);
            }
        }
    }

    fn expire_orphan_cursor_toy(&mut self) {
        let Some(t) = self.toy.as_ref() else {
            return;
        };
        if t.kind.is_cursor_driven() && t.age > 18.0 {
            self.toy = None;
            self.laser_trail.clear();
            if self.mode == Mode::Playing {
                self.pick_new_target();
                self.transition(Mode::Walking);
            }
        }
    }

    fn tick_gifting(&mut self, dt: f64) {
        let Some(mut g) = self.gift.take() else {
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        };

        if !g.dropped {
            let tx = self.target_x;
            let ty = self.target_y;
            let dx = tx - self.x;
            let dy = ty - self.floor_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 22.0 || self.mode_elapsed > self.mode_until {
                // Arrive / give up — put present down.
                g.dropped = true;
                g.drop_age = 0.0;
                let dir = if self.facing >= 0.0 { 1.0 } else { -1.0 };
                g.x = self.x + dir * 28.0;
                g.y = self.y + 18.0;
                self.mode_until = 2.5;
                self.mode_elapsed = 0.0;
                self.floor_y = self.target_y;
                self.y = self.floor_y;
                self.show_bubble("给你的~♡", 2.0);
                self.spawn_particle(ParticleKind::Heart, g.x, g.y - 20.0, 1.7);
            } else {
                self.move_toward(tx, ty, 55.0 * self.speed_mul() * dt);
                if dx.abs() > 8.0 {
                    self.facing = dx.signum();
                }
                self.walk_phase = (self.walk_phase + dt * 10.0) % (std::f64::consts::TAU);
                let dir = if self.facing >= 0.0 { 1.0 } else { -1.0 };
                g.x = self.x + dir * 20.0;
                g.y = self.floor_y - 4.0 + self.walk_phase.sin() * 2.0;
                self.y = self.floor_y + self.walk_phase.sin() * 2.0;
            }
            self.gift = Some(g);
            return;
        }

        // Proud wait beside the gift.
        self.y = self.floor_y + (self.mode_elapsed * 2.0).sin() * 1.2;
        if self.mode_elapsed > self.mode_until {
            g.lingering = true;
            g.drop_age = 0.0; // linger clock starts when pet walks away
            self.gift = Some(g);
            self.pick_new_target();
            self.transition(Mode::Walking);
            return;
        }
        self.gift = Some(g);
    }

    fn tick_gift_linger(&mut self, dt: f64) {
        let Some(g) = self.gift.as_mut() else {
            return;
        };
        if self.mode == Mode::Gifting && !g.lingering {
            return;
        }
        if !g.dropped {
            // Carried gift but mode left gifting somehow — drop cleanup.
            self.gift = None;
            return;
        }
        if !g.lingering && self.mode != Mode::Gifting {
            g.lingering = true;
        }
        if g.lingering {
            g.drop_age += dt;
            // Linger 6s opaque, then ~1s fade.
            if g.drop_age > 6.0 {
                g.fade = (1.0 - (g.drop_age - 6.0) / 1.0).clamp(0.0, 1.0);
            }
            if g.fade <= 0.02 {
                self.gift = None;
            }
        }
    }

    fn tick_bird_watch(&mut self, dt: f64) {
        self.y = self.floor_y;
        if let Some(f) = &self.flyer {
            if (f.x - self.x).abs() > 12.0 {
                self.facing = (f.x - self.x).signum();
            }
        }
        if self.mode_elapsed > self.mode_until || self.flyer.is_none() {
            self.pick_new_target();
            self.transition(Mode::Walking);
        }
        let _ = dt;
    }

    fn tick_butterfly_nose(&mut self, dt: f64) {
        self.y = self.floor_y;
        // freeze; sneeze scare-off after a beat
        if self.mode_elapsed > 1.6 {
            if let Some(f) = self.flyer.as_mut() {
                f.phase = FlyerPhase::FlyOff;
                f.vx = if fastrand_chance(0.5) { -240.0 } else { 240.0 };
            }
            self.mode = Mode::Startled;
            self.mode_elapsed = 0.0;
            self.show_bubble(bubble::pick(&["啊嚏!", "啾!", "atishoo!"], fastrand_u64()), 0.9);
        }
        let _ = dt;
    }

    fn phys_flyer(&mut self, dt: f64) {
        let Some(mut f) = self.flyer.take() else {
            return;
        };
        f.age += dt;
        let nose_x = self.x + self.facing * 22.0;
        let nose_y = self.y - 10.0;

        match f.phase {
            FlyerPhase::FlyBy => {
                f.x += f.vx * dt;
                f.y += (f.age * 6.0).sin() * 18.0 * dt;
                if f.x < -80.0 || f.x > self.screen_w + 80.0 || f.age > 16.0 {
                    // gone
                    if self.mode == Mode::BirdWatch {
                        self.pick_new_target();
                        self.transition(Mode::Walking);
                    }
                    return;
                }
            }
            FlyerPhase::FlyIn => {
                let dx = nose_x - f.x;
                let dy = nose_y - f.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d < 8.0 {
                    f.x = nose_x;
                    f.y = nose_y;
                    f.phase = FlyerPhase::Landed;
                    f.land_t = 0.0;
                    if matches!(
                        self.mode,
                        Mode::Walking
                            | Mode::Idle
                            | Mode::Interested
                            | Mode::Watching
                            | Mode::BirdWatch
                    ) {
                        self.mode = Mode::ButterflyNose;
                        self.mode_elapsed = 0.0;
                        self.mode_until = 1.8;
                    } else {
                        f.phase = FlyerPhase::FlyOff;
                        f.vx = if f.x < self.screen_w * 0.5 {
                            -220.0
                        } else {
                            220.0
                        };
                    }
                } else {
                    let sp = 160.0 * dt;
                    f.x += dx / d * sp;
                    f.y += dy / d * sp + (f.age * 10.0).sin() * 10.0 * dt;
                }
            }
            FlyerPhase::Landed => {
                f.land_t += dt;
                f.x = nose_x;
                f.y = nose_y + (f.land_t * 8.0).sin() * 1.5;
                if self.mode != Mode::ButterflyNose {
                    f.phase = FlyerPhase::FlyOff;
                    f.vx = if fastrand_chance(0.5) { -220.0 } else { 220.0 };
                }
            }
            FlyerPhase::FlyOff => {
                f.x += f.vx * dt;
                f.y -= 50.0 * dt;
                if f.x < -80.0 || f.x > self.screen_w + 80.0 || f.age > 20.0 {
                    return;
                }
            }
        }
        self.flyer = Some(f);
    }

    fn phys_toy(&mut self, dt: f64) {
        let Some(kind) = self.toy.as_ref().map(|t| t.kind) else {
            return;
        };

        if kind.is_cursor_driven() {
            let cursor = self.cursor;
            let clock = self.clock;
            let is_laser = kind == ToyKind::Laser;
            if let Some(t) = self.toy.as_mut() {
                t.age += dt;
                if let Some((cx, cy)) = cursor {
                    // WebView uses ms half-lives 30 (laser) / 80 (wand).
                    let half = if is_laser { 0.030 } else { 0.080 };
                    let k = 1.0 - 0.5_f64.powf(dt / half);
                    t.x += (cx - t.x) * k;
                    t.y += (cy - t.y) * k;
                }
                t.vx = 0.0;
                t.vy = 0.0;
                if !is_laser {
                    t.spin = (clock * (std::f64::consts::TAU / 0.38)).sin() * 22.0;
                }
            }
            if is_laser {
                if let Some(t) = self.toy.as_ref() {
                    let push = match self.laser_trail.back() {
                        Some(last) => {
                            let dx = t.x - last.x;
                            let dy = t.y - last.y;
                            (dx * dx + dy * dy).sqrt() > 2.0
                        }
                        None => true,
                    };
                    if push {
                        self.laser_trail.push_back(LaserTrailPt {
                            x: t.x,
                            y: t.y,
                            t: clock,
                        });
                    }
                }
                while self.laser_trail.len() > 12 {
                    self.laser_trail.pop_front();
                }
                while self
                    .laser_trail
                    .front()
                    .map(|p| clock - p.t > 0.22)
                    .unwrap_or(false)
                {
                    self.laser_trail.pop_front();
                }
            } else {
                self.laser_trail.clear();
            }
            return;
        }

        let Some(t) = self.toy.as_mut() else {
            return;
        };
        t.age += dt;

        if t.kind == ToyKind::Mouse {
            if t.age >= t.rat_next {
                t.rat_x = (t.x + ((fastrand_u64() % 440) as f64 - 220.0))
                    .clamp(40.0, self.screen_w - 40.0);
                t.rat_y = (t.y + ((fastrand_u64() % 240) as f64 - 120.0))
                    .clamp(self.screen_h * 0.45, self.screen_h * 0.88);
                t.rat_next = t.age + 0.9 + (fastrand_u64() % 800) as f64 / 1000.0;
            }
            let dx = t.rat_x - t.x;
            let dy = t.rat_y - t.y;
            let d = (dx * dx + dy * dy).sqrt();
            if d > 4.0 {
                let sp = 110.0 * dt;
                t.x += dx / d * sp;
                t.y += dy / d * sp;
                t.spin += 40.0 * dt;
            }
            t.vx = 0.0;
            t.vy = 0.0;
            return;
        }

        t.x += t.vx * dt;
        t.y += t.vy * dt;
        // friction + light gravity feel (paper drags more)
        let fric = if t.kind == ToyKind::Paper { 4.2 } else { 2.8 };
        let bounce = if t.kind == ToyKind::Paper {
            0.38
        } else if t.kind == ToyKind::Ball {
            0.85
        } else {
            0.7
        };
        t.vx *= (1.0 - fric * dt).max(0.0);
        t.vy *= (1.0 - 2.2 * dt).max(0.0);
        t.vy += 40.0 * dt;
        if t.kind == ToyKind::Paper || t.kind == ToyKind::Yarn {
            t.spin += t.vx * 0.08 * dt;
        }
        // bounce in play arena near floor band
        let min_x = 40.0;
        let max_x = self.screen_w - 40.0;
        let min_y = self.screen_h * 0.45;
        let max_y = self.screen_h * 0.88;
        if t.x < min_x {
            t.x = min_x;
            t.vx = t.vx.abs() * bounce;
        } else if t.x > max_x {
            t.x = max_x;
            t.vx = -t.vx.abs() * bounce;
        }
        if t.y < min_y {
            t.y = min_y;
            t.vy = t.vy.abs() * bounce * 0.7;
        } else if t.y > max_y {
            t.y = max_y;
            t.vy = -t.vy.abs() * bounce;
            t.vx *= 0.94;
        }
        if t.vx.abs() < 8.0 {
            t.vx = 0.0;
        }
        if t.vy.abs() < 8.0 {
            t.vy = 0.0;
        }
    }

    fn move_toward(&mut self, tx: f64, ty: f64, step: f64) {
        // Step ground along floor_y so walk bob never accumulates into the floor.
        let dx = tx - self.x;
        let dy = ty - self.floor_y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < 4.0 || step <= 0.0 {
            return;
        }
        let k = (step / d).min(1.0);
        self.x += dx * k;
        self.floor_y = (self.floor_y + dy * k).clamp(self.screen_h * 0.55, self.screen_h * 0.85);
        self.y = self.floor_y;
        let y_before = self.y;
        self.clamp_pos();
        // Only re-sync floor if screen clamp actually moved y.
        if (self.y - y_before).abs() > f64::EPSILON {
            self.floor_y = self.y.clamp(self.screen_h * 0.55, self.screen_h * 0.85);
        }
    }

    fn clamp_pos(&mut self) {
        self.x = self.x.clamp(Self::SIZE * 0.5, self.screen_w - Self::SIZE * 0.5);
        self.y = self.y.clamp(Self::SIZE * 0.5, self.screen_h - Self::SIZE * 0.5);
    }

    fn update_forced(&mut self, scene: ForceScene, dt: f64) {
        match scene {
            ForceScene::Walking => {
                self.mode = Mode::Walking;
                self.walk_phase = (self.walk_phase + dt * 8.0) % (std::f64::consts::TAU);
                let speed = 55.0 * self.speed_mul();
                let dir = (self.target_x - self.x).signum();
                if dir != 0.0 {
                    self.facing = dir;
                }
                self.x += dir * speed * dt;
                self.y = self.floor_y + self.walk_phase.sin() * 3.0;
                if (self.x - self.target_x).abs() < 4.0 {
                    self.pick_new_target();
                }
            }
            ForceScene::Idle => {
                self.mode = Mode::Idle;
                self.idle_t += dt;
                self.idle_action_t += dt;
                if self.idle_action_t >= self.idle_action.duration() {
                    self.pick_idle_action();
                }
                if self.idle_action == IdleAction::Look
                    && !self.look_flipped
                    && self.idle_action_t > self.idle_action.duration() * 0.4
                {
                    self.facing = -self.facing;
                    self.look_flipped = true;
                }
                self.y = self.floor_y + (self.idle_t * 1.6).sin() * 1.5;
            }
            ForceScene::Sleeping => {
                self.mode = Mode::Sleeping;
                self.sleep_t += dt;
                self.y = self.floor_y + 8.0 + (self.sleep_t * 0.8).sin() * 0.8;
            }
        }
        self.clamp_pos();
    }

    fn tick_walking(&mut self, dt: f64) {
        self.walk_phase = (self.walk_phase + dt * 8.0) % (std::f64::consts::TAU);
        let speed = 55.0 * self.speed_mul();
        let dir = (self.target_x - self.x).signum();
        if dir != 0.0 {
            self.facing = dir;
        }
        self.x += dir * speed * dt;
        self.y = self.floor_y + self.walk_phase.sin() * 3.0;
        if self.clock - self.last_footprint_t > 0.45 && fastrand_chance(0.35) {
            self.last_footprint_t = self.clock;
            self.spawn_particle(
                ParticleKind::Footprint,
                self.x - self.facing * 8.0,
                self.floor_y + 22.0,
                1.5,
            );
        }

        if (self.x - self.target_x).abs() < 4.0 {
            self.pick_new_target();
            if self.mode_elapsed > 3.5 && fastrand_chance(0.4) {
                self.transition(Mode::Idle);
            }
        }
        if self.mode_elapsed > 16.0 && fastrand_chance(0.02) {
            if fastrand_chance(0.45) {
                self.transition(Mode::InBed);
            } else {
                self.transition(Mode::Sleeping);
            }
        }
    }

    fn tick_idle(&mut self, dt: f64) {
        self.idle_t += dt;
        self.idle_action_t += dt;
        if self.idle_action == IdleAction::Look
            && !self.look_flipped
            && self.idle_action_t > self.idle_action.duration() * 0.4
        {
            self.facing = -self.facing;
            self.look_flipped = true;
        }
        self.y = self.floor_y + (self.idle_t * 1.6).sin() * 1.5;

        if self.idle_action_t >= self.idle_action.duration() {
            if self.mode_elapsed < 4.5 && fastrand_chance(0.55) {
                self.pick_idle_action();
            } else if fastrand_chance(0.35) {
                if fastrand_chance(0.5) {
                    self.transition(Mode::Sleeping);
                } else {
                    self.transition(Mode::InBed);
                }
            } else {
                self.pick_new_target();
                self.transition(Mode::Walking);
            }
        }
    }

    fn tick_sleeping(&mut self, dt: f64) {
        self.sleep_t += dt;
        self.y = self.floor_y + 8.0 + (self.sleep_t * 0.8).sin() * 0.8;
        self.tick_sleep_fx();
        if self.mode_elapsed > 14.0 && fastrand_chance(0.02) {
            self.show_bubble("睡饱啦~", 1.5);
            self.transition(Mode::Idle);
        }
    }

    fn tick_going_home(&mut self, dt: f64) {
        let dx = self.home_x - self.x;
        let dy = self.home_y - self.floor_y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 8.0 || self.mode_elapsed > self.mode_until {
            self.x = self.home_x;
            self.y = self.home_y;
            self.floor_y = self.home_y;
            self.transition(Mode::InBed);
            return;
        }
        let speed = 55.0 * self.species.speed(); // ~px/s
        let step = (speed * dt).min(dist);
        self.x += dx / dist * step;
        self.floor_y += dy / dist * step;
        if dx.abs() > 2.0 {
            self.facing = if dx > 0.0 { 1.0 } else { -1.0 };
        }
        self.walk_phase = (self.walk_phase + dt * 9.0) % std::f64::consts::TAU;
        // Align with Walking: bob on top of stable ground, never accumulate into floor.
        self.y = self.floor_y + self.walk_phase.sin() * 2.0;
    }

    fn tick_in_bed(&mut self, dt: f64) {
        self.sleep_t += dt;
        self.y = self.floor_y + 4.0 + (self.sleep_t * 0.7).sin() * 0.6;
        self.tick_sleep_fx();
        if self.mode_elapsed > 22.0 && fastrand_chance(0.01) {
            self.show_bubble("睡饱啦~", 1.5);
            self.transition(Mode::Idle);
        }
    }

    fn tick_sleep_fx(&mut self) {
        if self.clock - self.last_zzz_bubble_t > 5.5 {
            self.last_zzz_bubble_t = self.clock;
            self.show_bubble(bubble::pick_sleepy(fastrand_u64()), 2.2);
        }
        if fastrand_chance(0.03) {
            self.spawn_particle(
                ParticleKind::Zzz,
                self.x + 20.0 + ((fastrand_u64() % 16) as f64),
                self.y - 28.0,
                2.2,
            );
        }
        if self.clock - self.last_dream_t > 8.0 && fastrand_chance(0.01) {
            self.last_dream_t = self.clock;
            let food = match self.species {
                Species::Cat => "🐟",
                Species::Pig => "🥕",
                Species::Bear => "🍯",
            };
            self.particles.push(
                Particle::new(ParticleKind::Dream, self.x + 12.0, self.y - 40.0, 2.6)
                    .with_label(food),
            );
        }
    }

    /// Single mode-switch entry: resets timers; callers may override `mode_until`.
    fn transition(&mut self, mode: Mode) {
        self.mode = mode;
        self.mode_elapsed = 0.0;
        match mode {
            Mode::Idle => {
                self.idle_t = 0.0;
                self.pick_idle_action();
            }
            Mode::GoingHome => {
                self.mode_until = 12.0;
            }
            Mode::Sleeping | Mode::InBed => {
                self.sleep_t = 0.0;
                self.show_bubble(bubble::pick_sleepy(fastrand_u64()), 2.2);
                self.last_zzz_bubble_t = self.clock;
            }
            Mode::Dizzy => {
                self.dizzy_t = 0.0;
            }
            Mode::Interested => {
                self.mode_until = 3.0 + (fastrand_u64() % 2500) as f64 / 1000.0;
                self.interested_jitter = (fastrand_u64() % 628) as f64 / 100.0;
            }
            Mode::Watching => {
                self.mode_until = 0.9 + (fastrand_u64() % 900) as f64 / 1000.0;
            }
            Mode::Chasing => {
                self.mode_until = 2.2 + (fastrand_u64() % 1800) as f64 / 1000.0;
                self.chase_t = 0.0;
            }
            Mode::Clingy => {
                self.clingy_arrived = false;
                self.mode_until = 9.0;
            }
            Mode::Pet => {}
            Mode::Photo => {
                self.photo_t = 0.0;
            }
            _ => {}
        }
    }

    fn pick_idle_action(&mut self) {
        let margin = 100.0;
        let near_edge = self.x - margin < 40.0 || self.screen_w - self.x < margin + 40.0;
        self.idle_action = IdleAction::pick(self.species, near_edge);
        self.idle_action_t = 0.0;
        self.look_flipped = false;
        // bear back-scratch: face toward nearest screen edge
        if self.idle_action == IdleAction::BackScratch {
            self.facing = if self.x < self.screen_w * 0.5 {
                -1.0
            } else {
                1.0
            };
        }
        if let Some(text) = bubble::idle_start_bubble(self.idle_action, self.species) {
            self.show_bubble(text, 1.4);
        }
        if self.idle_action == IdleAction::MudRoll {
            for _ in 0..3 {
                self.spawn_particle(
                    ParticleKind::Mud,
                    self.x + ((fastrand_u64() % 30) as f64 - 15.0),
                    self.y + 10.0,
                    0.7,
                );
            }
        }
    }

    fn pick_new_target(&mut self) {
        let margin = Self::SIZE;
        let lo = margin;
        let hi = (self.screen_w - margin).max(lo + 1.0);
        const MIN_DIST: f64 = 48.0;
        for _ in 0..12 {
            let t = (fastrand_u64() as f64) / (u64::MAX as f64);
            let tx = lo + t * (hi - lo);
            if (tx - self.x).abs() >= MIN_DIST {
                self.target_x = tx;
                return;
            }
        }
        // Fallback: flip to the far side so we never "arrive" immediately.
        self.target_x = if self.x < (lo + hi) * 0.5 { hi } else { lo };
    }
}


fn fastrand_u64() -> u64 {
    rng::next_u64()
}

fn fastrand_chance(p: f64) -> bool {
    (fastrand_u64() as f64) / (u64::MAX as f64) < p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_still_decays_move_amt() {
        rng::seed(1);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.clock = 0.0;
        pet.note_cursor(Some((100.0, 100.0)));
        pet.clock = 0.05;
        pet.note_cursor(Some((250.0, 100.0)));
        assert!(
            pet.cursor_move_amt > 60.0,
            "expected motion, got {}",
            pet.cursor_move_amt
        );
        // Hold still past the 1.5s trail window — amt must decay to ~0.
        for i in 0..20 {
            pet.clock = 0.05 + i as f64 * 0.2;
            pet.note_cursor(Some((250.0, 100.0)));
        }
        assert!(
            pet.cursor_move_amt < 1.0,
            "stationary cursor should decay amt, got {}",
            pet.cursor_move_amt
        );
    }

    #[test]
    fn going_home_reaches_home_x() {
        rng::seed(2);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.x = pet.home_x - 180.0;
        pet.floor_y = pet.home_y;
        pet.y = pet.floor_y;
        pet.go_to_bed();
        assert_eq!(pet.mode, Mode::GoingHome);
        for _ in 0..900 {
            pet.update(1.0 / 60.0);
            if pet.mode == Mode::InBed {
                break;
            }
        }
        assert_eq!(pet.mode, Mode::InBed);
        assert!(
            (pet.x - pet.home_x).abs() < 1.0,
            "x={} home={}",
            pet.x,
            pet.home_x
        );
        assert!((pet.floor_y - pet.home_y).abs() < 1.0);
    }

    #[test]
    fn gifting_does_not_teleport_floor_y() {
        rng::seed(3);
        let mut pet = Pet::new(1440.0, 900.0);
        let floor0 = pet.floor_y;
        // High cursor used to rewrite floor_y immediately in start_gifting.
        pet.cursor = Some((800.0, 80.0));
        pet.start_gifting();
        assert_eq!(pet.mode, Mode::Gifting);
        assert!(
            (pet.floor_y - floor0).abs() < 0.01,
            "floor teleported from {floor0} to {}",
            pet.floor_y
        );
    }

    #[test]
    fn clingy_bob_does_not_drift_floor() {
        rng::seed(4);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.cursor = Some((pet.x + 400.0, pet.floor_y));
        pet.start_clingy();
        pet.target_y = pet.floor_y; // horizontal approach
        let floor0 = pet.floor_y;
        for _ in 0..90 {
            pet.update(1.0 / 60.0);
            if matches!(pet.mode, Mode::Clingy) && !pet.clingy_arrived {
                // Bob lives in y, not floor.
                assert!(
                    (pet.y - pet.floor_y).abs() <= 2.01,
                    "y={} floor={}",
                    pet.y,
                    pet.floor_y
                );
            }
        }
        assert!(
            (pet.floor_y - floor0).abs() < 0.5,
            "floor drifted by {}",
            pet.floor_y - floor0
        );
    }

    #[test]
    fn pick_new_target_rejects_near() {
        rng::seed(5);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.x = 400.0;
        for _ in 0..30 {
            pet.pick_new_target();
            assert!(
                (pet.target_x - pet.x).abs() >= 48.0 - 1e-6,
                "target {} too close to x={}",
                pet.target_x,
                pet.x
            );
        }
    }

    #[test]
    fn show_bubble_expires() {
        rng::seed(6);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.show_bubble("喵~", 0.5);
        assert!(pet.bubble.is_some());
        for _ in 0..40 {
            pet.update(1.0 / 60.0);
        }
        assert!(pet.bubble.is_none(), "bubble should expire");
    }

    #[test]
    fn short_click_enters_trick() {
        rng::seed(7);
        let mut pet = Pet::new(1440.0, 900.0);
        pet.mode = Mode::Idle;
        pet.on_short_click();
        assert_eq!(pet.mode, Mode::Trick);
        assert!(pet.trick_action.is_some());
        assert!(
            pet.bubble.is_some()
                || matches!(
                    pet.trick_action,
                    Some(
                        TrickAction::Spin
                            | TrickAction::Pounce
                            | TrickAction::HappyJump
                            | TrickAction::SwatCursor
                    )
                ),
            "meow/heart/etc should speak; silent actions ok"
        );
    }
}
