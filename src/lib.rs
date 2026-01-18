//! # bevy-tutti
//!
//! Minimal Bevy plugin for [Tutti](https://github.com/yourusername/tutti) audio engine.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use bevy_tutti::TuttiPlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(TuttiPlugin::default())
//!         .add_systems(Update, control_audio)
//!         .run();
//! }
//!
//! fn control_audio(audio: Res<TuttiEngineResource>) {
//!     // Fluent API!
//!     audio.transport()
//!         .tempo(128.0)
//!         .play();
//!
//!     audio.track(0)
//!         .volume(0.8)
//!         .pan(-0.5)
//!         .synth(1);
//!
//!     let voice_id = audio.track(0).trigger_note(60, 1.0).unwrap();
//!     audio.voice(voice_id).release(Some(0.5)).ok();
//! }
//! ```

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

// Re-export Tutti types for convenience
use std::sync::Arc;
pub use tutti::{config::AudioBackendConfig, TuttiEngine};

/// Bevy resource wrapper for TuttiEngine (Arc for cheap cloning)
#[derive(Resource, Clone)]
pub struct TuttiEngineResource(pub Arc<TuttiEngine>);

impl std::ops::Deref for TuttiEngineResource {
    type Target = TuttiEngine;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Bevy plugin for Tutti audio engine
///
/// Creates TuttiEngine, starts audio stream, and inserts it as a Bevy Resource.
pub struct TuttiPlugin {
    /// Audio backend configuration (capacities, sample rate, etc.)
    pub config: AudioBackendConfig,
}

impl Default for TuttiPlugin {
    fn default() -> Self {
        Self {
            config: AudioBackendConfig::default(),
        }
    }
}

impl TuttiPlugin {
    /// Create plugin with custom configuration
    pub fn with_config(config: AudioBackendConfig) -> Self {
        Self { config }
    }

    /// Create plugin with just max_tracks (all other defaults)
    pub fn with_max_tracks(max_tracks: usize) -> Self {
        Self {
            config: AudioBackendConfig::with_max_tracks(max_tracks),
        }
    }
}

impl Plugin for TuttiPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing Tutti Audio Plugin");
        info!("   Max tracks: {}", self.config.max_tracks);
        info!("   Voices/track: {}", self.config.voice_capacity_per_track);
        info!(
            "   Estimated RAM: {} MB",
            self.config.estimated_memory_bytes() / 1_000_000
        );

        // Create and start Tutti engine with fluent API!
        match TuttiEngine::builder()
            .with_config(self.config.clone())
            .start()
        {
            Ok(engine) => {
                info!("Tutti Audio Engine started successfully");

                // Insert as Bevy resource
                app.insert_resource(TuttiEngineResource(engine));

                // Add systems for note event processing
                #[cfg(feature = "export")]
                app.add_systems(bevy_app::Update, TuttiEngine::process_note_events);

                info!("Tutti Audio Plugin initialized");
            }
            Err(e) => {
                error!("❌ Failed to start Tutti Audio Engine: {}", e);
            }
        }
    }
}
