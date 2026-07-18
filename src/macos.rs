//! macOS helpers: accessory app, transparent window, click-through, cursor poll,
//! and CALayer present with real alpha (softbuffer 0.4 ignores alpha on macOS).

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
use objc2_quartz_core::{CATransaction};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

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
    layer.setContentsScale(window.scale_factor());

    // Premultiply; buffer ownership transfers to CGDataProvider (freed on image drop).
    let owned: Vec<u32> = pixels[..need].iter().copied().map(premultiply_argb).collect();

    let Ok(image) = cgimage_from_premul_owned(owned, width as usize, height as usize) else {
        return;
    };

    CATransaction::begin();
    CATransaction::setDisableActions(true);
    // SAFETY: CGImage is a valid contents class for CALayer.
    unsafe { layer.setContents(Some(image.as_ref())) };
    CATransaction::commit();
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
        drop(unsafe { Box::from_raw(slice) });
    }

    let buffer = pixels.into_boxed_slice();
    let len = buffer.len() * size_of::<u32>();
    let raw: *mut [u32] = Box::into_raw(buffer);
    let data_ptr = raw.cast::<c_void>();

    let data_provider = unsafe {
        match CGDataProvider::with_data(ptr::null_mut(), data_ptr, len, Some(release)) {
            Some(dp) => dp,
            None => {
                // Provider never took ownership — reclaim the box ourselves.
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
