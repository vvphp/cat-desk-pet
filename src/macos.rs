//! macOS helpers: accessory app, transparent window, click-through, cursor poll,
//! and CALayer present with real alpha (softbuffer 0.4 ignores alpha on macOS).

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{self, slice_from_raw_parts_mut, NonNull};

use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSColor, NSEvent, NSScreen, NSView,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGImageComponentInfo, CGImagePixelFormatInfo,
};
use objc2_quartz_core::CATransaction;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

thread_local! {
    /// Recycled premul buffers. Ownership transfers into CGImage; the release
    /// callback returns capacity here so we never free while the layer still
    /// references the pixels (and never leave dangling TLS pointers on exit).
    static PREMUL_FREE: RefCell<Vec<Vec<u32>>> = RefCell::new(Vec::new());
}

pub fn set_accessory_policy() {
    let mtm = MainThreadMarker::new().expect("macOS UI on main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

pub fn configure_transparent(window: &Window) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    ns_window.setOpaque(false);
    ns_window.setHasShadow(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));

    if let Some(ns_view) = ns_view(window) {
        ns_view.setWantsLayer(true);
        if let Some(layer) = ns_view.layer() {
            layer.setOpaque(false);
        }
    }

    // Accessory policy skips Dock; still force the pet above normal windows.
    ns_window.orderFrontRegardless();
}

pub fn order_front(window: &Window) {
    if let Some(ns_window) = ns_window(window) {
        ns_window.orderFrontRegardless();
    }
}

pub fn set_ignore_mouse(window: &Window, ignore: bool) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    ns_window.setIgnoresMouseEvents(ignore);
}

/// Drop CALayer image contents so any borrowed/owned present buffer can release
/// before process teardown (avoids UAF if a provider outlives TLS recycle).
pub fn clear_present(window: &Window) {
    let Some(ns_view) = ns_view(window) else {
        return;
    };
    let Some(layer) = ns_view.layer() else {
        return;
    };
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    unsafe { layer.setContents(Option::<&objc2::runtime::AnyObject>::None) };
    CATransaction::commit();
}

/// Full-screen photo flash overlay: white panel, click-through, alpha-driven.
pub fn configure_flash_overlay(window: &Window) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    ns_window.setOpaque(false);
    ns_window.setHasShadow(false);
    ns_window.setBackgroundColor(Some(&NSColor::whiteColor()));
    ns_window.setIgnoresMouseEvents(true);
    ns_window.setAlphaValue(0.0);
    ns_window.orderFrontRegardless();
}

pub fn set_window_alpha(window: &Window, alpha: f64) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    ns_window.setAlphaValue(alpha.clamp(0.0, 1.0));
}

/// Present straight ARGB `0xAARRGGBB` pixels with real transparency via CALayer.
///
/// Callers should pass a **physical** (Retina) buffer sized `logical × scale_factor`
/// so edges stay sharp. `contentsScale` tells Core Animation the buffer is already
/// in device pixels. View size is capped (`MAX_EDGE`), so peak RSS stays bounded.
///
/// softbuffer 0.4's macOS backend uses `CGImageAlphaInfo::NoneSkipFirst`, which
/// turns clear pixels into an opaque black square — so we present ourselves.
pub fn present_argb(window: &Window, pixels: &[u32], width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    let need = (width as usize).saturating_mul(height as usize);
    if pixels.len() < need {
        return;
    }

    let Some(ns_view) = ns_view(window) else {
        return;
    };
    ns_view.setWantsLayer(true);
    let Some(layer) = ns_view.layer() else {
        return;
    };
    layer.setOpaque(false);
    // Physical buffer + matching scale → 1:1 device pixels (no CA upscale blur).
    layer.setContentsScale(window.scale_factor());

    let mut buf = take_premul_buf(need);
    for (dst, &src) in buf.iter_mut().zip(pixels[..need].iter()) {
        *dst = premultiply_argb(src);
    }

    let Ok(image) = cgimage_from_premul_owned(buf, width as usize, height as usize) else {
        return;
    };

    CATransaction::begin();
    CATransaction::setDisableActions(true);
    // SAFETY: CGImage is a valid contents class for CALayer.
    unsafe { layer.setContents(Some(image.as_ref())) };
    CATransaction::commit();
}

fn take_premul_buf(need: usize) -> Vec<u32> {
    PREMUL_FREE.with(|cell| {
        let mut free = cell.borrow_mut();
        let mut buf = free
            .iter()
            .enumerate()
            .filter(|(_, v)| v.capacity() >= need)
            .max_by_key(|(_, v)| v.capacity())
            .map(|(i, _)| i)
            .map(|i| free.swap_remove(i))
            .unwrap_or_default();
        buf.clear();
        buf.resize(need, 0);
        buf
    })
}

fn recycle_premul_buf(buf: Vec<u32>) {
    PREMUL_FREE.with(|cell| {
        let mut free = cell.borrow_mut();
        // Keep a couple of recycled slots; drop extras to bound idle RSS.
        if free.len() >= 2 {
            return;
        }
        free.push(buf);
    });
}

fn premultiply_argb(p: u32) -> u32 {
    let a = (p >> 24) & 0xff;
    let r = (p >> 16) & 0xff;
    let g = (p >> 8) & 0xff;
    let b = p & 0xff;
    if a == 0 {
        return 0;
    }
    if a == 255 {
        return p;
    }
    let r = r * a / 255;
    let g = g * a / 255;
    let b = b * a / 255;
    (a << 24) | (r << 16) | (g << 8) | b
}

fn cgimage_from_premul_owned(
    pixels: Vec<u32>,
    width: usize,
    height: usize,
) -> Result<CFRetained<CGImage>, ()> {
    unsafe extern "C-unwind" fn release(_info: *mut c_void, data: NonNull<c_void>, size: usize) {
        let data = data.cast::<u32>();
        let slice = slice_from_raw_parts_mut(data.as_ptr(), size / size_of::<u32>());
        // SAFETY: same allocation we passed to Box::into_raw below.
        let buf = unsafe { Box::from_raw(slice) }.into_vec();
        recycle_premul_buf(buf);
    }

    let buffer = pixels.into_boxed_slice();
    let len = buffer.len() * size_of::<u32>();
    let raw: *mut [u32] = Box::into_raw(buffer);
    let data_ptr = raw.cast::<c_void>();

    let data_provider = unsafe {
        match CGDataProvider::with_data(ptr::null_mut(), data_ptr, len, Some(release)) {
            Some(dp) => dp,
            None => {
                drop(Box::from_raw(raw));
                return Err(());
            }
        }
    };

    let bitmap_info = CGBitmapInfo(
        CGImageAlphaInfo::PremultipliedFirst.0
            | CGImageComponentInfo::Integer.0
            | CGImageByteOrderInfo::Order32Little.0
            | CGImagePixelFormatInfo::Packed.0,
    );

    let color_space = CGColorSpace::new_device_rgb().ok_or(())?;
    let image = unsafe {
        CGImage::new(
            width,
            height,
            8,
            32,
            width * 4,
            Some(&color_space),
            bitmap_info,
            Some(&data_provider),
            ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or(())?;

    Ok(image)
}

/// Global cursor in **top-left** logical desktop coords (winit space).
///
/// Uses the **primary** screen (not `mainScreen`, which follows keyboard focus)
/// so Y conversion stays correct when the cursor is on a secondary display.
pub fn cursor_logical_top_left() -> Option<(f64, f64)> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary = screens.firstObject()?;
    let frame = primary.frame();
    let p = NSEvent::mouseLocation();
    // Cocoa global: origin at bottom-left of primary; winit: top-left, Y down.
    let x = p.x;
    let y = frame.origin.y + frame.size.height - p.y;
    Some((x, y))
}

/// Raw `NSView*` for muda context menus.
pub fn ns_view_ptr(window: &Window) -> Option<*const c_void> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    Some(appkit.ns_view.as_ptr() as *const c_void)
}

fn ns_view(window: &Window) -> Option<Retained<NSView>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let ns_view = appkit.ns_view.as_ptr();
    // SAFETY: ns_view from winit is a valid NSView*.
    let ns_view = unsafe { &*(ns_view as *const NSView) };
    Some(ns_view.retain())
}

fn ns_window(window: &Window) -> Option<Retained<objc2_app_kit::NSWindow>> {
    ns_view(window)?.window()
}
