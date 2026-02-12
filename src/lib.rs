//! # bevy-tutti
//!
//! Minimal Bevy plugin for [Tutti](https://github.com/PoHsuanLai/tutti) audio engine.
//!
//! ## Basic Usage
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
//!     audio.transport().tempo(128.0).play();
//! }
//! ```
//!
//! ## Plugin Hosting (requires `plugin` feature)
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use bevy_tutti::*;
//!
//! fn load_plugin(audio: Res<TuttiEngineResource>) {
//!     let id = audio.graph(|net| {
//!         let plugin = audio.vst3("path/to/plugin.vst3").build().unwrap();
//!         net.add(plugin).master()
//!     });
//! }
//! ```
//!
//! ## Neural Audio (requires `neural` feature)
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use bevy_tutti::*;
//!
//! fn setup_neural(audio: Res<TuttiEngineResource>) {
//!     let id = audio.graph(|net| {
//!         let effect = audio.neural_effect("model.onnx").build().unwrap();
//!         net.add(effect).master()
//!     });
//! }
//! ```

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

use std::sync::Arc;

// Re-export core Tutti types
pub use tutti::{TuttiEngine, TuttiEngineBuilder};

// Re-export plugin registration functions
#[cfg(feature = "plugin")]
pub use tutti::{register_all_system_plugins, register_plugin, register_plugin_directory};

// Re-export neural types
#[cfg(feature = "neural")]
pub use tutti::{NeuralHandle, NeuralSystem, NeuralSystemBuilder};

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
/// Sample rate is determined by the audio output device.
pub struct TuttiPlugin {
    /// Output device index (None = system default)
    pub output_device: Option<usize>,
    /// Number of input channels (default: 0)
    pub inputs: usize,
    /// Number of output channels (default: 2)
    pub outputs: usize,
    /// Enable MIDI subsystem (requires `midi` feature)
    pub enable_midi: bool,
}

impl Default for TuttiPlugin {
    fn default() -> Self {
        Self {
            output_device: None,
            inputs: 0,
            outputs: 2,
            enable_midi: cfg!(feature = "midi"),
        }
    }
}

impl TuttiPlugin {
    /// Create plugin with custom I/O configuration
    pub fn with_io(inputs: usize, outputs: usize) -> Self {
        Self {
            inputs,
            outputs,
            ..Default::default()
        }
    }

    /// Enable MIDI subsystem (requires `midi` feature)
    pub fn with_midi(mut self) -> Self {
        self.enable_midi = true;
        self
    }

    /// Set the output device index
    pub fn with_output_device(mut self, index: usize) -> Self {
        self.output_device = Some(index);
        self
    }
}

impl Plugin for TuttiPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing Tutti Audio Plugin");

        let mut builder = TuttiEngine::builder()
            .inputs(self.inputs)
            .outputs(self.outputs);

        if let Some(device) = self.output_device {
            builder = builder.output_device(device);
        }

        #[cfg(feature = "midi")]
        if self.enable_midi {
            builder = builder.midi();
        }

        match builder.build() {
            Ok(engine) => {
                info!("Tutti Audio Engine started ({}Hz, {}ch)", engine.sample_rate(), self.outputs);
                // Enable amplitude + CPU metering by default for UI
                engine.metering().amp().cpu();
                app.insert_resource(TuttiEngineResource(Arc::new(engine)));
            }
            Err(e) => {
                error!("Failed to start Tutti Audio Engine: {}", e);
            }
        }
    }
}
