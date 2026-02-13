//! Bevy plugin for the Tutti audio engine.
//!
//! Provides ECS components, asset loading, and systems for integrating
//! Tutti into Bevy applications.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use bevy_tutti::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(TuttiPlugin::default())
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands, assets: Res<AssetServer>) {
//!     commands.spawn(PlayAudio::once(assets.load("boom.wav")).despawn_on_finish());
//!     commands.spawn(PlayAudio::looping(assets.load("wind.ogg")).gain(0.3));
//! }
//! ```
//!
//! # Direct API Access
//!
//! ```rust,ignore
//! fn control_audio(engine: Res<TuttiEngineResource>) {
//!     engine.transport().tempo(128.0).play();
//!     engine.graph_mut(|net| net.add(sine_hz::<f32>(440.0)).master());
//! }
//! ```

mod assets;
#[cfg(feature = "analysis")]
mod analysis;
#[cfg(feature = "sampler")]
mod audio_input;
#[cfg(feature = "automation")]
mod automation_systems;
mod components;
#[cfg(feature = "sampler")]
mod content_bounds;
mod device_state;
mod dsp_systems;
#[cfg(feature = "export")]
mod export_systems;
mod metering;
#[cfg(feature = "midi")]
mod midi;
#[cfg(feature = "soundfont")]
mod soundfont_assets;
#[cfg(feature = "neural")]
mod neural_assets;
mod systems;
mod transport;

use bevy_app::{App, Plugin, Update};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

use std::sync::Arc;

pub use assets::{TuttiAudioLoader, TuttiAudioSource};

pub use components::{AudioEmitter, AudioPlaybackState, AudioVolume, DespawnOnFinish, PlayAudio};
#[cfg(feature = "spatial")]
pub use components::{AttenuationModel, AudioListener, SpatialAudio};

#[cfg(feature = "sampler")]
pub use systems::{audio_cleanup_system, audio_parameter_sync_system, audio_playback_system};
#[cfg(all(feature = "spatial", feature = "sampler"))]
pub use systems::spatial_audio_sync_system;

#[cfg(feature = "midi")]
pub use midi::components::{MidiReceiver, SendMidi};
#[cfg(feature = "midi")]
pub use midi::events::MidiInputEvent;
#[cfg(feature = "midi")]
pub use midi::systems::{midi_input_event_system, midi_routing_sync_system, midi_send_system};

#[cfg(feature = "midi-hardware")]
pub use midi::components::{ConnectMidiDevice, DisconnectMidiDevice};
#[cfg(feature = "midi-hardware")]
pub use midi::events::MidiDeviceEvent;
#[cfg(feature = "midi-hardware")]
pub use midi::systems::{midi_device_connect_system, midi_device_poll_system};

#[cfg(feature = "mpe")]
pub use midi::components::MpeReceiver;
#[cfg(feature = "mpe")]
pub use midi::systems::MpeExpressionResource;
#[cfg(feature = "mpe")]
pub use tutti::{MpeHandle, MpeMode, MpeZone, MpeZoneConfig};

#[cfg(feature = "midi2")]
pub use midi::components::SendMidi2;
#[cfg(feature = "midi2")]
pub use midi::systems::midi2_send_system;
#[cfg(feature = "midi2")]
pub use tutti::{Midi2Event, Midi2Handle, Midi2MessageType, UnifiedMidiEvent};

#[cfg(feature = "soundfont")]
pub use soundfont_assets::{Sf2LoadError, Sf2Loader, SoundFontSource};
#[cfg(feature = "soundfont")]
pub use components::PlaySoundFont;
#[cfg(feature = "soundfont")]
pub use systems::soundfont_playback_system;

#[cfg(feature = "neural")]
pub use neural_assets::{NeuralModelLoadError, NeuralModelLoader, NeuralModelSource};
#[cfg(all(feature = "neural", feature = "midi"))]
pub use components::PlayNeuralSynth;
#[cfg(all(feature = "neural", feature = "midi"))]
pub use systems::neural_synth_playback_system;
#[cfg(feature = "neural")]
pub use components::PlayNeuralEffect;
#[cfg(feature = "neural")]
pub use systems::neural_effect_playback_system;

pub use metering::{metering_sync_system, MasterMeterLevels};
pub use transport::{transport_sync_system, TransportState};

pub use device_state::{device_state_sync_system, AudioDeviceState};

#[cfg(feature = "sampler")]
pub use content_bounds::{content_bounds_sync_system, ContentBounds};

pub use tutti::{NodeId, TuttiEngine, TuttiEngineBuilder, Wave};

#[cfg(feature = "midi")]
pub use tutti::{
    Channel, ChannelVoiceMsg, ControlChange, MidiEvent, MidiHandle, Note,
};
#[cfg(feature = "midi-hardware")]
pub use tutti::{MidiInputDevice, MidiOutputDevice};

#[cfg(feature = "plugin")]
pub use components::{LoadPlugin, PluginEditorOpen, PluginEmitter};
#[cfg(feature = "plugin")]
pub use systems::{plugin_crash_detect_system, plugin_editor_idle_system, plugin_load_system};
#[cfg(feature = "plugin")]
pub use tutti::{
    register_all_system_plugins, register_plugin, register_plugin_directory, ParameterFlags,
    ParameterInfo, PluginHandle, PluginMetadata,
};

#[cfg(feature = "neural")]
pub use tutti::{NeuralHandle, NeuralSystem, NeuralSystemBuilder};

#[cfg(feature = "sampler")]
pub use components::{RecordingActive, StartRecording, StopRecording};
#[cfg(feature = "sampler")]
pub use systems::{recording_start_system, recording_stop_system, RecordingResult};
#[cfg(feature = "sampler")]
pub use tutti::{RecordedData, RecordingMode, RecordingSource};

#[cfg(feature = "analysis")]
pub use analysis::{
    live_analysis_control_system, live_analysis_sync_system, LiveAnalysisData,
};
#[cfg(feature = "analysis")]
pub use components::{DisableLiveAnalysis, EnableLiveAnalysis};

#[cfg(feature = "export")]
pub use components::StartExport;
#[cfg(feature = "export")]
pub use export_systems::{
    export_poll_system, export_start_system, ExportComplete, ExportFailed, ExportInProgress,
};
#[cfg(feature = "export")]
pub use tutti::{AudioFormat, ExportHandle, ExportStatus, NormalizationMode};

#[cfg(feature = "sampler")]
pub use audio_input::{
    audio_input_control_system, audio_input_sync_system, AudioInputDeviceInfo, AudioInputState,
};
#[cfg(feature = "sampler")]
pub use components::{DisableAudioInput, EnableAudioInput};

#[cfg(feature = "automation")]
pub use components::{AddAutomationLane, AutomationLaneEmitter};
#[cfg(feature = "automation")]
pub use automation_systems::automation_lane_system;
#[cfg(feature = "automation")]
pub use tutti::{
    AutomationClip, AutomationEnvelope, AutomationLane, AutomationPoint, AutomationState,
    CurveType, LiveAutomationLane,
};

#[cfg(feature = "sampler")]
pub use components::{TimeStretch, TimeStretchControl};
#[cfg(feature = "sampler")]
pub use systems::time_stretch_sync_system;
#[cfg(feature = "sampler")]
pub use tutti::TimeStretchUnit;

pub use components::AddLfo;
pub use dsp_systems::dsp_lfo_system;
pub use tutti::{LfoMode, LfoNode, LfoShape};
#[cfg(feature = "dsp")]
pub use components::{AddCompressor, AddGate};
#[cfg(feature = "dsp")]
pub use dsp_systems::{dsp_compressor_system, dsp_gate_system};
#[cfg(feature = "dsp")]
pub use tutti::{SidechainCompressor, SidechainGate, StereoSidechainCompressor, StereoSidechainGate};

/// Arc wrapper for `TuttiEngine`, exposed as a Bevy resource.
#[derive(Resource, Clone)]
pub struct TuttiEngineResource(pub Arc<TuttiEngine>);

impl std::ops::Deref for TuttiEngineResource {
    type Target = TuttiEngine;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Bevy plugin that creates a `TuttiEngine`, starts the audio stream,
/// and registers ECS components, asset loaders, and systems.
pub struct TuttiPlugin {
    /// `None` = system default device
    pub output_device: Option<usize>,
    pub inputs: usize,
    pub outputs: usize,
    pub enable_midi: bool,
    /// Set to `false` to only expose `TuttiEngineResource` without ECS systems.
    pub enable_ecs: bool,
    #[cfg(feature = "mpe")]
    pub mpe_mode: Option<tutti::MpeMode>,
}

impl Default for TuttiPlugin {
    fn default() -> Self {
        Self {
            output_device: None,
            inputs: 0,
            outputs: 2,
            enable_midi: cfg!(feature = "midi"),
            enable_ecs: true,
            #[cfg(feature = "mpe")]
            mpe_mode: None,
        }
    }
}

impl TuttiPlugin {
    pub fn with_io(inputs: usize, outputs: usize) -> Self {
        Self {
            inputs,
            outputs,
            ..Default::default()
        }
    }

    pub fn with_midi(mut self) -> Self {
        self.enable_midi = true;
        self
    }

    pub fn with_output_device(mut self, index: usize) -> Self {
        self.output_device = Some(index);
        self
    }

    /// Automatically enables MIDI.
    #[cfg(feature = "mpe")]
    pub fn with_mpe(mut self, mode: tutti::MpeMode) -> Self {
        self.mpe_mode = Some(mode);
        self.enable_midi = true;
        self
    }

    pub fn without_ecs(mut self) -> Self {
        self.enable_ecs = false;
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

        #[cfg(feature = "mpe")]
        if let Some(ref mode) = self.mpe_mode {
            builder = builder.mpe(*mode);
        }

        match builder.build() {
            Ok(engine) => {
                info!(
                    "Tutti Audio Engine started ({}Hz, {}ch)",
                    engine.sample_rate(),
                    self.outputs
                );
                engine.metering().amp().cpu();
                app.insert_resource(TuttiEngineResource(Arc::new(engine)));
            }
            Err(e) => {
                error!("Failed to start Tutti Audio Engine: {}", e);
            }
        }

        app.init_resource::<TransportState>();
        app.init_resource::<MasterMeterLevels>();
        app.init_resource::<AudioDeviceState>();
        app.add_systems(
            Update,
            (
                transport::transport_sync_system,
                metering::metering_sync_system,
                device_state::device_state_sync_system,
            ),
        );

        #[cfg(feature = "sampler")]
        {
            app.init_resource::<content_bounds::ContentBounds>();
            app.add_systems(Update, content_bounds::content_bounds_sync_system);
        }

        #[cfg(feature = "analysis")]
        app.init_resource::<analysis::LiveAnalysisData>();

        #[cfg(feature = "sampler")]
        app.init_resource::<audio_input::AudioInputState>();

        if self.enable_ecs {
            app.init_asset::<TuttiAudioSource>()
                .register_asset_loader(TuttiAudioLoader);

            #[cfg(feature = "sampler")]
            app.add_systems(
                Update,
                (
                    systems::audio_playback_system,
                    systems::audio_parameter_sync_system,
                    systems::audio_cleanup_system,
                )
                    .chain(),
            );

            #[cfg(all(feature = "spatial", feature = "sampler"))]
            app.add_systems(
                Update,
                systems::spatial_audio_sync_system
                    .after(systems::audio_playback_system)
                    .before(systems::audio_cleanup_system),
            );

            #[cfg(feature = "soundfont")]
            {
                app.init_asset::<SoundFontSource>()
                    .register_asset_loader(Sf2Loader);
                app.add_systems(Update, systems::soundfont_playback_system);
            }

            #[cfg(feature = "neural")]
            {
                app.init_asset::<NeuralModelSource>()
                    .register_asset_loader(NeuralModelLoader);

                #[cfg(feature = "midi")]
                app.add_systems(Update, systems::neural_synth_playback_system);

                app.add_systems(Update, systems::neural_effect_playback_system);
            }

            #[cfg(feature = "midi")]
            {
                let (sender, receiver) = crossbeam_channel::unbounded();
                app.insert_resource(midi::systems::MidiInputObserver { receiver });
                app.insert_resource(midi::systems::MidiObserverSender {
                    sender: Some(sender),
                });

                app.add_message::<midi::events::MidiInputEvent>();

                app.add_systems(
                    bevy_app::Startup,
                    midi::systems::midi_observer_setup_system,
                );

                app.add_systems(
                    Update,
                    (
                        midi::systems::midi_input_event_system,
                        midi::systems::midi_routing_sync_system,
                        midi::systems::midi_send_system,
                    )
                        .chain(),
                );
            }

            #[cfg(feature = "midi-hardware")]
            {
                app.add_message::<midi::events::MidiDeviceEvent>();
                app.init_resource::<midi::systems::MidiDeviceState>();

                app.add_systems(
                    Update,
                    (
                        midi::systems::midi_device_connect_system,
                        midi::systems::midi_device_poll_system,
                    ),
                );
            }

            #[cfg(feature = "mpe")]
            {
                app.add_systems(
                    bevy_app::Startup,
                    midi::systems::mpe_setup_system,
                );
            }

            #[cfg(feature = "midi2")]
            {
                app.add_systems(Update, midi::systems::midi2_send_system);
            }

            #[cfg(feature = "plugin")]
            {
                app.add_systems(
                    Update,
                    (
                        systems::plugin_load_system,
                        systems::plugin_editor_idle_system,
                        systems::plugin_crash_detect_system,
                    ),
                );
            }

            #[cfg(feature = "sampler")]
            {
                app.add_systems(
                    Update,
                    (
                        systems::recording_start_system,
                        systems::recording_stop_system,
                    ),
                );
            }

            #[cfg(feature = "analysis")]
            {
                app.add_systems(
                    Update,
                    (
                        analysis::live_analysis_control_system,
                        analysis::live_analysis_sync_system,
                    ),
                );
            }

            #[cfg(feature = "export")]
            {
                app.add_systems(
                    Update,
                    (
                        export_systems::export_start_system,
                        export_systems::export_poll_system,
                    ),
                );
            }

            #[cfg(feature = "sampler")]
            {
                app.add_systems(
                    Update,
                    (
                        audio_input::audio_input_control_system,
                        audio_input::audio_input_sync_system,
                    ),
                );
            }

            #[cfg(feature = "sampler")]
            {
                app.add_systems(
                    Update,
                    systems::time_stretch_sync_system
                        .after(systems::audio_playback_system),
                );
            }

            #[cfg(feature = "automation")]
            {
                app.add_systems(Update, automation_systems::automation_lane_system);
            }

            app.add_systems(Update, dsp_systems::dsp_lfo_system);

            #[cfg(feature = "dsp")]
            {
                app.add_systems(
                    Update,
                    (
                        dsp_systems::dsp_compressor_system,
                        dsp_systems::dsp_gate_system,
                    ),
                );
            }
        }
    }
}
