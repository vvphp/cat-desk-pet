//! Lightweight desktop-space particles (hearts, zzz, dust, …).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleKind {
    Heart,
    Zzz,
    Dust,
    Kiss,
    Footprint,
    Mud,
    Dream,
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub kind: ParticleKind,
    pub x: f64,
    pub y: f64,
    pub age: f64,
    pub life: f64,
    /// Optional label (dream emoji / custom).
    pub label: Option<&'static str>,
    pub vx: f64,
    pub vy: f64,
}

impl Particle {
    pub fn new(kind: ParticleKind, x: f64, y: f64, life: f64) -> Self {
        let (vx, vy) = match kind {
            ParticleKind::Heart | ParticleKind::Kiss | ParticleKind::Zzz | ParticleKind::Dream => {
                (0.0, -28.0)
            }
            ParticleKind::Dust => (0.0, -12.0),
            ParticleKind::Mud => {
                (
                    ((super_rng() % 40) as f64 - 20.0),
                    -40.0 - (super_rng() % 20) as f64,
                )
            }
            ParticleKind::Footprint => (0.0, 0.0),
        };
        Self {
            kind,
            x,
            y,
            age: 0.0,
            life,
            label: None,
            vx,
            vy,
        }
    }

    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn t(&self) -> f64 {
        (self.age / self.life).clamp(0.0, 1.0)
    }

    pub fn alpha(&self) -> f64 {
        let t = self.t();
        if t < 0.15 {
            t / 0.15
        } else {
            1.0 - (t - 0.15) / 0.85
        }
    }
}

fn super_rng() -> u64 {
    super::rng::next_u64()
}

pub fn tick_particles(particles: &mut Vec<Particle>, dt: f64) {
    for p in particles.iter_mut() {
        p.age += dt;
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        match p.kind {
            ParticleKind::Dust => {
                p.vx *= 0.92;
                p.vy *= 0.95;
            }
            ParticleKind::Mud => {
                p.vy += 120.0 * dt;
            }
            _ => {}
        }
    }
    particles.retain(|p| p.age < p.life);
}
