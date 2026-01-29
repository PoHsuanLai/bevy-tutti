//! # bevy-tutti
//!
//! Minimal Bevy plugin for [Tutti](https://github.com/PoHsuanLai/tutti) audio engine.
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
//!     // Direct access to TuttiEngine
//!     audio.transport().set_tempo(128.0);
//!     audio.transport().play();
//! }
//! ```

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

// Re-export Tutti types for convenience
use std::sync::Arc;
pub use tutti::{TuttiEngine, TuttiEngineBuilder};

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
    /// Sample rate (default: system default)
    pub sample_rate: Option<f64>,
    /// Number of input channels
    pub inputs: usize,
    /// Number of output channels
    pub outputs: usize,
}

impl TuttiPlugin {
    /// Create plugin with custom sample rate
    pub fn with_sample_rate(sample_rate: f64) -> Self {
        Self {
            sample_rate: Some(sample_rate),
            inputs: 2,
            outputs: 2,
        }
    }

    /// Create plugin with custom I/O configuration
    pub fn with_io(inputs: usize, outputs: usize) -> Self {
        Self {
            sample_rate: None,
            inputs,
            outputs,
        }
    }
}

impl Plugin for TuttiPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing Tutti Audio Plugin");

        if let Some(sr) = self.sample_rate {
            info!("   Sample rate: {} Hz", sr);
        }

        // Build Tutti engine with optional configuration
        let mut builder = TuttiEngine::builder()
            .inputs(self.inputs)
            .outputs(self.outputs);

        if let Some(sr) = self.sample_rate {
            builder = builder.sample_rate(sr);
        }

        match builder.build() {
            Ok(engine) => {
                info!("Tutti Audio Engine started successfully");

                // Insert as Bevy resource
                app.insert_resource(TuttiEngineResource(Arc::new(engine)));

                info!("Tutti Audio Plugin initialized");
            }
            Err(e) => {
                error!("Failed to start Tutti Audio Engine: {}", e);
            }
        }
    }
}
