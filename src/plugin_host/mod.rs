//! Plugin (VST2/VST3/CLAP) hosting: editor lifecycle + crash detection.
//!
//! Sub-concepts:
//! - [`editor`] — open / attach / idle / window-resize / close. The 5-system
//!   choreography that owns the plugin GUI window's lifecycle.
//! - [`crash`] — polls each plugin's crashed flag and unwires from the graph.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;

mod crash;
mod editor;

pub use crash::plugin_crash_detect_system;
pub use editor::{
    plugin_editor_attach_system, plugin_editor_close_system, plugin_editor_idle_system,
    plugin_editor_open_system, plugin_editor_resize_request_system,
    plugin_editor_window_resize_system, ClosePluginEditor, OpenPluginEditor, PendingPluginEditor,
    PluginEditorOpen, PluginEmitter,
};

/// Bevy plugin: plugin editor lifecycle + crash detection.
///
/// Inserts the [`crate::resources::PluginEditorMainThread`] non-send marker
/// to pin editor systems to the main thread (required by AppKit / Win32 / X11).
pub struct TuttiHostingPlugin;

impl Plugin for TuttiHostingPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy_tokio_tasks::TokioTasksPlugin>() {
            app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());
        }

        app.insert_non_send_resource(crate::resources::PluginEditorMainThread);

        app.add_systems(
            Update,
            (
                plugin_editor_open_system,
                plugin_editor_attach_system,
                plugin_editor_close_system,
                plugin_editor_idle_system,
                plugin_editor_resize_request_system.after(plugin_editor_idle_system),
                plugin_editor_window_resize_system.after(plugin_editor_resize_request_system),
                plugin_crash_detect_system,
            ),
        );
    }
}
