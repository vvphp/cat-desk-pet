//! 摸鱼猫 — native small-window pet. No WebView.
//!
//! From repo root:
//!   npm run dev
//!   npm run build
//! Or:
//!   cargo run --release

mod pet;
mod render;
mod sprite;
mod text;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "macos"))]
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pet::{CoatColor, ForceScene, Mode, Pet, Species, ToyKind};
use render::WIN;
use sprite::SpriteCache;
#[cfg(not(target_os = "macos"))]
use softbuffer::{Context, Surface};
use tray_icon::menu::{
    ContextMenu, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId, WindowLevel};

#[derive(Debug)]
enum UserEvent {
    MenuEvent(tray_icon::menu::MenuEvent),
}

#[derive(Debug, Clone, Copy)]
enum MenuCommand {
    Quit,
    Toggle,
    Sleep,
    Bed,
    Feed,
    Toy(ToyKind),
    CancelToy,
    Bird,
    Butterfly,
    Photo,
    Gift,
    Clingy,
    Coat(CoatColor),
    Species(Species),
}

/// Left-button press before we know drag vs pet (mirrors WebView `state.press`).
struct PressState {
    t0: Instant,
    /// Desktop logical coords at press.
    desk_x: f64,
    desk_y: f64,
    dragging: bool,
    petting: bool,
}

struct App {
    window: Option<Rc<Window>>,
    /// Monitor-sized white overlay for photo flash (WebView `#flash`).
    #[cfg(target_os = "macos")]
    flash_window: Option<Rc<Window>>,
    #[cfg(not(target_os = "macos"))]
    context: Option<Context<Rc<Window>>>,
    #[cfg(not(target_os = "macos"))]
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pet: Pet,
    sprites: SpriteCache,
    /// Logical canvas (straight ARGB).
    pixels: Vec<u32>,
    /// Physical buffer for Retina upscale / present (non-macOS softbuffer path).
    #[cfg(not(target_os = "macos"))]
    present_buf: Vec<u32>,
    last_tick: Instant,
    next_frame: Instant,
    _tray: Option<tray_icon::TrayIcon>,
    /// Shared tray + right-click menu (Clone / Rc internally).
    ctx_menu: Option<Menu>,
    _submenus: Option<Vec<Submenu>>,
    _menu_items: Option<Vec<MenuItem>>,
    menu_cmds: Vec<(tray_icon::menu::MenuId, MenuCommand)>,
    hidden: bool,
    cursor_local: Option<(f64, f64)>,
    ignore_mouse: bool,
    drag_grab: Option<(f64, f64)>, // offset from pet center while dragging
    press: Option<PressState>,
    scale: f64,
    /// Committed OS window top-left (logical). May lag `pet` while walking;
    /// drawing uses this origin so the sprite stays on-screen.
    last_win_pos: Option<(f64, f64)>,
    /// Committed logical canvas size (grows to fit toys / flyers).
    view_w: u32,
    view_h: u32,
    last_win_move: Instant,
    last_passthrough: Instant,
}

/// How far (logical px) the pet may drift inside the window before we move the OS window.
const WIN_MOVE_THRESHOLD: f64 = 6.0;
/// Cap OS window moves even while walking (~12 Hz).
const WIN_MOVE_MIN_INTERVAL: Duration = Duration::from_millis(80);
/// Ignore sub-threshold view size jitter from ceil/round while props move.
const VIEW_SIZE_THRESHOLD: u32 = 8;
/// Shared alpha hit pad for passthrough capture and click/context menu.
const PET_HIT_PAD: i32 = 4;
/// Leave-capture ellipse scale (enter uses 1.0). Stops bob/orbit from
/// flipping `ignoresMouseEvents` every frame under the cursor.
const PET_HIT_LEAVE_SCALE: f64 = 1.4;

impl App {
    fn new(screen_w: f64, screen_h: f64) -> Self {
        let now = Instant::now();
        Self {
            window: None,
            #[cfg(target_os = "macos")]
            flash_window: None,
            #[cfg(not(target_os = "macos"))]
            context: None,
            #[cfg(not(target_os = "macos"))]
            surface: None,
            pet: Pet::new(screen_w, screen_h),
            sprites: SpriteCache::new(),
            pixels: vec![0; (WIN * WIN) as usize],
            #[cfg(not(target_os = "macos"))]
            present_buf: Vec::new(),
            last_tick: now,
            next_frame: now,
            _tray: None,
            ctx_menu: None,
            _submenus: None,
            _menu_items: None,
            menu_cmds: Vec::new(),
            hidden: false,
            cursor_local: None,
            ignore_mouse: true,
            drag_grab: None,
            press: None,
            scale: 1.0,
            last_win_pos: None,
            view_w: WIN,
            view_h: WIN,
            last_win_move: now,
            last_passthrough: now,
        }
    }

    /// Window top-left + logical size covering pet and world props.
    fn desired_view(&self) -> (f64, f64, u32, u32) {
        let (x0, y0, x1, y1) = self.pet.visible_bounds();
        let w = ((x1 - x0).ceil() as u32).max(WIN);
        let h = ((y1 - y0).ceil() as u32).max(WIN);
        (x0.round(), y0.round(), w, h)
    }

    /// Commit OS window size/position; canvas grows for toys / flyers.
    /// Returns true when size or origin changed (caller should present immediately).
    fn sync_window_pos(&mut self, force: bool) -> bool {
        let Some(window) = &self.window else { return false };
        let (lx, ly, lw, lh) = self.desired_view();
        let now = Instant::now();

        // Grow immediately so near props aren't clipped. Shrink as soon as the
        // desired size drops by ≥ threshold (props left) — don't keep a huge
        // canvas around after a flyer/laser pass. Tiny ±ceil jitter is ignored.
        let grow = lw > self.view_w || lh > self.view_h;
        let shrink_enough = (lw < self.view_w || lh < self.view_h)
            && (self.view_w.abs_diff(lw) >= VIEW_SIZE_THRESHOLD
                || self.view_h.abs_diff(lh) >= VIEW_SIZE_THRESHOLD);
        let size_changed = force || grow || shrink_enough;
        let should_move = force
            || size_changed
            || match self.last_win_pos {
                None => true,
                Some((ox, oy)) => {
                    let far = (ox - lx).abs() >= WIN_MOVE_THRESHOLD
                        || (oy - ly).abs() >= WIN_MOVE_THRESHOLD;
                    let due = now.duration_since(self.last_win_move) >= WIN_MOVE_MIN_INTERVAL;
                    let very_far = (ox - lx).abs() >= WIN_MOVE_THRESHOLD * 2.0
                        || (oy - ly).abs() >= WIN_MOVE_THRESHOLD * 2.0;
                    (far && due) || very_far || self.pet.dragging
                }
            };

        if size_changed && (self.view_w != lw || self.view_h != lh) {
            let _ = window.request_inner_size(LogicalSize::new(lw, lh));
            self.view_w = lw;
            self.view_h = lh;
            // Keep hit/draw buffer in sync before the next redraw (passthrough
            // may run in the same tick).
            let need = (lw as usize).saturating_mul(lh as usize);
            self.pixels.resize(need, 0);
        }

        if should_move {
            let _ = window.set_outer_position(LogicalPosition::new(lx, ly));
            self.last_win_pos = Some((lx, ly));
            self.last_win_move = now;
        }
        size_changed || should_move
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };

        // Paint in logical view space (capped; see `Pet::visible_bounds`).
        // Use committed view size (not lagging inner_size) so a pending
        // request_inner_size can't present one wrong frame while the OS catches up.
        let lw = self.view_w.max(1);
        let lh = self.view_h.max(1);
        let need = (lw as usize).saturating_mul(lh as usize);
        if self.pixels.len() != need {
            self.pixels.resize(need, 0);
        }

        let (ox, oy) = self.last_win_pos.unwrap_or_else(|| {
            let (x, y, _, _) = self.desired_view();
            (x, y)
        });
        render::draw_pet(
            &mut self.pixels,
            lw,
            lh,
            &self.pet,
            ox,
            oy,
            &mut self.sprites,
        );

        #[cfg(target_os = "macos")]
        {
            // Present logical pixels; CALayer `contentsScale` upscales on Retina.
            // Avoids a full physical present_buf (+premuls) every frame.
            macos::present_argb(&window, &self.pixels, lw, lh);
        }

        #[cfg(not(target_os = "macos"))]
        {
            let scale = window.scale_factor().max(0.01);
            let pw = ((lw as f64) * scale).round().max(1.0) as u32;
            let ph = ((lh as f64) * scale).round().max(1.0) as u32;
            let phys = (pw as usize).saturating_mul(ph as usize);
            if self.present_buf.len() != phys {
                self.present_buf.resize(phys, 0);
            }
            if pw != lw || ph != lh {
                blit_nn(&self.pixels, lw, lh, &mut self.present_buf, pw, ph);
            } else {
                self.present_buf.copy_from_slice(&self.pixels[..need]);
            }
            let Some(surface) = &mut self.surface else {
                return;
            };
            if let (Some(nw), Some(nh)) = (NonZeroU32::new(pw), NonZeroU32::new(ph)) {
                let _ = surface.resize(nw, nh);
            }
            if let Ok(mut buffer) = surface.buffer_mut() {
                let n = buffer.len().min(self.present_buf.len());
                buffer[..n].copy_from_slice(&self.present_buf[..n]);
                let _ = buffer.present();
            }
        }
    }

    fn cursor_logical_in_window(&self) -> Option<(f64, f64)> {
        let (px, py) = self.cursor_local?;
        let scale = self.scale.max(0.01);
        Some((px / scale, py / scale))
    }

    fn passthrough_interval(&self) -> Duration {
        match self.pet.mode {
            Mode::Sleeping | Mode::InBed => Duration::from_millis(100),
            Mode::Idle | Mode::Dizzy | Mode::Pet | Mode::Watching | Mode::BirdWatch => {
                Duration::from_millis(66)
            }
            Mode::Walking
            | Mode::GoingHome
            | Mode::Clingy
            | Mode::Interested
            | Mode::Feeding
            | Mode::ButterflyNose
            | Mode::Photo
            | Mode::Gifting => Duration::from_millis(50),
            Mode::Dragged | Mode::Chasing | Mode::Playing | Mode::Startled | Mode::Trick => {
                Duration::from_millis(33)
            }
        }
    }

    fn desk_from_cursor_local(&self, lx: f64, ly: f64) -> Option<(f64, f64)> {
        let window = self.window.as_ref()?;
        let outer = window
            .outer_position()
            .unwrap_or(PhysicalPosition::new(0, 0));
        let scale = window.scale_factor().max(0.01);
        Some((outer.x as f64 / scale + lx, outer.y as f64 / scale + ly))
    }

    /// Promote long-press → pet, or move → drag (WebView press semantics).
    fn tick_press(&mut self) {
        let Some(press) = self.press.as_mut() else {
            return;
        };
        if press.dragging || press.petting {
            return;
        }
        if press.t0.elapsed() >= Duration::from_millis(500) {
            press.petting = true;
            self.pet.start_pet();
            self.next_frame = Instant::now();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn feed_cursor(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.pet.note_cursor(macos::cursor_logical_top_left());
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.pet.note_cursor(None);
        }
    }

    fn update_passthrough(&mut self) {
        #[cfg(target_os = "macos")]
        {
            let now = Instant::now();
            let holding = self.pet.dragging || self.press.is_some();
            if !holding && now.duration_since(self.last_passthrough) < self.passthrough_interval()
            {
                return;
            }
            self.last_passthrough = now;

            let Some(window) = &self.window else { return };
            // While holding, keep capture so drag/release stay on this window.
            if holding {
                if self.ignore_mouse {
                    self.ignore_mouse = false;
                    macos::set_ignore_mouse(window, false);
                }
                return;
            }
            // Large drawable stays click-through except over the pet body ellipse
            // — so toys/flyers/transparent strips never block the desktop, but a
            // click on the cat is swallowed by us (not Finder underneath).
            // Hysteresis: enter on the tight ellipse, leave only outside a fatter
            // one — otherwise walk bob / Interested orbit flips ignore every tick
            // and the window/sprite looks like it's shaking under the cursor.
            let scale = if self.ignore_mouse {
                1.0
            } else {
                PET_HIT_LEAVE_SCALE
            };
            let over = macos::cursor_logical_top_left()
                .map(|(cx, cy)| self.hits_pet_body_scaled(cx, cy, scale))
                .unwrap_or(false);
            let want_ignore = !over;
            if want_ignore != self.ignore_mouse {
                self.ignore_mouse = want_ignore;
                macos::set_ignore_mouse(window, want_ignore);
            }
        }
    }

    /// Tight body hit in desktop logical coords (props / transparent canvas ignored).
    fn hits_pet_body(&self, desk_x: f64, desk_y: f64) -> bool {
        self.hits_pet_body_scaled(desk_x, desk_y, 1.0)
    }

    fn hits_pet_body_scaled(&self, desk_x: f64, desk_y: f64, scale: f64) -> bool {
        let dx = desk_x - self.pet.x;
        let dy = desk_y - self.pet.y;
        // Ellipse ~ WebView 120×110 body (was sized for the old 160² sprite).
        let rx = (39.0 + PET_HIT_PAD as f64) * scale;
        let ry = (33.0 + PET_HIT_PAD as f64) * scale;
        (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0
    }

    fn hits_pet_local(&self, lx: f64, ly: f64) -> bool {
        let (wx, wy) = self.window_origin();
        self.hits_pet_body(wx + lx, wy + ly)
    }

    fn window_origin(&self) -> (f64, f64) {
        match self.last_win_pos {
            Some(p) => p,
            None => {
                let (x, y, _, _) = self.desired_view();
                (x, y)
            }
        }
    }

    fn schedule_wake(&self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        // Hover/press: wake often enough to toggle ignore before the click.
        let input_iv = if self.press.is_some() || self.pet.dragging {
            Duration::from_millis(16)
        } else {
            self.passthrough_interval()
        };
        let poll_at = self.last_passthrough + input_iv;
        let wake_at = if self.next_frame < poll_at {
            self.next_frame
        } else {
            poll_at
        };
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at.max(now)));
    }

    /// Monitor containing desktop-logical `(x, y)`, else primary.
    fn monitor_at(event_loop: &ActiveEventLoop, x: f64, y: f64) -> Option<winit::monitor::MonitorHandle> {
        for m in event_loop.available_monitors() {
            let scale = m.scale_factor().max(0.01);
            let pos = m.position();
            let size = m.size();
            let x0 = pos.x as f64 / scale;
            let y0 = pos.y as f64 / scale;
            let x1 = x0 + size.width as f64 / scale;
            let y1 = y0 + size.height as f64 / scale;
            if x >= x0 && x < x1 && y >= y0 && y < y1 {
                return Some(m);
            }
        }
        event_loop.primary_monitor()
    }

    /// WebView `#flash`: full-monitor white overlay. Pet window alone is only ~180².
    #[cfg(target_os = "macos")]
    fn sync_flash_overlay(&mut self, event_loop: &ActiveEventLoop) {
        const FLASH_EPS: f64 = 0.02;
        let intensity = self.pet.flash;
        if intensity < FLASH_EPS {
            if let Some(w) = &self.flash_window {
                macos::set_window_alpha(w, 0.0);
                w.set_visible(false);
            }
            return;
        }

        let Some(monitor) = Self::monitor_at(event_loop, self.pet.x, self.pet.y)
            .or_else(|| event_loop.available_monitors().next())
        else {
            return;
        };
        let size = monitor.size();
        let pos = monitor.position();

        if self.flash_window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("摸鱼猫闪光")
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(false)
                .with_inner_size(PhysicalSize::new(size.width.max(1), size.height.max(1)))
                .with_position(PhysicalPosition::new(pos.x, pos.y));
            match event_loop.create_window(attrs) {
                Ok(w) => {
                    let w = Rc::new(w);
                    macos::configure_flash_overlay(&w);
                    self.flash_window = Some(w);
                }
                Err(_) => return,
            }
        }

        let Some(w) = &self.flash_window else {
            return;
        };
        let _ = w.request_inner_size(PhysicalSize::new(size.width.max(1), size.height.max(1)));
        let _ = w.set_outer_position(PhysicalPosition::new(pos.x, pos.y));
        // Match WebView peak (~0.95).
        let alpha = (intensity * 0.95).clamp(0.0, 0.95);
        w.set_visible(true);
        macos::set_window_alpha(w, alpha);
        macos::order_front(w);
    }

    #[cfg(not(target_os = "macos"))]
    fn sync_flash_overlay(&mut self, _event_loop: &ActiveEventLoop) {}
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            #[cfg(target_os = "macos")]
            macos::set_accessory_policy();

            // Tray + right-click menu aligned with WebView ctx-menu.
            let menu = Menu::new();
            let mut items: Vec<MenuItem> = Vec::new();
            let mut submenus: Vec<Submenu> = Vec::new();
            let mut menu_cmds: Vec<(tray_icon::menu::MenuId, MenuCommand)> = Vec::new();
            let mut bind = |item: &MenuItem, cmd: MenuCommand| {
                menu_cmds.push((item.id().clone(), cmd));
            };

            // --- 🐾 动物 ---
            let mut species_items: Vec<MenuItem> = Vec::new();
            for sp in Species::all() {
                let item = MenuItem::new(sp.label(), true, None);
                bind(&item, MenuCommand::Species(*sp));
                species_items.push(item);
            }
            let species_refs: Vec<&dyn IsMenuItem> = species_items
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .collect();
            let species_menu =
                Submenu::with_items("🐾 动物", true, &species_refs).expect("species submenu");

            // --- 🎨 毛色 ---
            let mut color_items: Vec<MenuItem> = Vec::new();
            for coat in CoatColor::all() {
                let item = MenuItem::new(coat.label(), true, None);
                bind(&item, MenuCommand::Coat(*coat));
                color_items.push(item);
            }
            let color_refs: Vec<&dyn IsMenuItem> =
                color_items.iter().map(|i| i as &dyn IsMenuItem).collect();
            let color_menu =
                Submenu::with_items("🎨 毛色", true, &color_refs).expect("color submenu");

            // --- 🎮 互动 ---
            let feed = MenuItem::new("🍴 投食", true, None);
            let sleep = MenuItem::new("💤 让她睡一下", true, None);
            let bed = MenuItem::new("🏠 回窝睡觉", true, None);
            let photo = MenuItem::new("📸 拍照模式", true, None);
            bind(&feed, MenuCommand::Feed);
            bind(&sleep, MenuCommand::Sleep);
            bind(&bed, MenuCommand::Bed);
            bind(&photo, MenuCommand::Photo);
            let interact_menu = Submenu::with_items(
                "🎮 互动",
                true,
                &[
                    &feed as &dyn IsMenuItem,
                    &sleep as &dyn IsMenuItem,
                    &bed as &dyn IsMenuItem,
                    &photo as &dyn IsMenuItem,
                ],
            )
            .expect("interact submenu");

            // --- 🧸 玩具 ---
            let yarn = MenuItem::new("🧶 毛线球", true, None);
            let ball = MenuItem::new("⚽ 弹力球", true, None);
            let paper = MenuItem::new("📄 纸团", true, None);
            let mouse = MenuItem::new("🐭 假老鼠", true, None);
            let laser = MenuItem::new("🔴 激光笔", true, None);
            let wand = MenuItem::new("🪶 逗猫棒", true, None);
            let cancel_toy = MenuItem::new("❌ 收起玩具", true, None);
            bind(&yarn, MenuCommand::Toy(ToyKind::Yarn));
            bind(&ball, MenuCommand::Toy(ToyKind::Ball));
            bind(&paper, MenuCommand::Toy(ToyKind::Paper));
            bind(&mouse, MenuCommand::Toy(ToyKind::Mouse));
            bind(&laser, MenuCommand::Toy(ToyKind::Laser));
            bind(&wand, MenuCommand::Toy(ToyKind::Wand));
            bind(&cancel_toy, MenuCommand::CancelToy);
            let toy_menu = Submenu::with_items(
                "🧸 玩具",
                true,
                &[
                    &yarn as &dyn IsMenuItem,
                    &ball as &dyn IsMenuItem,
                    &paper as &dyn IsMenuItem,
                    &mouse as &dyn IsMenuItem,
                    &laser as &dyn IsMenuItem,
                    &wand as &dyn IsMenuItem,
                    &cancel_toy as &dyn IsMenuItem,
                ],
            )
            .expect("toy submenu");

            // --- ✨ 更多 ---
            let toggle = MenuItem::new("隐藏 / 显示宠物", true, None);
            let clingy = MenuItem::new("💕 过来撒娇", true, None);
            let gift = MenuItem::new("🎁 送你礼物", true, None);
            let bird = MenuItem::new("🐦 小鸟飞过", true, None);
            let butterfly = MenuItem::new("🦋 蝴蝶落鼻", true, None);
            bind(&toggle, MenuCommand::Toggle);
            bind(&clingy, MenuCommand::Clingy);
            bind(&gift, MenuCommand::Gift);
            bind(&bird, MenuCommand::Bird);
            bind(&butterfly, MenuCommand::Butterfly);
            let more_menu = Submenu::with_items(
                "✨ 更多",
                true,
                &[
                    &toggle as &dyn IsMenuItem,
                    &clingy as &dyn IsMenuItem,
                    &gift as &dyn IsMenuItem,
                    &bird as &dyn IsMenuItem,
                    &butterfly as &dyn IsMenuItem,
                ],
            )
            .expect("more submenu");

            let quit = MenuItem::new("退出", true, None);
            bind(&quit, MenuCommand::Quit);
            let sep = PredefinedMenuItem::separator();

            let _ = menu.append(&species_menu);
            let _ = menu.append(&color_menu);
            let _ = menu.append(&interact_menu);
            let _ = menu.append(&toy_menu);
            let _ = menu.append(&more_menu);
            let _ = menu.append(&sep);
            let _ = menu.append(&quit);

            items.extend(species_items);
            items.extend(color_items);
            items.push(feed);
            items.push(sleep);
            items.push(bed);
            items.push(photo);
            items.push(yarn);
            items.push(ball);
            items.push(paper);
            items.push(mouse);
            items.push(laser);
            items.push(wand);
            items.push(cancel_toy);
            items.push(toggle);
            items.push(clingy);
            items.push(gift);
            items.push(bird);
            items.push(butterfly);
            items.push(quit);
            submenus.push(species_menu);
            submenus.push(color_menu);
            submenus.push(interact_menu);
            submenus.push(toy_menu);
            submenus.push(more_menu);

            self.menu_cmds = menu_cmds;
            self._menu_items = Some(items);
            self._submenus = Some(submenus);
            self.ctx_menu = Some(menu.clone());

            let icon = tray_icon_from_orange();
            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("摸鱼猫")
                .with_icon(icon)
                .build()
                .ok();
            self._tray = tray;
        }

        let now = Instant::now();
        if now >= self.next_frame {
            let dt = now.duration_since(self.last_tick).as_secs_f64().min(0.1);
            self.last_tick = now;
            self.feed_cursor();
            self.tick_press();
            self.pet.update(dt);
            // Present in the same turn as any window move/resize — async
            // request_redraw leaves one frame of stale layer at the new origin
            // (visible as flicker while walking).
            self.sync_window_pos(false);
            self.update_passthrough();
            self.sync_flash_overlay(event_loop);
            self.redraw();
            self.next_frame = now + self.pet.mode.frame_interval();
        } else {
            // Refresh pet-ellipse ignore toggle between paints (pre-click capture).
            self.update_passthrough();
            if self.pet.flash > 0.02 {
                self.sync_flash_overlay(event_loop);
            }
        }
        self.schedule_wake(event_loop);

        // Drain menu events (also delivered as UserEvent when proxy works).
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            self.handle_menu(&ev, event_loop);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        if let Some(monitor) = event_loop.primary_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            self.pet
                .set_screen(size.width as f64 / scale, size.height as f64 / scale);
        }

        let attrs = Window::default_attributes()
            .with_title("摸鱼猫")
            .with_inner_size(LogicalSize::new(WIN, WIN))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);

        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        self.scale = window.scale_factor();

        #[cfg(target_os = "macos")]
        {
            macos::configure_transparent(&window);
            macos::set_ignore_mouse(&window, true);
            self.ignore_mouse = true;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let context = Context::new(window.clone()).expect("softbuffer context");
            let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");
            self.context = Some(context);
            self.surface = Some(surface);
        }

        self.window = Some(window);
        self.sync_window_pos(true);
        self.redraw();
        // Accessory (no Dock) — force above other windows so the pet is findable.
        #[cfg(target_os = "macos")]
        if let Some(w) = &self.window {
            macos::order_front(w);
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        #[cfg(target_os = "macos")]
        if self.flash_window.as_ref().is_some_and(|w| w.id() == id) {
            // Overlay is click-through; ignore its lifecycle besides suppress close.
            if matches!(event, WindowEvent::CloseRequested) {
                if let Some(w) = &self.flash_window {
                    w.set_visible(false);
                }
            }
            return;
        }

        match event {
            // Tray app: Cmd+W / system close must NOT quit — only hide.
            WindowEvent::CloseRequested => {
                self.set_pet_visible(false);
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                self.update_passthrough();
            }
            // On macOS these fire only while ignore=false (cursor over pet ellipse
            // or during an active press/drag). Transparent expanded areas stay
            // click-through via `update_passthrough`.
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale.max(0.01);
                let lx = position.x / scale;
                let ly = position.y / scale;
                self.cursor_local = Some((position.x, position.y));

                // Promote press → drag if moved > 8 logical px (WebView threshold).
                let desk = self.desk_from_cursor_local(lx, ly);
                let promote = self.press.as_ref().is_some_and(|p| {
                    !p.dragging
                        && !p.petting
                        && desk.is_some_and(|(dx, dy)| {
                            ((dx - p.desk_x).powi(2) + (dy - p.desk_y).powi(2)).sqrt() > 8.0
                        })
                });
                if promote {
                    if let Some(press) = self.press.as_mut() {
                        press.dragging = true;
                    }
                    self.pet.begin_drag();
                    if let Some((dx, dy)) = desk {
                        self.drag_grab = Some((dx - self.pet.x, dy - self.pet.y));
                    }
                }

                // Only drag after promote (>8px). Setting grab on mousedown made
                // sub-threshold jitter call drag_to and rewrite floor_y.
                let dragging = self.press.as_ref().is_some_and(|p| p.dragging);
                if dragging {
                    if let Some((ox, oy)) = self.drag_grab {
                        if let Some(window) = self.window.clone() {
                            let outer = window
                                .outer_position()
                                .unwrap_or(PhysicalPosition::new(0, 0));
                            let scale = window.scale_factor();
                            let desk_x = outer.x as f64 / scale + position.x / scale;
                            let desk_y = outer.y as f64 / scale + position.y / scale;
                            self.pet.drag_to(desk_x - ox, desk_y - oy);
                            self.sync_window_pos(true);
                            self.next_frame = Instant::now();
                            window.request_redraw();
                        }
                    }
                } else {
                    self.update_passthrough();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_local = None;
                self.update_passthrough();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if let Some((lx, ly)) = self.cursor_logical_in_window() {
                    if self.hits_pet_local(lx, ly) {
                        self.show_ctx_menu();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((lx, ly)) = self.cursor_logical_in_window() {
                    if let Some(window) = &self.window {
                        if self.hits_pet_local(lx, ly) {
                            self.pet.on_press();
                            if let Some((dx, dy)) = self.desk_from_cursor_local(lx, ly) {
                                self.press = Some(PressState {
                                    t0: Instant::now(),
                                    desk_x: dx,
                                    desk_y: dy,
                                    dragging: false,
                                    petting: false,
                                });
                                // drag_grab is set only after >8px promote (see CursorMoved).
                            }
                            #[cfg(target_os = "macos")]
                            {
                                // Own this click (hover toggle may have raced).
                                macos::set_ignore_mouse(window, false);
                                self.ignore_mouse = false;
                            }
                            self.next_frame = Instant::now();
                            window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let was_drag = self.press.as_ref().map(|p| p.dragging).unwrap_or(false);
                let was_pet = self.press.as_ref().map(|p| p.petting).unwrap_or(false)
                    || self.pet.mode == Mode::Pet;
                let short_click = self.press.as_ref().is_some_and(|p| {
                    !p.dragging
                        && !p.petting
                        && p.t0.elapsed() < Duration::from_millis(500)
                });
                self.press = None;
                self.drag_grab = None;
                if was_drag || self.pet.dragging {
                    self.pet.end_drag();
                } else if was_pet {
                    self.pet.end_pet();
                } else if short_click {
                    // WebView: short click → mood-weighted trick / double-tap kiss.
                    self.pet.on_short_click();
                }
                self.next_frame = Instant::now();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                // Esc hides (same as tray toggle); quit only via tray「退出」.
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    self.set_pet_visible(false);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor;
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::MenuEvent(ev) => self.handle_menu(&ev, event_loop),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Must match new_events: sleeping frame interval is long; passthrough
        // still needs to wake so click-to-wake stays responsive.
        self.schedule_wake(event_loop);
    }
}

impl App {
    fn handle_menu(&mut self, ev: &MenuEvent, event_loop: &ActiveEventLoop) {
        let Some(cmd) = self
            .menu_cmds
            .iter()
            .find(|(id, _)| id == &ev.id)
            .map(|(_, c)| *c)
        else {
            return;
        };
        match cmd {
            MenuCommand::Quit => {
                event_loop.exit();
            }
            MenuCommand::Toggle => {
                self.set_pet_visible(self.hidden);
            }
            MenuCommand::Species(sp) => {
                self.pet.set_species(sp);
                self.poke();
            }
            MenuCommand::Coat(coat) => {
                self.pet.set_coat(coat);
                self.poke();
            }
            MenuCommand::Feed => {
                self.pet.spawn_feed();
                self.poke();
            }
            MenuCommand::Toy(kind) => {
                self.pet.spawn_toy(kind);
                self.poke();
            }
            MenuCommand::CancelToy => {
                self.pet.cancel_toy();
                self.poke();
            }
            MenuCommand::Bird => {
                self.pet.spawn_bird_flyby();
                self.poke();
            }
            MenuCommand::Butterfly => {
                self.pet.spawn_nose_butterfly();
                self.poke();
            }
            MenuCommand::Photo => {
                self.pet.take_photo();
                self.sync_flash_overlay(event_loop);
                self.poke();
            }
            MenuCommand::Clingy => {
                self.feed_cursor();
                self.pet.start_clingy();
                self.poke();
            }
            MenuCommand::Gift => {
                self.pet.start_gifting();
                self.poke();
            }
            MenuCommand::Sleep => {
                self.pet.force_scene = None;
                self.pet.go_sleep();
                self.poke();
            }
            MenuCommand::Bed => {
                self.pet.force_scene = None;
                self.pet.go_to_bed();
                self.poke();
            }
        }
    }

    fn poke(&mut self) {
        self.next_frame = Instant::now();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Show/hide the pet window without quitting the process (tray stays).
    fn set_pet_visible(&mut self, visible: bool) {
        self.hidden = !visible;
        if let Some(window) = &self.window {
            window.set_visible(visible);
            if visible {
                #[cfg(target_os = "macos")]
                macos::order_front(window);
                window.request_redraw();
                self.next_frame = Instant::now();
            }
        }
    }

    fn show_ctx_menu(&self) {
        let Some(menu) = &self.ctx_menu else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        #[cfg(target_os = "macos")]
        {
            if let Some(view) = macos::ns_view_ptr(window) {
                // SAFETY: view is the live NSView from winit.
                unsafe {
                    menu.show_context_menu_for_nsview(view, None);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (menu, window);
        }
    }
}

/// Nearest-neighbor blit from a logical canvas to a physical softbuffer.
#[cfg(not(target_os = "macos"))]
fn blit_nn(src: &[u32], sw: u32, sh: u32, dst: &mut [u32], dw: u32, dh: u32) {
    let need = (dw * dh) as usize;
    if dst.len() < need || sw == 0 || sh == 0 {
        return;
    }
    if sw == dw && sh == dh {
        dst[..need].copy_from_slice(&src[..need.min(src.len())]);
        return;
    }
    for y in 0..dh {
        let sy = (y as u64 * sh as u64 / dh as u64) as u32;
        let src_row = (sy * sw) as usize;
        let dst_row = (y * dw) as usize;
        for x in 0..dw {
            let sx = (x as u64 * sw as u64 / dw as u64) as u32;
            dst[dst_row + x as usize] = src[src_row + sx as usize];
        }
    }
}

fn tray_icon_from_orange() -> Icon {
    // 32x32 solid orange with simple ears baked in (ARGB).
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let cx = x as f64 - 15.5;
            let cy = y as f64 - 17.0;
            let body = (cx * cx) / (11.0 * 11.0) + (cy * cy) / (9.0 * 9.0) <= 1.0;
            let ear_l = (x as i32 - 8).pow(2) + (y as i32 - 6).pow(2) < 18;
            let ear_r = (x as i32 - 23).pow(2) + (y as i32 - 6).pow(2) < 18;
            let i = ((y * size + x) * 4) as usize;
            if body || ear_l || ear_r {
                rgba[i] = 0xE8;
                rgba[i + 1] = 0x9A;
                rgba[i + 2] = 0x3A;
                rgba[i + 3] = 0xFF;
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("tray icon")
}

fn parse_force_scene() -> Option<ForceScene> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--mode" {
            return parse_force_scene_value(&args.next()?);
        }
        if let Some(v) = a.strip_prefix("--mode=") {
            return parse_force_scene_value(v);
        }
    }
    None
}

fn parse_force_scene_value(v: &str) -> Option<ForceScene> {
    match v.to_ascii_lowercase().as_str() {
        "sleeping" | "sleep" => Some(ForceScene::Sleeping),
        "idle" => Some(ForceScene::Idle),
        "walking" | "walk" => Some(ForceScene::Walking),
        other => {
            eprintln!("unknown --mode {other} (use sleeping|idle|walking)");
            None
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "?".into());
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<Any>".into()
        };
        let line = format!("panic at {loc}: {msg}\n");
        let _ = std::fs::write("/tmp/cat-desk-pet-panic.log", &line);
        eprintln!("{line}");
    }));

    let force = parse_force_scene();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));

    // Rough primary-screen logical size fallback; refined once window exists.
    let (sw, sh) = (1440.0, 900.0);
    let mut app = App::new(sw, sh);
    if let Some(scene) = force {
        app.pet.force_scene = Some(scene);
        app.pet.mode = match scene {
            ForceScene::Walking => Mode::Walking,
            ForceScene::Idle => Mode::Idle,
            ForceScene::Sleeping => Mode::Sleeping,
        };
        eprintln!("cat-desk-pet: force scene = {scene:?}");
    }

    // Seed control flow so WaitUntil fires.
    app.next_frame = Instant::now() + Duration::from_millis(16);

    event_loop.run_app(&mut app).unwrap();
}
