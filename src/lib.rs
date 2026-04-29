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
//! Each subsystem of the `TuttiEngine` is surfaced as its own Bevy resource.
//! Systems take only the ones they need:
//!
//! ```rust,ignore
//! fn control_audio(transport: Res<TransportRes>, mut graph: ResMut<TuttiGraphRes>) {
//!     transport.tempo(128.0).play();
//!     let id = graph.0.add(tutti::dsp::sine_hz(440.0));
//!     graph.0.pipe_output(id);
//!     graph.0.commit();
//! }
//! ```

mod loader;
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
pub mod graph_reconcile;
#[cfg(feature = "automation")]
pub mod automation_bind;
#[cfg(feature = "sampler")]
pub mod pending_load;
#[cfg(feature = "midi")]
pub mod scheduled;
pub mod sidechain;
#[cfg(all(feature = "plugin", feature = "vst2"))]
pub mod vst2_load;
mod metering;
#[cfg(feature = "midi")]
mod midi;
#[cfg(feature = "neural")]
mod neural_status;
#[cfg(feature = "plugin")]
#[cfg(all(target_os = "macos", feature = "plugin"))]
mod live_resize;
#[cfg(feature = "plugin")]
pub mod native_window;
mod systems;
mod transport;

use bevy_app::{App, Plugin, Update};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use bevy_log::{error, info};

#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural"))]
use std::sync::Arc;

pub use loader::{TuttiLoader, TuttiLoaderError, TuttiStreamingLoader, TuttiStreamingLoaderError};
pub use tutti::core::WaveAsset;
#[cfg(feature = "soundfont")]
pub use tutti::synth::SoundFontAsset;
#[cfg(feature = "sampler")]
pub use tutti::sampler::StreamingSample;
#[cfg(feature = "neural")]
pub use tutti::neural::NeuralModel;

pub use components::{AudioEmitter, AudioPlaybackState, AudioVolume, DespawnOnFinish, PlayAudio};
#[cfg(feature = "spatial")]
pub use components::{AttenuationModel, AudioListener, SpatialAudio};

#[cfg(feature = "sampler")]
pub use systems::{audio_cleanup_system, audio_parameter_sync_system, audio_playback_system};
#[cfg(all(feature = "spatial", feature = "sampler"))]
pub use systems::spatial_audio_sync_system;

#[cfg(feature = "midi")]
pub use midi::components::{MidiReceiver, MidiSequence, MidiSequenceNote};
#[cfg(feature = "midi")]
pub use midi::events::MidiInputEvent;
#[cfg(feature = "midi")]
pub use midi::systems::{
    midi_input_event_system, midi_routing_sync_system, midi_sequence_setup_system,
    midi_sequence_tick_system, MidiSequenceState,
};

#[cfg(feature = "midi-hardware")]
pub use midi::components::{ConnectMidiDevice, DisconnectMidiDevice};
#[cfg(feature = "midi-hardware")]
pub use midi::events::MidiDeviceEvent;
#[cfg(feature = "midi-hardware")]
pub use midi::systems::{midi_device_connect_system, midi_device_poll_system};

#[cfg(feature = "mpe")]
pub use midi::components::MpeReceiver;
#[cfg(feature = "mpe")]
pub use tutti::midi::{MpeMode, MpeZone, MpeZoneConfig};

#[cfg(feature = "soundfont")]
pub use components::PlaySoundFont;
#[cfg(feature = "soundfont")]
pub use systems::soundfont_playback_system;

#[cfg(feature = "neural")]
pub use neural_status::{neural_status_sync_system, NeuralStatusResource};
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

pub use tutti::{DeviceInfo, NodeId, TuttiDriver, TuttiEngine, TuttiEngineBuilder, TuttiGraph, Wave};

// Entity-as-node ECS primitives — re-exported from `tutti` so apps don't
// need to reach into `tutti::core::ecs` directly.
pub use tutti::{AudioNode, Mute, NodeKind, Pan, PluginParam, Volume};
#[cfg(feature = "sampler")]
pub use tutti::{SamplerLooping, SamplerSpeed};
pub use graph_reconcile::{
    commit_graph, crossfade_audio_node, reconcile_node_despawn, reconcile_params, GraphDirty,
    GraphReconcileSet, SpawnAudioNode,
};
#[cfg(feature = "sampler")]
pub use graph_reconcile::reconcile_sampler_params;
#[cfg(feature = "sampler")]
pub use pending_load::{
    poll_wave_imports, promote_pending_samplers, PendingSamplerLoad, WaveImportQueue,
};
#[cfg(feature = "midi")]
pub use scheduled::{tick_scheduled_midi, MidiSynthMarker, ScheduledMidi};
pub use sidechain::{reconcile_sidechain_links, SidechainOf, SidechainSources};
#[cfg(all(feature = "plugin", feature = "vst2"))]
pub use vst2_load::{process_pending_vst2_builds, PendingVst2Build};
#[cfg(feature = "plugin")]
pub use graph_reconcile::reconcile_plugin_params;
#[cfg(feature = "automation")]
pub use automation_bind::{
    reconcile_automation_writes, AutomationDrivesParam, AutomationLaneNode, AutomationParam,
};

#[cfg(feature = "midi")]
pub use tutti::midi::{MidiEvent, Note};
#[cfg(feature = "midi")]
pub use tutti::midi_runtime::MidiBus;
#[cfg(feature = "midi-hardware")]
pub use tutti::midi::MidiIo;

#[cfg(feature = "plugin")]
pub use components::{
    ClosePluginEditor, OpenPluginEditor, PendingPluginEditor, PluginEditorOpen, PluginEmitter,
};
#[cfg(feature = "plugin")]
pub use systems::{
    plugin_crash_detect_system, plugin_editor_attach_system, plugin_editor_close_system,
    plugin_editor_idle_system, plugin_editor_open_system, plugin_editor_resize_request_system,
    plugin_editor_window_resize_system,
};
#[cfg(feature = "plugin")]
pub use tutti::plugin::catalog::{
    PluginCatalog, PluginRecord, PluginScanner, Plugins, PluginsConfig, ScanHandle, ScanPhase,
    ScanProgress, ScanResult,
};
#[cfg(feature = "plugin")]
pub use tutti::plugin::catalog::JsonCatalog;
#[cfg(feature = "plugin")]
pub use tutti::plugin::handles::PluginHandle;
#[cfg(feature = "plugin")]
pub use tutti::plugin::metadata::{ParameterFlags, ParameterInfo};

#[cfg(feature = "neural")]
pub use tutti::neural::Engine as NeuralEngine;

#[cfg(feature = "sampler")]
pub use components::{RecordingActive, StartRecording, StopRecording};
#[cfg(feature = "sampler")]
pub use systems::{recording_start_system, recording_stop_system, RecordingResult};
#[cfg(feature = "sampler")]
pub use tutti::sampler::capture::{Mode as RecordingMode, Recorded as RecordedData, Source as RecordingSource};

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
pub use tutti::export::{AudioFormat, Handle as ExportHandle, Normalize, State as ExportState, Written};

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
pub use tutti::automation::{
    AutomationClip, AutomationEnvelope, AutomationLane, AutomationPoint, AutomationState,
    CurveType, LiveAutomationLane,
};

#[cfg(feature = "sampler")]
pub use components::{TimeStretch, TimeStretchControl};
#[cfg(feature = "sampler")]
pub use systems::time_stretch_sync_system;
#[cfg(feature = "sampler")]
pub use tutti::sampler::stretch::Unit as TimeStretchUnit;

pub use components::AddLfo;
pub use dsp_systems::dsp_lfo_system;
pub use tutti::units::{LfoMode, LfoNode, LfoShape};
#[cfg(feature = "dsp")]
pub use components::{AddCompressor, AddGate};
#[cfg(feature = "dsp")]
pub use dsp_systems::{dsp_compressor_system, dsp_gate_system};
#[cfg(feature = "dsp")]
pub use tutti::units::{Compressor, Gate};

// =========================================================================
// Resource wrappers around the flat TuttiEngine bundle.
// =========================================================================

/// Audio device configuration captured at engine build time.
#[derive(Resource, Clone, Copy, Debug)]
pub struct AudioConfig {
    pub sample_rate: f64,
    pub channels: usize,
}

/// Owns the editable DSP graph. `&mut` edits; call `commit()` once per frame
/// after a batch of edits to publish them to the audio thread.
#[derive(Resource)]
pub struct TuttiGraphRes(pub TuttiGraph);

/// Owns the CPAL stream lifecycle (device selection, restart, enumeration).
///
/// The inner `TuttiDriver` holds a `cpal::Stream` which is `Send` but not
/// `Sync` (CPAL streams are not reentrant). Wrapping in a `Mutex` gives us a
/// `Resource`-compatible (`Send + Sync`) handle; in practice driver
/// operations (`set_device` / `restart`) are infrequent and exclusive.
#[derive(Resource)]
pub struct TuttiDriverRes(pub std::sync::Mutex<TuttiDriver>);

impl TuttiDriverRes {
    pub fn new(driver: TuttiDriver) -> Self {
        Self(std::sync::Mutex::new(driver))
    }
}

/// Lock-free transport handle (play/stop/seek/tempo/loop).
#[derive(Resource, Clone)]
pub struct TransportRes(pub tutti::TransportHandle);

impl std::ops::Deref for TransportRes {
    type Target = tutti::TransportHandle;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Lock-free metering handle (peak/RMS/LUFS/CPU snapshots).
#[derive(Resource, Clone)]
pub struct MeteringRes(pub tutti::MeteringHandle);

impl std::ops::Deref for MeteringRes {
    type Target = tutti::MeteringHandle;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// MIDI fan-out bus — audio-thread event dispatch to per-unit inboxes.
#[cfg(feature = "midi")]
#[derive(Resource, Clone)]
pub struct MidiBusRes(pub MidiBus);

#[cfg(feature = "midi")]
impl std::ops::Deref for MidiBusRes {
    type Target = MidiBus;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Hardware MIDI I/O (OS port management + virtual ports). Only present
/// when `.midi()` was called on the builder.
#[cfg(feature = "midi-hardware")]
#[derive(Resource, Clone)]
pub struct MidiIoRes(pub MidiIo);

#[cfg(feature = "midi-hardware")]
impl std::ops::Deref for MidiIoRes {
    type Target = MidiIo;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Sampler subsystem (disk streaming, clip playback, capture).
#[cfg(feature = "sampler")]
#[derive(Resource, Clone)]
pub struct SamplerRes(pub Arc<tutti::sampler::Sampler>);

/// SoundFont system (file cache + synth instantiation).
#[cfg(feature = "soundfont")]
#[derive(Resource, Clone)]
pub struct SoundFontRes(pub Arc<tutti::synth::SoundFontSystem>);

/// Analysis handle (transient / pitch / stereo analysis).
///
/// `AnalysisHandle` is not `Clone` upstream.
#[cfg(feature = "analysis")]
#[derive(Resource)]
pub struct AnalysisRes(pub tutti::analysis::AnalysisHandle);

#[cfg(feature = "analysis")]
impl std::ops::Deref for AnalysisRes {
    type Target = tutti::analysis::AnalysisHandle;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Neural inference engine. Only inserted when a neural backend factory was
/// supplied to the builder; absent otherwise.
#[cfg(feature = "neural")]
#[derive(Resource, Clone)]
pub struct NeuralRes(pub Arc<NeuralEngine>);

/// Non-Send marker resource that forces plugin editor systems to run on the
/// main thread. AppKit (macOS), Win32, and X11 window operations must happen
/// on the main thread. JUCE, VSTGUI, and other plugin GUI frameworks assume
/// this. Inserted as `insert_non_send_resource` so any system that takes
/// `NonSend<PluginEditorMainThread>` is pinned to the main thread.
#[cfg(feature = "plugin")]
pub struct PluginEditorMainThread;

/// Bevy plugin that creates a `TuttiEngine`, starts the audio stream,
/// and registers ECS components, asset loaders, and systems.
pub struct TuttiPlugin {
    /// `None` = system default device
    pub output_device: Option<usize>,
    pub inputs: usize,
    pub outputs: usize,
    pub enable_midi: bool,
    /// Set to `false` to only expose the engine resources without ECS systems.
    pub enable_ecs: bool,
    #[cfg(feature = "mpe")]
    pub mpe_mode: Option<tutti::midi::MpeMode>,
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
    pub fn with_mpe(mut self, mode: tutti::midi::MpeMode) -> Self {
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
                    engine.sample_rate, self.outputs
                );

                // Enable amplitude + CPU metering by default (used by the
                // metering_sync_system).
                engine.metering.inner().enable_amp();
                engine.metering.inner().cpu().enable();

                let sample_rate = engine.sample_rate;
                let channels = engine.channels;

                app.insert_resource(AudioConfig {
                    sample_rate,
                    channels,
                });

                // Destructure the flat bundle into per-subsystem resources.
                let TuttiEngine {
                    graph,
                    driver,
                    transport,
                    metering,
                    #[cfg(feature = "midi")]
                    midi,
                    #[cfg(feature = "midi")]
                    midi_io,
                    #[cfg(feature = "sampler")]
                    sampler,
                    #[cfg(feature = "soundfont")]
                    soundfont,
                    #[cfg(feature = "analysis")]
                    analysis,
                    #[cfg(feature = "neural")]
                    neural,
                    ..
                } = engine;

                app.insert_resource(TuttiGraphRes(graph));
                app.insert_resource(TuttiDriverRes::new(driver));
                app.insert_resource(TransportRes(transport));
                app.insert_resource(MeteringRes(metering));

                #[cfg(feature = "midi")]
                app.insert_resource(MidiBusRes(midi));
                #[cfg(feature = "midi-hardware")]
                if let Some(io) = midi_io {
                    app.insert_resource(MidiIoRes(io));
                }
                #[cfg(all(feature = "midi", not(feature = "midi-hardware")))]
                {
                    let _ = midi_io;
                }

                #[cfg(feature = "sampler")]
                app.insert_resource(SamplerRes(sampler));

                #[cfg(feature = "soundfont")]
                app.insert_resource(SoundFontRes(soundfont));

                #[cfg(feature = "analysis")]
                app.insert_resource(AnalysisRes(analysis));

                #[cfg(feature = "neural")]
                if let Some(n) = neural {
                    app.insert_resource(NeuralRes(n));
                }
            }
            Err(e) => {
                error!("Failed to start Tutti Audio Engine: {}", e);
            }
        }

        app.init_resource::<TransportState>();
        app.init_resource::<MasterMeterLevels>();
        app.init_resource::<AudioDeviceState>();
        app.init_resource::<graph_reconcile::GraphDirty>();
        app.add_systems(
            bevy_app::Startup,
            device_state::device_state_init_system,
        );
        app.add_systems(
            Update,
            (
                transport::transport_sync_system,
                metering::metering_sync_system,
                device_state::device_state_sync_system,
            ),
        );

        // Entity-as-node reconcile pipeline. Runs even when `enable_ecs`
        // is false because it is itself the ECS layer; opting out of all
        // ECS systems is what `enable_ecs = false` covers (trigger
        // components like `PlayAudio`).
        app.configure_sets(
            Update,
            (
                graph_reconcile::GraphReconcileSet::Spawn,
                graph_reconcile::GraphReconcileSet::Params,
                graph_reconcile::GraphReconcileSet::Despawn,
                graph_reconcile::GraphReconcileSet::Commit,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                graph_reconcile::reconcile_params
                    .in_set(graph_reconcile::GraphReconcileSet::Params),
                graph_reconcile::reconcile_node_despawn
                    .in_set(graph_reconcile::GraphReconcileSet::Despawn),
                graph_reconcile::commit_graph
                    .in_set(graph_reconcile::GraphReconcileSet::Commit),
                sidechain::reconcile_sidechain_links
                    .in_set(graph_reconcile::GraphReconcileSet::Spawn),
            ),
        );

        #[cfg(feature = "sampler")]
        {
            app.init_resource::<content_bounds::ContentBounds>();
            app.add_systems(Update, content_bounds::content_bounds_sync_system);
            app.add_systems(
                Update,
                graph_reconcile::reconcile_sampler_params
                    .in_set(graph_reconcile::GraphReconcileSet::Params),
            );

            app.init_resource::<pending_load::WaveImportQueue>();
            app.add_systems(
                Update,
                (
                    pending_load::poll_wave_imports,
                    pending_load::promote_pending_samplers
                        .after(pending_load::poll_wave_imports)
                        .in_set(graph_reconcile::GraphReconcileSet::Spawn),
                ),
            );
        }

        #[cfg(feature = "automation")]
        {
            app.add_systems(
                Update,
                automation_bind::reconcile_automation_writes
                    .in_set(graph_reconcile::GraphReconcileSet::Params)
                    .before(graph_reconcile::reconcile_params),
            );
        }

        #[cfg(feature = "analysis")]
        app.init_resource::<analysis::LiveAnalysisData>();

        #[cfg(feature = "sampler")]
        app.init_resource::<audio_input::AudioInputState>();

        if self.enable_ecs {
            app.init_asset::<WaveAsset>()
                .register_asset_loader(TuttiLoader::<WaveAsset>::default());

            #[cfg(feature = "sampler")]
            {
                app.init_asset::<StreamingSample>()
                    .register_asset_loader(TuttiStreamingLoader::<StreamingSample>::default());
            }

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
                app.init_asset::<SoundFontAsset>()
                    .register_asset_loader(TuttiLoader::<SoundFontAsset>::default());
                app.add_systems(Update, systems::soundfont_playback_system);
            }

            #[cfg(feature = "neural")]
            {
                app.init_asset::<NeuralModel>()
                    .register_asset_loader(TuttiStreamingLoader::<NeuralModel>::default());
                app.init_resource::<NeuralStatusResource>();

                #[cfg(feature = "midi")]
                app.add_systems(Update, systems::neural_synth_playback_system);

                app.add_systems(Update, systems::neural_effect_playback_system);
                app.add_systems(Update, neural_status::neural_status_sync_system);
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
                        midi::systems::midi_sequence_setup_system,
                        midi::systems::midi_sequence_tick_system,
                    )
                        .chain(),
                );

                app.add_systems(Update, scheduled::tick_scheduled_midi);
            }

            #[cfg(feature = "mpe")]
            {
                app.add_systems(
                    bevy_app::Startup,
                    midi::systems::mpe_setup_system,
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

            #[cfg(feature = "plugin")]
            {
                if !app.is_plugin_added::<bevy_tokio_tasks::TokioTasksPlugin>() {
                    app.add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default());
                }

                app.insert_non_send_resource(PluginEditorMainThread);

                app.add_systems(
                    Update,
                    (
                        systems::plugin_editor_open_system,
                        systems::plugin_editor_attach_system,
                        systems::plugin_editor_close_system,
                        systems::plugin_editor_idle_system,
                        systems::plugin_editor_resize_request_system
                            .after(systems::plugin_editor_idle_system),
                        systems::plugin_editor_window_resize_system
                            .after(systems::plugin_editor_resize_request_system),
                        systems::plugin_crash_detect_system,
                    ),
                );

                app.add_systems(
                    Update,
                    graph_reconcile::reconcile_plugin_params
                        .in_set(graph_reconcile::GraphReconcileSet::Params),
                );

                #[cfg(feature = "vst2")]
                app.add_systems(
                    Update,
                    vst2_load::process_pending_vst2_builds
                        .in_set(graph_reconcile::GraphReconcileSet::Spawn),
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
                    bevy_app::Startup,
                    audio_input::audio_input_init_system,
                );
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
