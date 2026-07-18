//! Pet behaviour modes + frame pacing.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Walking,
    Idle,
    Sleeping,
    /// Walk toward the corner bed, then become `InBed`.
    GoingHome,
    InBed,
    Dragged,
    Dizzy,
    /// Held/stroked without dragging — hearts face until release.
    Pet,
    /// Walk over to idle cursor after long inactivity.
    Clingy,
    Interested,
    Watching,
    Chasing,
    Feeding,
    Playing,
    BirdWatch,
    ButterflyNose,
    Startled,
    Photo,
    Gifting,
    /// Short-click reaction (meow / heart / spin / …).
    Trick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrickAction {
    Meow,
    Heart,
    Spin,
    Pounce,
    HappyJump,
    Grumpy,
    Wave,
    Shy,
    Kiss,
    SwatCursor,
}

impl TrickAction {
    pub fn duration(self) -> f64 {
        match self {
            TrickAction::Meow => 0.9,
            TrickAction::Heart => 1.3,
            TrickAction::Spin => 0.7,
            TrickAction::Pounce => 0.6,
            TrickAction::HappyJump => 0.9,
            TrickAction::Grumpy => 1.2,
            TrickAction::Wave => 1.4,
            TrickAction::Shy => 1.6,
            TrickAction::Kiss => 1.1,
            TrickAction::SwatCursor => 0.7,
        }
    }
}

impl Mode {
    pub fn frame_interval(self) -> Duration {
        match self {
            Mode::Sleeping | Mode::InBed => Duration::from_millis(125),
            Mode::Idle | Mode::Dizzy | Mode::Pet | Mode::Watching | Mode::BirdWatch => {
                Duration::from_millis(66)
            }
            Mode::Walking
            | Mode::GoingHome
            | Mode::Clingy
            | Mode::Interested
            | Mode::Feeding
            | Mode::ButterflyNose
            | Mode::Gifting
            | Mode::Trick => Duration::from_millis(40),
            Mode::Dragged
            | Mode::Chasing
            | Mode::Playing
            | Mode::Startled
            | Mode::Photo => Duration::from_millis(33),
        }
    }

    pub fn is_asleep(self) -> bool {
        matches!(self, Mode::Sleeping | Mode::InBed)
    }

    pub(crate) fn cursor_locked(self) -> bool {
        matches!(
            self,
            Mode::Sleeping
                | Mode::GoingHome
                | Mode::InBed
                | Mode::Dragged
                | Mode::Dizzy
                | Mode::Pet
                | Mode::Clingy
                | Mode::Feeding
                | Mode::Playing
                | Mode::ButterflyNose
                | Mode::Startled
                | Mode::Photo
                | Mode::Gifting
                | Mode::Trick
        )
    }
}
