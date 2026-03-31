//! Cross-platform plugin editor view management.
//!
//! The [`PluginEditorPlatform`] trait abstracts native view operations (create,
//! position, mask, input, z-order) so the frontend can manage plugin editor
//! windows without platform-specific code.

use bevy_ecs::prelude::Resource;

/// Handle to a platform-native editor view (NSView pointer, HWND, XWindow, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorViewHandle(pub u64);

/// Platform-specific plugin editor view management.
pub trait PluginEditorPlatform: Send + Sync + 'static {
    /// Create a child view inside the parent window's content view.
    fn create_view(&self, parent: u64) -> EditorViewHandle;

    /// Resize the child view to match the plugin's reported dimensions.
    fn resize_view(&self, handle: EditorViewHandle, width: u32, height: u32);

    /// Remove and destroy the child view.
    fn destroy_view(&self, handle: EditorViewHandle);

    /// Position the view within the parent (egui viewport coords, top-left origin).
    fn set_frame(&self, handle: EditorViewHandle, x: f64, y: f64, w: f64, h: f64);

    /// Show or hide the view.
    fn set_visible(&self, handle: EditorViewHandle, visible: bool);

    /// Apply a visual clip mask. `clip_rects` are `(x, y, w, h)` in view-local
    /// top-left coords. Areas inside clip_rects become transparent.
    fn set_visual_mask(
        &self,
        handle: EditorViewHandle,
        view_w: f64,
        view_h: f64,
        clip_rects: &[(f64, f64, f64, f64)],
    );

    /// Update input masking. Place shield views over clipped regions so clicks
    /// don't reach the plugin.
    fn set_input_mask(&self, handle: EditorViewHandle, clip_rects: &[(f64, f64, f64, f64)]);

    /// Reorder views to match the given back-to-front order.
    fn reorder_views(&self, parent: u64, ordered: &[EditorViewHandle]);

    /// Returns the handle of the last view that received a valid (non-clipped)
    /// click, if any. Clears the value after reading.
    fn take_last_clicked(&self) -> Option<EditorViewHandle>;

    /// Clean up all state.
    fn cleanup(&self);
}

/// Bevy resource wrapping the platform implementation.
#[derive(Resource)]
pub struct PluginEditorPlatformRes(pub Box<dyn PluginEditorPlatform>);

impl std::ops::Deref for PluginEditorPlatformRes {
    type Target = dyn PluginEditorPlatform;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

/// Create the appropriate platform implementation for the current OS.
pub fn create_platform() -> Box<dyn PluginEditorPlatform> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSPluginEditor::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NullPluginEditor)
    }
}

/// No-op implementation for unsupported platforms.
pub struct NullPluginEditor;

impl PluginEditorPlatform for NullPluginEditor {
    fn create_view(&self, _parent: u64) -> EditorViewHandle { EditorViewHandle(0) }
    fn resize_view(&self, _h: EditorViewHandle, _w: u32, _h2: u32) {}
    fn destroy_view(&self, _h: EditorViewHandle) {}
    fn set_frame(&self, _h: EditorViewHandle, _x: f64, _y: f64, _w: f64, _h2: f64) {}
    fn set_visible(&self, _h: EditorViewHandle, _v: bool) {}
    fn set_visual_mask(&self, _h: EditorViewHandle, _vw: f64, _vh: f64, _c: &[(f64, f64, f64, f64)]) {}
    fn set_input_mask(&self, _h: EditorViewHandle, _c: &[(f64, f64, f64, f64)]) {}
    fn reorder_views(&self, _p: u64, _o: &[EditorViewHandle]) {}
    fn take_last_clicked(&self) -> Option<EditorViewHandle> { None }
    fn cleanup(&self) {}
}

// =============================================================================
// macOS implementation
// =============================================================================

#[cfg(target_os = "macos")]
mod macos {
    use super::{EditorViewHandle, PluginEditorPlatform};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Per-plugin state: the shield views currently covering clipped regions.
    struct PluginShields {
        /// Shield NSView pointers (leaked Retained, like the plugin view itself).
        shields: Vec<u64>,
    }

    pub struct MacOSPluginEditor {
        /// Map from plugin view handle → its shield views.
        shields: Mutex<HashMap<u64, PluginShields>>,
    }

    impl MacOSPluginEditor {
        pub fn new() -> Self {
            Self {
                shields: Mutex::new(HashMap::new()),
            }
        }
    }

    impl PluginEditorPlatform for MacOSPluginEditor {
        fn create_view(&self, parent: u64) -> EditorViewHandle {
            use objc2::rc::Retained;
            use objc2::MainThreadOnly;
            use objc2_app_kit::NSView;
            use objc2_foundation::NSRect;

            unsafe {
                let parent_view: &NSView = &*(parent as *const NSView);
                let frame: NSRect = parent_view.frame();
                let mtm = objc2::MainThreadMarker::new_unchecked();
                let child: Retained<NSView> = NSView::initWithFrame(NSView::alloc(mtm), frame);
                parent_view.addSubview(&child);
                let ptr = Retained::into_raw(child) as u64;
                bevy_log::info!("Created plugin editor NSView at {ptr:#x}");
                EditorViewHandle(ptr)
            }
        }

        fn resize_view(&self, handle: EditorViewHandle, width: u32, height: u32) {
            use objc2_app_kit::NSView;
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            unsafe {
                let view: &NSView = &*(handle.0 as *const NSView);
                view.setFrame(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(width as f64, height as f64),
                ));
            }
        }

        fn destroy_view(&self, handle: EditorViewHandle) {
            use objc2_app_kit::NSView;

            // Remove shields first.
            self.remove_shields(handle.0);

            unsafe {
                let view: &NSView = &*(handle.0 as *const NSView);
                view.removeFromSuperview();
            }
        }

        fn set_frame(&self, handle: EditorViewHandle, x: f64, y: f64, w: f64, h: f64) {
            use objc2_app_kit::NSView;
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            unsafe {
                let view: &NSView = &*(handle.0 as *const NSView);
                view.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)));
            }
        }

        fn set_visible(&self, handle: EditorViewHandle, visible: bool) {
            use objc2_app_kit::NSView;

            unsafe {
                let view: &NSView = &*(handle.0 as *const NSView);
                view.setHidden(!visible);
            }

            // Also hide/show shields.
            if let Ok(shields) = self.shields.lock() {
                if let Some(ps) = shields.get(&handle.0) {
                    for &shield_ptr in &ps.shields {
                        unsafe {
                            let shield: &objc2_app_kit::NSView =
                                &*(shield_ptr as *const objc2_app_kit::NSView);
                            shield.setHidden(!visible);
                        }
                    }
                }
            }
        }

        fn set_visual_mask(
            &self,
            handle: EditorViewHandle,
            view_w: f64,
            view_h: f64,
            clip_rects: &[(f64, f64, f64, f64)],
        ) {
            use objc2_app_kit::NSView;
            use objc2_core_graphics::CGMutablePath;
            use objc2_foundation::{NSPoint, NSRect, NSSize};
            use objc2_quartz_core::CAShapeLayer;

            unsafe {
                let view: &NSView = &*(handle.0 as *const NSView);
                view.setWantsLayer(true);
                let Some(layer) = view.layer() else { return };

                if clip_rects.is_empty() {
                    layer.setMask(None);
                    return;
                }

                let path = CGMutablePath::new();
                let bounds =
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(view_w, view_h));
                CGMutablePath::add_rect(Some(&path), std::ptr::null(), bounds);

                // Clip rects: top-left (egui) → bottom-left (CALayer).
                for &(x, y, w, h) in clip_rects {
                    let flipped_y = view_h - y - h;
                    let clip = NSRect::new(NSPoint::new(x, flipped_y), NSSize::new(w, h));
                    CGMutablePath::add_rect(Some(&path), std::ptr::null(), clip);
                }

                let mask_layer = CAShapeLayer::new();
                mask_layer.setPath(Some(&path));
                mask_layer.setFillRule(objc2_quartz_core::kCAFillRuleEvenOdd);
                layer.setMask(Some(&mask_layer));
            }
        }

        fn set_input_mask(&self, handle: EditorViewHandle, clip_rects: &[(f64, f64, f64, f64)]) {
            use objc2::MainThreadOnly;
            use objc2::rc::Retained;
            use objc2_app_kit::NSView;
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            let mut shields_map = self.shields.lock().unwrap();
            let entry = shields_map.entry(handle.0).or_insert_with(|| PluginShields {
                shields: Vec::new(),
            });

            unsafe {
                let plugin_view: &NSView = &*(handle.0 as *const NSView);
                let Some(parent) = plugin_view.superview() else { return };
                let plugin_frame = plugin_view.frame();

                // Resize pool: create or remove shield views as needed.
                let mtm = objc2::MainThreadMarker::new_unchecked();
                while entry.shields.len() < clip_rects.len() {
                    let shield: Retained<NSView> = NSView::initWithFrame(
                        NSView::alloc(mtm),
                        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                    );
                    // Place shield directly above the plugin view.
                    parent.addSubview_positioned_relativeTo(
                        &shield,
                        objc2_app_kit::NSWindowOrderingMode::Above,
                        Some(plugin_view),
                    );
                    let ptr = Retained::into_raw(shield) as u64;
                    entry.shields.push(ptr);
                }
                while entry.shields.len() > clip_rects.len() {
                    let ptr = entry.shields.pop().unwrap();
                    let shield: &NSView = &*(ptr as *const NSView);
                    shield.removeFromSuperview();
                }

                // Position each shield to cover its clip rect.
                // Clip rects are in plugin-local top-left coords.
                // Parent (content view) is flipped (top-left origin).
                // Shield frame = plugin_frame.origin + clip_rect offset.
                for (i, &(cx, cy, cw, ch)) in clip_rects.iter().enumerate() {
                    let shield: &NSView = &*(entry.shields[i] as *const NSView);
                    let frame = NSRect::new(
                        NSPoint::new(
                            plugin_frame.origin.x + cx,
                            plugin_frame.origin.y + cy,
                        ),
                        NSSize::new(cw, ch),
                    );
                    shield.setFrame(frame);
                }
            }
        }

        fn reorder_views(&self, parent: u64, ordered: &[EditorViewHandle]) {
            use objc2_app_kit::{NSView, NSWindowOrderingMode};

            unsafe {
                let parent: &NSView = &*(parent as *const NSView);
                for i in 1..ordered.len() {
                    let view: &NSView = &*(ordered[i].0 as *const NSView);
                    let below: &NSView = &*(ordered[i - 1].0 as *const NSView);

                    // Place plugin view above the previous plugin.
                    parent.addSubview_positioned_relativeTo(
                        view,
                        NSWindowOrderingMode::Above,
                        Some(below),
                    );

                    // Also move this plugin's shields above it.
                    if let Ok(shields) = self.shields.lock() {
                        if let Some(ps) = shields.get(&ordered[i].0) {
                            for &shield_ptr in &ps.shields {
                                let shield: &NSView = &*(shield_ptr as *const NSView);
                                parent.addSubview_positioned_relativeTo(
                                    shield,
                                    NSWindowOrderingMode::Above,
                                    Some(view),
                                );
                            }
                        }
                    }
                }

                // Also ensure first plugin's shields are above it.
                if let Some(first) = ordered.first() {
                    let view: &NSView = &*(first.0 as *const NSView);
                    if let Ok(shields) = self.shields.lock() {
                        if let Some(ps) = shields.get(&first.0) {
                            for &shield_ptr in &ps.shields {
                                let shield: &NSView = &*(shield_ptr as *const NSView);
                                parent.addSubview_positioned_relativeTo(
                                    shield,
                                    NSWindowOrderingMode::Above,
                                    Some(view),
                                );
                            }
                        }
                    }
                }
            }
        }

        fn take_last_clicked(&self) -> Option<EditorViewHandle> {
            // With shield views, clicks in clipped regions never reach the
            // plugin — they hit the shield (which doesn't handle events) and
            // fall through to egui. We don't need click tracking anymore.
            // The egui FloatingWindow's own click detection handles focus.
            None
        }

        fn cleanup(&self) {
            // Remove all shield views.
            let mut shields_map = self.shields.lock().unwrap();
            for (_, ps) in shields_map.drain() {
                for shield_ptr in ps.shields {
                    unsafe {
                        let shield: &objc2_app_kit::NSView =
                            &*(shield_ptr as *const objc2_app_kit::NSView);
                        shield.removeFromSuperview();
                    }
                }
            }
        }
    }

    impl MacOSPluginEditor {
        fn remove_shields(&self, handle: u64) {
            let mut shields_map = self.shields.lock().unwrap();
            if let Some(ps) = shields_map.remove(&handle) {
                for shield_ptr in ps.shields {
                    unsafe {
                        let shield: &objc2_app_kit::NSView =
                            &*(shield_ptr as *const objc2_app_kit::NSView);
                        shield.removeFromSuperview();
                    }
                }
            }
        }
    }
}
