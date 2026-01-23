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
//!     // Access the handle for fluent API
//!     let handle = audio.handle();
//!
//!     handle.transport().set_tempo(128.0);
//!     handle.transport().play();
//! }
//! ```

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

// Re-export Tutti types for convenience
use std::sync::Arc;
pub use tutti::{config::TuttiConfig, TuttiEngine};

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
#[derive(Default)]
pub struct TuttiPlugin {
    /// Audio backend configuration
    pub config: TuttiConfig,
}

impl TuttiPlugin {
    /// Create plugin with custom configuration
    pub fn with_config(config: TuttiConfig) -> Self {
        Self { config }
    }

    /// Create plugin with custom sample rate
    pub fn with_sample_rate(sample_rate: f64) -> Self {
        Self {
            config: TuttiConfig::builder()
                .sample_rate(sample_rate)
                .build(),
        }
    }
}

impl Plugin for TuttiPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing Tutti Audio Plugin");
        info!("   Sample rate: {} Hz", self.config.sample_rate);

        // Create and start Tutti engine
        match TuttiEngine::builder()
            .with_config(self.config.clone())
            .start()
        {
            Ok((engine, _handle)) => {
                info!("Tutti Audio Engine started successfully");

                // Insert as Bevy resource
                app.insert_resource(TuttiEngineResource(engine));

                info!("Tutti Audio Plugin initialized");
            }
            Err(e) => {
                error!("Failed to start Tutti Audio Engine: {}", e);
            }
        }
    }
}
