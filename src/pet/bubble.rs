//! Speech bubbles + species sound tables (ported from WebView desktop-cat).

use super::{IdleAction, Species};

#[derive(Clone, Debug)]
pub struct SpeechBubble {
    pub text: String,
    pub age: f64,
    pub dur: f64,
}

impl SpeechBubble {
    pub fn alive(&self) -> bool {
        self.age < self.dur
    }

    /// 0..1 pop scale (matches CSS bubble-pop ~0.9s, compressed to ~0.35s).
    pub fn pop_scale(&self) -> f64 {
        let t = (self.age / 0.35).clamp(0.0, 1.0);
        if t < 0.33 {
            0.4 + (t / 0.33) * 0.75
        } else if t < 0.66 {
            1.15 - ((t - 0.33) / 0.33) * 0.15
        } else {
            1.0
        }
    }
}

#[derive(Clone, Copy)]
enum MoodPool {
    Happy,
    Excited,
    Curious,
    Sleepy,
    Grumpy,
    Shy,
    Neutral,
}

fn pool(species: Species, mood: MoodPool) -> &'static [&'static str] {
    match (species, mood) {
        (Species::Cat, MoodPool::Happy) => &["喵~", "喵♪", "咕噜咕噜~", "喵呼~", "喵♡"],
        (Species::Cat, MoodPool::Excited) => &["喵!", "喵喵!", "嗷呜!", "喵呜!", "喵—!"],
        (Species::Cat, MoodPool::Curious) => &["喵?", "嗯?", "...?", "喵嗯?", "唔?"],
        (Species::Cat, MoodPool::Sleepy) => &["唔...", "喵...", "...zzz", "嗯......"],
        (Species::Cat, MoodPool::Grumpy) => &["嘶!", "哼!", "fff...", "走开!"],
        (Species::Cat, MoodPool::Shy) => &["...", "唔...", "(///)", "嗯?"],
        (Species::Cat, MoodPool::Neutral) => &["喵~", "喵喵~", "喵呜~", "咕噜咕噜..."],
        (Species::Pig, MoodPool::Happy) => &["哼噜~", "呼噜~", "嘿嘿~", "哼♪"],
        (Species::Pig, MoodPool::Excited) => &["哼!", "嘿嘿!", "嗯哼!"],
        (Species::Pig, MoodPool::Curious) => &["哼?", "嗯?", "...?"],
        (Species::Pig, MoodPool::Sleepy) => &["呼...", "哼...", "..."],
        (Species::Pig, MoodPool::Grumpy) => &["哼!", "呼噜!", "走开!"],
        (Species::Pig, MoodPool::Shy) => &["...", "唔...", "(///)"],
        (Species::Pig, MoodPool::Neutral) => &["哼噜~", "呼~", "嘿~"],
        (Species::Bear, MoodPool::Happy) => &["嗷呜~", "嗯~", "呼噜~", "唔~"],
        (Species::Bear, MoodPool::Excited) => &["嗷!", "吼!", "呜啊!"],
        (Species::Bear, MoodPool::Curious) => &["嗷?", "嗯?", "...?"],
        (Species::Bear, MoodPool::Sleepy) => &["呼...", "嗯...", "ZZ..."],
        (Species::Bear, MoodPool::Grumpy) => &["吼!", "呼!", "嗷!"],
        (Species::Bear, MoodPool::Shy) => &["...", "唔...", "嗯..."],
        (Species::Bear, MoodPool::Neutral) => &["嗷~", "嗯~", "呼~"],
    }
}

pub fn pick(xs: &[&str], rng: u64) -> String {
    xs[(rng as usize) % xs.len()].to_string()
}

pub fn pick_meow(
    species: Species,
    mood: f64,
    asleep: bool,
    cursor_move_amt: f64,
    secs_since_sig_move: f64,
    rng: u64,
) -> String {
    let p = if asleep {
        MoodPool::Sleepy
    } else if mood < 30.0 {
        MoodPool::Grumpy
    } else if cursor_move_amt > 220.0 {
        MoodPool::Excited
    } else if secs_since_sig_move > 30.0 {
        MoodPool::Curious
    } else if mood > 70.0 {
        MoodPool::Happy
    } else if mood < 40.0 && secs_since_sig_move > 20.0 {
        MoodPool::Shy
    } else {
        MoodPool::Neutral
    };
    pick(pool(species, p), rng)
}

pub fn pick_curious(species: Species, rng: u64) -> String {
    pick(pool(species, MoodPool::Curious), rng)
}

pub fn pick_hearts(rng: u64) -> String {
    pick(&["❤", "❤❤", "♡", "(´• ω •`)♡"], rng)
}

pub fn pick_sleepy(rng: u64) -> String {
    pick(&["zzz...", "Zzz", "💤"], rng)
}

pub fn pick_grumpy_line(rng: u64) -> String {
    pick(&["嘶!", "哼!", "走开!", "別碰!"], rng)
}

pub fn pick_wave(rng: u64) -> String {
    pick(&["嗨~", "哟~", "Hi!"], rng)
}

pub fn pick_shy(rng: u64) -> String {
    pick(&["...", "唔...", "(///∇///)"], rng)
}

pub fn pick_kiss(rng: u64) -> String {
    pick(&["啾~", "mua~", "啾♡"], rng)
}

pub fn eat_bubble(species: Species) -> &'static str {
    match species {
        Species::Cat => "好饱~ 喵呜!",
        Species::Pig => "哼噜~ 好吃!",
        Species::Bear => "嗯~ 好甜~♡",
    }
}

pub fn eat_heart_count(species: Species) -> u32 {
    match species {
        Species::Cat => 0,
        Species::Pig => 1,
        Species::Bear => 4,
    }
}

pub fn idle_start_bubble(action: IdleAction, species: Species) -> Option<&'static str> {
    match action {
        IdleAction::MudRoll => Some(match species {
            Species::Cat => "哼噜~ 舒服!",
            Species::Pig => "哼噜~ 舒服!",
            Species::Bear => "哼噜~ 舒服!",
        }),
        IdleAction::BackScratch => Some(match species {
            Species::Cat => "嗷~ 舒服~",
            Species::Pig => "嗷~ 舒服~",
            Species::Bear => "嗷~ 舒服~",
        }),
        IdleAction::Yawn => Some(match species {
            Species::Cat => "哈欠~",
            Species::Pig => "呼啊~",
            Species::Bear => "呼啊~",
        }),
        IdleAction::Sit | IdleAction::Stretch | IdleAction::Look | IdleAction::TailCurl => None,
    }
}

pub fn toy_done_bubble(rng: u64) -> String {
    pick(&["玩够了~", "哼~", "下次再玩!", "满足~"], rng)
}
