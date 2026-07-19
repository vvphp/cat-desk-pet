//! Softbuffer blit: original WebView SVG (via resvg) + procedural props.

#![allow(dead_code)] // shared ellipse/rect helpers + walking fallback body

use crate::pet::{
    CoatColor, FlyerKind, GiftKind, Mode, ParticleKind, Pet, Species, ToyKind,
};
use crate::sprite::{self, SpriteCache};
use crate::text;

pub const WIN: u32 = 180;

pub fn pack(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub fn clear(buf: &mut [u32]) {
    buf.fill(0);
}

/// Draw pet + world props into a logical canvas.
/// `origin_x/y` = desktop coords of the window's top-left (so props far from the
/// pet stay visible when the OS window has grown to `visible_bounds`).
pub fn draw_pet(
    buf: &mut [u32],
    w: u32,
    h: u32,
    pet: &Pet,
    origin_x: f64,
    origin_y: f64,
    sprites: &mut SpriteCache,
) {
    clear(buf);
    let to_local = |x: f64, y: f64| (x - origin_x, y - origin_y);
    let (cx, cy) = to_local(pet.x, pet.y);

    if pet.mode == Mode::InBed {
        let (bx, by) = to_local(pet.home_x, pet.home_y);
        draw_bed(buf, w, h, bx, by + 18.0);
    } else if pet.mode == Mode::GoingHome {
        let (bx, by) = to_local(pet.home_x, pet.home_y);
        draw_bed(buf, w, h, bx, by + 18.0);
    }

    let bob = match pet.mode {
        Mode::Walking
        | Mode::GoingHome
        | Mode::Clingy
        | Mode::Interested
        | Mode::Chasing
        | Mode::Playing
        | Mode::Trick => pet.walk_phase.sin() * 3.0,
        Mode::Idle => (pet.idle_t * 1.6).sin() * 1.5,
        Mode::Pet => 0.0, // bob applied in pet tick via y
        Mode::Sleeping | Mode::InBed => (pet.sleep_t * 0.8).sin() * 0.8 + 6.0,
        Mode::Dragged => -8.0,
        Mode::Dizzy => (pet.dizzy_t * 20.0).sin() * 2.0,
        Mode::Feeding if pet.eat_anim_t > 0.0 => (pet.eat_anim_t * 14.0).sin() * 2.0,
        Mode::Gifting => (pet.walk_phase).sin() * 1.2,
        _ => 0.0,
    };

    let sprite = sprites.pixels_for(pet);
    if !sprite.is_empty() {
        sprite::blit_sprite(buf, w, h, sprite, pet.facing, bob, cx, cy);
    } else {
        // fallback if resvg fails
        draw_walking(buf, w, h, cx, cy + bob, pet);
    }

    if pet.mode == Mode::Sleeping || pet.mode == Mode::InBed {
        let z_alpha = (((pet.sleep_t * 1.2).sin() * 0.5 + 0.5) * 220.0) as u8;
        draw_z(buf, w, h, cx + 36.0, cy - 36.0 + bob, z_alpha);
    }

    // World props in the same desktop→local space as the pet.
    if let Some(feed) = &pet.feed {
        let (fx, fy) = to_local(feed.x, feed.y);
        draw_food(buf, w, h, fx, fy, feed.eat_t.is_some(), pet.species);
    }
    if let Some(toy) = &pet.toy {
        if toy.kind == ToyKind::Laser {
            draw_laser_trail(buf, w, h, pet, origin_x, origin_y);
        }
        let (tx, ty) = to_local(toy.x, toy.y);
        match toy.kind {
            ToyKind::Yarn => draw_yarn(buf, w, h, tx, ty, toy.age),
            ToyKind::Ball => draw_ball(buf, w, h, tx, ty),
            ToyKind::Paper => draw_paper(buf, w, h, tx, ty, toy.spin),
            ToyKind::Mouse => draw_mouse_toy(buf, w, h, tx, ty, toy.spin),
            ToyKind::Laser => draw_laser(buf, w, h, tx, ty, toy.age),
            ToyKind::Wand => draw_wand(buf, w, h, tx, ty, toy.spin),
        }
    }
    if let Some(flyer) = &pet.flyer {
        let (fx, fy) = to_local(flyer.x, flyer.y);
        match flyer.kind {
            FlyerKind::Bird => draw_bird(buf, w, h, fx, fy, flyer.vx),
            FlyerKind::Butterfly => draw_butterfly(buf, w, h, fx, fy, flyer.age),
        }
    }
    for p in &pet.particles {
        let (px, py) = to_local(p.x, p.y);
        draw_particle(buf, w, h, px, py, p);
    }

    if let Some(b) = &pet.bubble {
        let (bx, by) = to_local(pet.x, pet.y - 58.0);
        draw_speech_bubble(buf, w, h, bx, by, b);
    }

    if let Some(gift) = &pet.gift {
        let (gx, gy) = to_local(gift.x, gift.y);
        draw_gift(buf, w, h, gx, gy, gift.kind, gift.fade);
    }

    // Camera flash: punch opaque pixels toward white (keep clear areas clear).
    if pet.flash > 0.02 {
        for px in buf.iter_mut() {
            let a = ((*px >> 24) & 0xFF) as u8;
            if a < 16 {
                continue;
            }
            let r = ((*px >> 16) & 0xFF) as u8;
            let g = ((*px >> 8) & 0xFF) as u8;
            let b = (*px & 0xFF) as u8;
            let k = pet.flash;
            let nr = r.saturating_add(((255 - r) as f64 * k) as u8);
            let ng = g.saturating_add(((255 - g) as f64 * k) as u8);
            let nb = b.saturating_add(((255 - b) as f64 * k) as u8);
            *px = pack(a, nr, ng, nb);
        }
    }
}

fn fur(pet: &Pet) -> (u8, u8, u8) {
    pet.coat.palette().0
}
fn fur_dark(pet: &Pet) -> (u8, u8, u8) {
    pet.coat.palette().1
}
fn ink(pet: &Pet) -> (u8, u8, u8) {
    pet.coat.palette().2
}
fn belly(pet: &Pet) -> (u8, u8, u8) {
    pet.coat.palette().3
}
fn accent(pet: &Pet) -> (u8, u8, u8) {
    pet.coat.palette().4
}

fn draw_coat_pattern(buf: &mut [u32], w: u32, h: u32, cx: f64, cy: f64, pet: &Pet) {
    let (ar, ag, ab) = accent(pet);
    let (br, bg, bb) = belly(pet);
    match pet.coat {
        CoatColor::Orange | CoatColor::Pink | CoatColor::Cream | CoatColor::Brown => {}
        CoatColor::Calico => {
            fill_ellipse(buf, w, h, cx - 14.0, cy - 6.0, 10.0, 8.0, ar, ag, ab);
            fill_ellipse(buf, w, h, cx + 16.0, cy + 4.0, 9.0, 7.0, 0x2C, 0x28, 0x28);
            fill_ellipse(buf, w, h, cx, cy + 14.0, 12.0, 8.0, br, bg, bb);
        }
        CoatColor::Cow => {
            fill_ellipse(buf, w, h, cx - 12.0, cy - 4.0, 9.0, 8.0, ar, ag, ab);
            fill_ellipse(buf, w, h, cx + 14.0, cy + 6.0, 10.0, 7.0, ar, ag, ab);
            fill_ellipse(buf, w, h, cx + 4.0, cy - 18.0, 7.0, 6.0, ar, ag, ab);
        }
        CoatColor::Tabby => {
            for i in 0..3 {
                let oy = cy - 10.0 + i as f64 * 8.0;
                fill_ellipse(buf, w, h, cx - 6.0, oy, 16.0, 2.2, ar, ag, ab);
            }
        }
        CoatColor::Tuxedo => {
            // white chest / muzzle
            fill_ellipse(buf, w, h, cx, cy + 10.0, 14.0, 12.0, br, bg, bb);
            fill_ellipse(buf, w, h, cx + pet.facing * 18.0, cy - 8.0, 10.0, 8.0, br, bg, bb);
        }
        CoatColor::Black => {
            fill_ellipse(buf, w, h, cx, cy + 12.0, 12.0, 9.0, br, bg, bb);
        }
        CoatColor::Polar => {
            fill_ellipse(buf, w, h, cx, cy + 12.0, 14.0, 10.0, br, bg, bb);
        }
    }
}

fn draw_walking(buf: &mut [u32], w: u32, h: u32, cx: f64, cy: f64, pet: &Pet) {
    let (r, g, b) = fur(pet);
    let (dr, dg, db) = fur_dark(pet);
    let (ir, ig, ib) = ink(pet);
    let bob = pet.walk_phase.sin() * 3.0;
    let leg = pet.walk_phase.sin() * 6.0;
    fill_ellipse(buf, w, h, cx, cy + bob, 28.0, 21.0, r, g, b);
    fill_ellipse(
        buf,
        w,
        h,
        cx + pet.facing * 28.0,
        cy - 18.0 + bob,
        26.0,
        24.0,
        r,
        g,
        b,
    );
    draw_coat_pattern(buf, w, h, cx, cy + bob, pet);
    ear(buf, w, h, cx + pet.facing * 18.0, cy - 40.0 + bob, pet.facing, pet);
    ear(buf, w, h, cx + pet.facing * 36.0, cy - 38.0 + bob, pet.facing, pet);
    fill_ellipse(
        buf,
        w,
        h,
        cx + pet.facing * 34.0,
        cy - 20.0 + bob,
        3.0,
        4.0,
        ir,
        ig,
        ib,
    );
    draw_face(
        buf,
        w,
        h,
        cx + pet.facing * 28.0,
        cy - 18.0 + bob,
        pet,
    );
    fill_ellipse(
        buf,
        w,
        h,
        cx - 16.0,
        cy + 22.0 + bob + leg,
        7.0,
        10.0,
        dr,
        dg,
        db,
    );
    fill_ellipse(
        buf,
        w,
        h,
        cx + 16.0,
        cy + 22.0 + bob - leg,
        7.0,
        10.0,
        dr,
        dg,
        db,
    );
    draw_tail(
        buf,
        w,
        h,
        cx - pet.facing * 40.0,
        cy - 4.0 + bob + leg * 0.4,
        pet,
    );
}





fn draw_gift(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, kind: GiftKind, fade: f64) {
    if fade < 0.05 {
        return;
    }
    let a = (fade * 255.0) as u8;
    match kind {
        GiftKind::Leaf => {
            fill_ellipse_alpha(buf, w, h, x, y, 10.0, 6.0, a, 0xC4, 0x8A, 0x2A);
            fill_ellipse_alpha(buf, w, h, x + 4.0, y - 2.0, 7.0, 4.0, a, 0x8B, 0x5A, 0x2B);
        }
        GiftKind::Flower => {
            for i in 0..5 {
                let ang = i as f64 * std::f64::consts::TAU / 5.0;
                fill_ellipse_alpha(
                    buf,
                    w,
                    h,
                    x + ang.cos() * 6.0,
                    y + ang.sin() * 6.0,
                    4.0,
                    4.0,
                    a,
                    0xF4,
                    0xA0,
                    0xC8,
                );
            }
            fill_ellipse_alpha(buf, w, h, x, y, 3.5, 3.5, a, 0xFF, 0xE0, 0x6A);
        }
        GiftKind::Mouse => {
            fill_ellipse_alpha(buf, w, h, x, y, 9.0, 7.0, a, 0xB0, 0xB0, 0xB8);
            fill_ellipse_alpha(buf, w, h, x - 7.0, y - 5.0, 3.5, 3.5, a, 0xB0, 0xB0, 0xB8);
            fill_ellipse_alpha(buf, w, h, x - 7.0, y + 5.0, 3.5, 3.5, a, 0xB0, 0xB0, 0xB8);
            fill_ellipse_alpha(buf, w, h, x + 5.0, y - 1.0, 1.5, 1.5, a, 0x20, 0x20, 0x28);
        }
        GiftKind::Candy => {
            fill_ellipse_alpha(buf, w, h, x, y, 8.0, 5.0, a, 0xFF, 0x6B, 0x8A);
            fill_triangle_alpha(
                buf,
                w,
                h,
                x - 8.0,
                y,
                x - 14.0,
                y - 5.0,
                x - 14.0,
                y + 5.0,
                a,
                0xFF,
                0xC0,
                0x40,
            );
            fill_triangle_alpha(
                buf,
                w,
                h,
                x + 8.0,
                y,
                x + 14.0,
                y - 5.0,
                x + 14.0,
                y + 5.0,
                a,
                0xFF,
                0xC0,
                0x40,
            );
        }
    }
}

fn fill_triangle_alpha(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    a: u8,
    r: u8,
    g: u8,
    b: u8,
) {
    let min_x = (x0.min(x1).min(x2) - 1.0).floor().max(0.0) as i32;
    let max_x = (x0.max(x1).max(x2) + 1.0).ceil().min(w as f64 - 1.0) as i32;
    let min_y = (y0.min(y1).min(y2) - 1.0).floor().max(0.0) as i32;
    let max_y = (y0.max(y1).max(y2) + 1.0).ceil().min(h as f64 - 1.0) as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let sd = sd_triangle(px, py, x0, y0, x1, y1, x2, y2);
            let cov = (0.5 - sd).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let aa = ((a as f64) * cov).round().clamp(0.0, 255.0) as u8;
            if aa > 0 {
                blend_over(buf, w, h, x as u32, y as u32, aa, r, g, b);
            }
        }
    }
}

fn draw_food(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, eating: bool, species: Species) {
    let fade = if eating { 0.55 } else { 1.0 };
    match species {
        Species::Cat => {
            let r = (0x5B as f64 * fade) as u8;
            let g = (0x9B as f64 * fade) as u8;
            let b = (0xD5 as f64 * fade) as u8;
            fill_ellipse(buf, w, h, x, y, 14.0, 8.0, r, g, b);
            fill_triangle(
                buf,
                w,
                h,
                x - 14.0,
                y,
                x - 22.0,
                y - 7.0,
                x - 22.0,
                y + 7.0,
                r,
                g,
                b,
            );
            fill_ellipse(buf, w, h, x + 6.0, y - 1.0, 1.8, 1.8, 0x1A, 0x1A, 0x2A);
        }
        Species::Pig => {
            // carrot
            let r = (0xF0 as f64 * fade) as u8;
            let g = (0x8A as f64 * fade) as u8;
            let b = (0x2A as f64 * fade) as u8;
            fill_triangle(buf, w, h, x, y + 12.0, x - 7.0, y - 6.0, x + 7.0, y - 6.0, r, g, b);
            fill_ellipse(buf, w, h, x - 3.0, y - 10.0, 3.0, 5.0, 0x4C, 0xA8, 0x3A);
            fill_ellipse(buf, w, h, x + 3.0, y - 10.0, 3.0, 5.0, 0x4C, 0xA8, 0x3A);
        }
        Species::Bear => {
            // honey pot
            let r = (0xE8 as f64 * fade) as u8;
            let g = (0xB8 as f64 * fade) as u8;
            let b = (0x3A as f64 * fade) as u8;
            fill_ellipse(buf, w, h, x, y + 2.0, 12.0, 10.0, r, g, b);
            fill_ellipse(buf, w, h, x, y - 8.0, 10.0, 4.0, 0xC4, 0x8A, 0x28);
            fill_ellipse(buf, w, h, x, y - 2.0, 6.0, 3.0, 0xFF, 0xD2, 0x6A);
        }
    }
}

fn draw_yarn(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, age: f64) {
    fill_ellipse(buf, w, h, x, y, 11.0, 11.0, 0xE8, 0x6A, 0x8A);
    let spin = age * 6.0;
    for i in 0..5 {
        let a = spin + i as f64 * 0.9;
        let px = x + a.cos() * 7.0;
        let py = y + a.sin() * 7.0;
        fill_ellipse(buf, w, h, px, py, 2.0, 2.0, 0xC0, 0x40, 0x60);
    }
}

fn draw_ball(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64) {
    fill_ellipse(buf, w, h, x, y, 10.0, 10.0, 0x6E, 0xC6, 0x6E);
    fill_ellipse(buf, w, h, x - 3.0, y - 3.0, 3.0, 2.5, 0xB8, 0xE8, 0xB8);
}

fn draw_paper(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, spin: f64) {
    let ang = spin.to_radians();
    let c = ang.cos();
    let s = ang.sin();
    // crumpled diamond-ish square
    let corners = [(-7.0, -6.0), (7.0, -5.0), (6.0, 7.0), (-6.0, 6.0)];
    for i in 0..4 {
        let (ax, ay) = corners[i];
        let (bx, by) = corners[(i + 1) % 4];
        let axr = x + ax * c - ay * s;
        let ayr = y + ax * s + ay * c;
        let bxr = x + bx * c - by * s;
        let byr = y + bx * s + by * c;
        draw_line_alpha(buf, w, h, axr, ayr, bxr, byr, 1.6, 255, 0xE8, 0xE4, 0xD8);
    }
    fill_ellipse(buf, w, h, x, y, 5.5, 4.5, 0xF2, 0xEE, 0xE0);
}

fn draw_mouse_toy(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, spin: f64) {
    let bob = (spin * 0.08).sin() * 1.5;
    fill_ellipse(buf, w, h, x, y + bob, 9.0, 7.0, 0xA8, 0xA8, 0xB0);
    fill_ellipse(buf, w, h, x - 7.0, y - 4.0 + bob, 3.5, 3.5, 0xA8, 0xA8, 0xB0);
    fill_ellipse(buf, w, h, x - 7.0, y + 4.0 + bob, 3.5, 3.5, 0xA8, 0xA8, 0xB0);
    fill_ellipse(buf, w, h, x + 5.0, y - 1.0 + bob, 1.5, 1.5, 0x20, 0x20, 0x28);
    // tail
    fill_ellipse(buf, w, h, x - 12.0, y + 2.0 + bob, 5.0, 1.6, 0xC0, 0x80, 0x90);
}

fn draw_laser(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, age: f64) {
    let pulse = 1.0 + (age * 28.0).sin() * 0.18;
    let r = 5.0 * pulse;
    fill_ellipse(buf, w, h, x, y, r + 3.0, r + 3.0, 0xFF, 0x80, 0x80);
    fill_ellipse(buf, w, h, x, y, r, r, 0xFF, 0x35, 0x35);
    fill_ellipse(buf, w, h, x - 1.0, y - 1.0, 1.8, 1.8, 0xFF, 0xC8, 0xC8);
}

fn draw_wand(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, spin_deg: f64) {
    let ang = spin_deg.to_radians();
    let dx = ang.cos();
    let dy = ang.sin();
    // stick
    for i in 0..10 {
        let t = i as f64;
        fill_ellipse(
            buf,
            w,
            h,
            x - dx * t * 2.2,
            y - dy * t * 2.2 + 8.0,
            1.6,
            1.6,
            0x8B,
            0x5A,
            0x2B,
        );
    }
    // feather tuft
    for i in 0..5 {
        let a = ang + (i as f64 - 2.0) * 0.35;
        let fx = x + a.cos() * 12.0;
        let fy = y + a.sin() * 12.0 - 2.0;
        fill_ellipse(buf, w, h, fx, fy, 4.0, 2.2, 0xE8, 0xD0, 0x6A);
    }
    fill_ellipse(buf, w, h, x, y, 3.5, 3.5, 0xF4, 0xE8, 0xA0);
}

fn draw_laser_trail(buf: &mut [u32], w: u32, h: u32, pet: &Pet, origin_x: f64, origin_y: f64) {
    let pts: Vec<_> = pet.laser_trail.iter().copied().collect();
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();
    for i in 1..n {
        let a = pts[i - 1];
        let b = pts[i];
        let ax = a.x - origin_x;
        let ay = a.y - origin_y;
        let bx = b.x - origin_x;
        let by = b.y - origin_y;
        let op = ((i as f64 / n as f64) * 0.55 * 255.0) as u8;
        let thick = 1.0 + i as f64 * 0.2;
        draw_line_alpha(buf, w, h, ax, ay, bx, by, thick, op, 0xFF, 0x35, 0x35);
    }
}

fn draw_line_alpha(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    thick: f64,
    a: u8,
    r: u8,
    g: u8,
    b: u8,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        return;
    }
    let steps = (len * 1.5).ceil() as i32;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x0 + dx * t;
        let y = y0 + dy * t;
        fill_ellipse_alpha(buf, w, h, x, y, thick, thick, a, r, g, b);
    }
}

fn fill_ellipse_alpha(
    buf: &mut [u32],
    w: u32,
    h: u32,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    a: u8,
    r: u8,
    g: u8,
    b: u8,
) {
    let x0 = (cx - rx).floor().max(0.0) as i32;
    let y0 = (cy - ry).floor().max(0.0) as i32;
    let x1 = (cx + rx).ceil().min(w as f64 - 1.0) as i32;
    let y1 = (cy + ry).ceil().min(h as f64 - 1.0) as i32;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let c = pack(a, r, g, b);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if (dx * dx) / rx2 + (dy * dy) / ry2 <= 1.0 {
                // simple overwrite (trail is drawn before opaque sprites)
                put(buf, w, h, x as u32, y as u32, c);
            }
        }
    }
}

fn draw_bird(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, vx: f64) {
    let dir = if vx >= 0.0 { 1.0 } else { -1.0 };
    fill_ellipse(buf, w, h, x, y, 10.0, 7.0, 0x5A, 0x8A, 0xC8);
    fill_ellipse(buf, w, h, x + dir * 8.0, y - 1.0, 5.0, 4.5, 0x5A, 0x8A, 0xC8);
    fill_triangle(
        buf,
        w,
        h,
        x + dir * 12.0,
        y,
        x + dir * 18.0,
        y - 2.0,
        x + dir * 18.0,
        y + 2.0,
        0xE8,
        0xA0,
        0x40,
    );
    fill_ellipse(buf, w, h, x - dir * 4.0, y - 6.0, 7.0, 3.0, 0x4A, 0x70, 0xA8);
}

fn draw_butterfly(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, age: f64) {
    let flap = (age * 18.0).sin().abs();
    let wing = 6.0 + flap * 5.0;
    fill_ellipse(buf, w, h, x - wing * 0.7, y, wing, wing * 0.7, 0xC8, 0x7A, 0xE0);
    fill_ellipse(buf, w, h, x + wing * 0.7, y, wing, wing * 0.7, 0xC8, 0x7A, 0xE0);
    fill_ellipse(buf, w, h, x, y, 2.5, 5.0, 0x3A, 0x2A, 0x40);
}






fn draw_bed(buf: &mut [u32], w: u32, h: u32, cx: f64, cy: f64) {
    fill_ellipse(buf, w, h, cx, cy + 6.0, 58.0, 16.0, 0x6B, 0x4E, 0x3A);
    fill_ellipse(buf, w, h, cx, cy, 48.0, 12.0, 0xC4, 0xA4, 0x84);
    fill_ellipse(buf, w, h, cx - 6.0, cy - 2.0, 18.0, 4.0, 0xE8, 0xD4, 0xB8);
}

fn ear(buf: &mut [u32], w: u32, h: u32, tip_x: f64, tip_y: f64, side: f64, pet: &Pet) {
    let (r, g, b) = fur(pet);
    match pet.species {
        Species::Cat => {
            let base_l = tip_x - 8.0;
            let base_r = tip_x + 8.0;
            let base_y = tip_y + 18.0;
            let tip = tip_x + side * 2.0;
            fill_triangle(buf, w, h, tip, tip_y, base_l, base_y, base_r, base_y, r, g, b);
        }
        Species::Pig => {
            // drooping floppy ears
            fill_ellipse(
                buf,
                w,
                h,
                tip_x + side * 4.0,
                tip_y + 10.0,
                9.0,
                12.0,
                r,
                g,
                b,
            );
            fill_ellipse(
                buf,
                w,
                h,
                tip_x + side * 6.0,
                tip_y + 16.0,
                6.0,
                8.0,
                (r as u16 * 9 / 10) as u8,
                (g as u16 * 9 / 10) as u8,
                (b as u16 * 9 / 10) as u8,
            );
        }
        Species::Bear => {
            // round ears
            fill_ellipse(buf, w, h, tip_x, tip_y + 6.0, 9.0, 9.0, r, g, b);
            fill_ellipse(
                buf,
                w,
                h,
                tip_x,
                tip_y + 7.0,
                4.5,
                4.5,
                (r as u16 * 7 / 10) as u8,
                (g as u16 * 7 / 10) as u8,
                (b as u16 * 7 / 10) as u8,
            );
        }
    }
}

fn draw_face(buf: &mut [u32], w: u32, h: u32, hx: f64, hy: f64, pet: &Pet) {
    match pet.species {
        Species::Cat => {
            // tiny nose already covered by eye dots in most poses
        }
        Species::Pig => {
            // big snout
            fill_ellipse(buf, w, h, hx + pet.facing * 10.0, hy + 6.0, 10.0, 7.0, 0xE8, 0x7A, 0x96);
            fill_ellipse(
                buf,
                w,
                h,
                hx + pet.facing * 7.0,
                hy + 6.0,
                1.8,
                2.4,
                0x6A,
                0x30,
                0x40,
            );
            fill_ellipse(
                buf,
                w,
                h,
                hx + pet.facing * 13.0,
                hy + 6.0,
                1.8,
                2.4,
                0x6A,
                0x30,
                0x40,
            );
        }
        Species::Bear => {
            // big black nose + muzzle
            let (br, bg, bb) = belly(pet);
            fill_ellipse(
                buf,
                w,
                h,
                hx + pet.facing * 8.0,
                hy + 6.0,
                9.0,
                7.0,
                br,
                bg,
                bb,
            );
            fill_ellipse(
                buf,
                w,
                h,
                hx + pet.facing * 12.0,
                hy + 4.0,
                4.5,
                3.5,
                0x1A,
                0x12,
                0x10,
            );
        }
    }
}

fn draw_tail(buf: &mut [u32], w: u32, h: u32, tx: f64, ty: f64, pet: &Pet) {
    let (r, g, b) = fur(pet);
    match pet.species {
        Species::Cat => {
            fill_ellipse(buf, w, h, tx, ty, 14.0, 6.0, r, g, b);
        }
        Species::Pig => {
            // curly tail: small spiral of blobs
            for i in 0..5 {
                let a = i as f64 * 0.9 + pet.walk_phase * 0.3;
                let rad = 4.0 + i as f64 * 1.2;
                fill_ellipse(
                    buf,
                    w,
                    h,
                    tx + a.cos() * rad,
                    ty + a.sin() * rad,
                    2.4,
                    2.4,
                    r,
                    g,
                    b,
                );
            }
        }
        Species::Bear => {
            fill_ellipse(buf, w, h, tx, ty, 6.0, 6.0, r, g, b);
        }
    }
}

fn draw_z(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, a: u8) {
    fill_rect_alpha(buf, w, h, x, y, 10.0, 2.0, a, 0x55, 0x44, 0x66);
    fill_rect_alpha(buf, w, h, x + 2.0, y + 5.0, 10.0, 2.0, a, 0x55, 0x44, 0x66);
    for i in 0..8 {
        fill_rect_alpha(
            buf,
            w,
            h,
            x + 8.0 - i as f64,
            y + 1.0 + i as f64 * 0.6,
            2.0,
            2.0,
            a,
            0x55,
            0x44,
            0x66,
        );
    }
}

fn draw_speech_bubble(
    buf: &mut [u32],
    w: u32,
    h: u32,
    cx: f64,
    cy: f64,
    bubble: &crate::pet::SpeechBubble,
) {
    const PX: f32 = 13.0;
    let (tw, th) = text::measure(&bubble.text, PX);
    let pad_x = 12.0;
    let pad_y = 7.0;
    let scale = bubble.pop_scale();
    let bw = (tw as f64 + pad_x * 2.0) * scale;
    let bh = (th as f64 + pad_y * 2.0).max(22.0) * scale;
    let x0 = cx - bw * 0.5;
    let y0 = cy - bh;
    // White capsule + soft shadow tint.
    fill_round_rect(buf, w, h, x0 + 1.0, y0 + 2.0, bw, bh, 11.0 * scale, 0xE8, 0xE0, 0xD8, 90);
    fill_round_rect(buf, w, h, x0, y0, bw, bh, 11.0 * scale, 0xFF, 0xFF, 0xFF, 245);
    // Tail triangle pointing down at pet.
    let tx = cx;
    let ty = y0 + bh;
    fill_triangle_alpha(
        buf,
        w,
        h,
        tx - 5.0 * scale,
        ty,
        tx + 5.0 * scale,
        ty,
        tx,
        ty + 7.0 * scale,
        245,
        0xFF,
        0xFF,
        0xFF,
    );
    let text_x = x0 + pad_x * scale;
    let text_y = y0 + pad_y * scale * 0.6;
    text::blit_text(
        buf,
        w,
        h,
        text_x,
        text_y,
        &bubble.text,
        PX * scale as f32,
        0x3A,
        0x2A,
        0x20,
        1.0,
    );
}

fn draw_particle(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x: f64,
    y: f64,
    p: &crate::pet::Particle,
) {
    let a = (p.alpha() * 255.0) as u8;
    if a < 8 {
        return;
    }
    match p.kind {
        ParticleKind::Heart | ParticleKind::Kiss => {
            let label = if p.kind == ParticleKind::Kiss {
                "💋"
            } else {
                "❤"
            };
            text::blit_text(buf, w, h, x - 6.0, y - 8.0, label, 14.0, 0xE0, 0x40, 0x70, p.alpha() as f32);
        }
        ParticleKind::Zzz => {
            text::blit_text(buf, w, h, x, y, "Z", 12.0, 0x55, 0x44, 0x66, p.alpha() as f32);
        }
        ParticleKind::Dust => {
            fill_ellipse_alpha(buf, w, h, x, y, 4.0 + p.t() * 6.0, 3.0 + p.t() * 4.0, a / 2, 0xC0, 0xB0, 0xA0);
        }
        ParticleKind::Footprint => {
            fill_ellipse_alpha(buf, w, h, x, y, 4.0, 2.5, a / 3, 0x80, 0x70, 0x60);
            fill_ellipse_alpha(buf, w, h, x + 5.0, y + 1.0, 3.5, 2.2, a / 3, 0x80, 0x70, 0x60);
        }
        ParticleKind::Mud => {
            fill_ellipse_alpha(buf, w, h, x, y, 3.5, 3.0, a, 0x6B, 0x4E, 0x31);
        }
        ParticleKind::Dream => {
            let label = p.label.unwrap_or("💤");
            fill_round_rect(buf, w, h, x - 12.0, y - 10.0, 28.0, 22.0, 8.0, 0xFF, 0xFF, 0xFF, a);
            text::blit_text(buf, w, h, x - 8.0, y - 8.0, label, 14.0, 0x3A, 0x2A, 0x20, p.alpha() as f32);
        }
    }
}

fn fill_round_rect(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x: f64,
    y: f64,
    rw: f64,
    rh: f64,
    radius: f64,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    if rw <= 0.0 || rh <= 0.0 || a == 0 {
        return;
    }
    let rad = radius.min(rw * 0.5).min(rh * 0.5).max(0.0);
    let x0 = (x - 1.0).floor().max(0.0) as i32;
    let y0 = (y - 1.0).floor().max(0.0) as i32;
    let x1 = (x + rw + 1.0).ceil().min(w as f64 - 1.0) as i32;
    let y1 = (y + rh + 1.0).ceil().min(h as f64 - 1.0) as i32;
    let cx = x + rw * 0.5;
    let cy = y + rh * 0.5;
    let half_w = rw * 0.5;
    let half_h = rh * 0.5;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let sd = sd_rounded_box(
                px as f64 + 0.5 - cx,
                py as f64 + 0.5 - cy,
                half_w,
                half_h,
                rad,
            );
            let cov = (0.5 - sd).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let aa = ((a as f64) * cov).round().clamp(0.0, 255.0) as u8;
            if aa > 0 {
                blend_over(buf, w, h, px as u32, py as u32, aa, r, g, b);
            }
        }
    }
}

/// Signed distance to a rounded box centered at origin (Inigo Quilez).
fn sd_rounded_box(px: f64, py: f64, half_w: f64, half_h: f64, rad: f64) -> f64 {
    let bx = (half_w - rad).max(0.0);
    let by = (half_h - rad).max(0.0);
    let qx = px.abs() - bx;
    let qy = py.abs() - by;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    // Must be min(max(qx,qy), 0) — min(min(...), 0) inflates the box badly.
    ox.hypot(oy) + qx.max(qy).min(0.0) - rad
}

/// Approximate signed distance to a triangle (negative inside).
fn sd_triangle(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let d0 = sd_segment(px, py, x0, y0, x1, y1);
    let d1 = sd_segment(px, py, x1, y1, x2, y2);
    let d2 = sd_segment(px, py, x2, y2, x0, y0);
    let d = d0.min(d1).min(d2);
    if point_in_tri(px, py, x0, y0, x1, y1, x2, y2) {
        -d
    } else {
        d
    }
}

fn sd_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let denom = bax * bax + bay * bay;
    let h = if denom <= 1e-12 {
        0.0
    } else {
        ((pax * bax + pay * bay) / denom).clamp(0.0, 1.0)
    };
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    dx.hypot(dy)
}

fn blend_over(buf: &mut [u32], w: u32, h: u32, x: u32, y: u32, a: u8, r: u8, g: u8, b: u8) {
    if x >= w || y >= h || a == 0 {
        return;
    }
    let i = (y * w + x) as usize;
    if a == 255 {
        buf[i] = pack(255, r, g, b);
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

fn fill_ellipse(
    buf: &mut [u32],
    w: u32,
    h: u32,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    r: u8,
    g: u8,
    b: u8,
) {
    let x0 = (cx - rx).floor().max(0.0) as i32;
    let y0 = (cy - ry).floor().max(0.0) as i32;
    let x1 = (cx + rx).ceil().min(w as f64 - 1.0) as i32;
    let y1 = (cy + ry).ceil().min(h as f64 - 1.0) as i32;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if (dx * dx) / rx2 + (dy * dy) / ry2 <= 1.0 {
                put(buf, w, h, x as u32, y as u32, pack(255, r, g, b));
            }
        }
    }
}

fn fill_triangle(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    r: u8,
    g: u8,
    b: u8,
) {
    let min_x = x0.min(x1).min(x2).floor().max(0.0) as i32;
    let max_x = x0.max(x1).max(x2).ceil().min(w as f64 - 1.0) as i32;
    let min_y = y0.min(y1).min(y2).floor().max(0.0) as i32;
    let max_y = y0.max(y1).max(y2).ceil().min(h as f64 - 1.0) as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            if point_in_tri(px, py, x0, y0, x1, y1, x2, y2) {
                put(buf, w, h, x as u32, y as u32, pack(255, r, g, b));
            }
        }
    }
}

fn point_in_tri(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    let d =
        |ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64| (cx - ax) * (by - ay) - (bx - ax) * (cy - ay);
    let b0 = d(x0, y0, x1, y1, px, py) >= 0.0;
    let b1 = d(x1, y1, x2, y2, px, py) >= 0.0;
    let b2 = d(x2, y2, x0, y0, px, py) >= 0.0;
    b0 == b1 && b1 == b2
}

fn fill_rect(buf: &mut [u32], w: u32, h: u32, x: f64, y: f64, rw: f64, rh: f64, r: u8, g: u8, b: u8) {
    fill_rect_alpha(buf, w, h, x, y, rw, rh, 255, r, g, b);
}

fn fill_rect_alpha(
    buf: &mut [u32],
    w: u32,
    h: u32,
    x: f64,
    y: f64,
    rw: f64,
    rh: f64,
    a: u8,
    r: u8,
    g: u8,
    b: u8,
) {
    let x0 = x.floor().max(0.0) as i32;
    let y0 = y.floor().max(0.0) as i32;
    let x1 = (x + rw).ceil().min(w as f64 - 1.0) as i32;
    let y1 = (y + rh).ceil().min(h as f64 - 1.0) as i32;
    let c = pack(a, r, g, b);
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            put(buf, w, h, xx as u32, yy as u32, c);
        }
    }
}

fn put(buf: &mut [u32], w: u32, h: u32, x: u32, y: u32, c: u32) {
    if x < w && y < h {
        buf[(y * w + x) as usize] = c;
    }
}

pub fn hit_pet(buf: &[u32], w: u32, h: u32, lx: f64, ly: f64) -> bool {
    hit_pet_padded(buf, w, h, lx, ly, 0)
}

/// Alpha hit-test with optional padding (passthrough uses a small pad).
pub fn hit_pet_padded(buf: &[u32], w: u32, h: u32, lx: f64, ly: f64, pad: i32) -> bool {
    let need = (w as usize).saturating_mul(h as usize);
    if w == 0 || h == 0 || buf.len() < need {
        return false;
    }
    let x = lx.floor() as i32;
    let y = ly.floor() as i32;
    let wi = w as i32;
    let hi = h as i32;
    let pad = pad.max(0);
    for dy in -pad..=pad {
        for dx in -pad..=pad {
            let xx = x + dx;
            let yy = y + dy;
            if xx < 0 || yy < 0 || xx >= wi || yy >= hi {
                continue;
            }
            let c = buf[(yy as u32 * w + xx as u32) as usize];
            if ((c >> 24) & 0xFF) > 16 {
                return true;
            }
        }
    }
    false
}
