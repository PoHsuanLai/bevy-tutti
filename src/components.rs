use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use tutti::NodeId;

use crate::assets::TuttiAudioSource;

/// Marks an entity as an audio emitter with a live node in tutti's graph.
///
/// Added automatically by `audio_playback_system` when a `PlayAudio` trigger
/// is processed. Remove this component (or despawn the entity) to stop
/// playback and clean up the graph node.
#[derive(Component)]
pub struct AudioEmitter {
    pub node_id: NodeId,
}

/// Marks an entity as the audio listener (typically the camera).
///
/// Only one listener should exist at a time. Spatial audio positions
/// are computed relative to this entity's `GlobalTransform`.
///
/// Requires the `spatial` feature.
#[cfg(feature = "spatial")]
#[derive(Component, Default)]
pub struct AudioListener;

/// Enables 3D spatial audio for an emitter entity.
///
/// Requires `GlobalTransform` on the same entity. The system lazily creates
/// a `SpatialPannerNode` in tutti's graph and syncs position every frame.
///
/// Requires the `spatial` feature.
#[cfg(feature = "spatial")]
#[derive(Component)]
pub struct SpatialAudio {
    pub(crate) panner_node_id: Option<NodeId>,
    pub attenuation: AttenuationModel,
    pub max_distance: f32,
    pub ref_distance: f32,
}

#[cfg(feature = "spatial")]
impl Default for SpatialAudio {
    fn default() -> Self {
        Self {
            panner_node_id: None,
            attenuation: AttenuationModel::InverseDistance,
            max_distance: 100.0,
            ref_distance: 1.0,
        }
    }
}

#[cfg(feature = "spatial")]
#[derive(Debug, Clone, Copy, Default)]
pub enum AttenuationModel {
    #[default]
    InverseDistance,
    Linear,
    Exponential,
}

/// Playback state for audio emitters.
///
/// Updated by `audio_cleanup_system` when a non-looping sample finishes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPlaybackState {
    #[default]
    Stopped,
    Playing,
    Finished,
}

/// Volume control component. Synced to the tutti graph node by `audio_parameter_sync_system`.
#[derive(Component)]
pub struct AudioVolume(pub f32);

impl Default for AudioVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Trigger component: spawn an entity with this to start audio playback.
///
/// The `audio_playback_system` processes entities with `Added<PlayAudio>`,
/// creates a `SamplerUnit` in tutti's graph, attaches `AudioEmitter`, and
/// removes this component.
///
/// # Examples
///
/// ```rust,ignore
/// // One-shot sound effect
/// commands.spawn(PlayAudio::once(asset_server.load("boom.wav")));
///
/// // Looping ambient sound
/// commands.spawn(PlayAudio::looping(asset_server.load("wind.ogg")).gain(0.3));
///
/// // Auto-despawn when finished
/// commands.spawn(PlayAudio::once(handle).despawn_on_finish());
/// ```
#[derive(Component)]
pub struct PlayAudio {
    pub source: Handle<TuttiAudioSource>,
    pub looping: bool,
    pub gain: f32,
    pub speed: f32,
    pub(crate) auto_despawn: bool,
}

impl PlayAudio {
    pub fn once(source: Handle<TuttiAudioSource>) -> Self {
        Self {
            source,
            looping: false,
            gain: 1.0,
            speed: 1.0,
            auto_despawn: false,
        }
    }

    pub fn looping(source: Handle<TuttiAudioSource>) -> Self {
        Self {
            source,
            looping: true,
            gain: 1.0,
            speed: 1.0,
            auto_despawn: false,
        }
    }

    pub fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn despawn_on_finish(mut self) -> Self {
        self.auto_despawn = true;
        self
    }

    /// Enable time stretching on this audio source.
    ///
    /// Returns a `(PlayAudio, TimeStretch)` tuple for spawning.
    /// Must be the last method in the chain since it changes the return type.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// commands.spawn(PlayAudio::once(handle).gain(0.8).time_stretch(0.5, 0.0));
    /// ```
    #[cfg(feature = "sampler")]
    pub fn time_stretch(self, stretch_factor: f32, pitch_cents: f32) -> (Self, TimeStretch) {
        (self, TimeStretch { stretch_factor, pitch_cents })
    }
}

/// Marker component: entity will be despawned when its sample finishes playing.
#[derive(Component)]
pub struct DespawnOnFinish;

/// Trigger component: spawn an entity with this to create a SoundFont instrument.
///
/// The `soundfont_playback_system` processes entities with `Added<PlaySoundFont>`,
/// creates a `SoundFontUnit` in tutti's graph with MIDI routing, attaches
/// `AudioEmitter`, and removes this component.
///
/// # Examples
///
/// ```rust,ignore
/// // Load a SoundFont and spawn a piano (preset 0)
/// let gm = asset_server.load("sounds/GeneralMidi.sf2");
/// commands.spawn(PlaySoundFont::new(gm).preset(0));
/// ```
#[cfg(feature = "soundfont")]
#[derive(Component)]
pub struct PlaySoundFont {
    pub source: Handle<crate::soundfont_assets::SoundFontSource>,
    pub preset: i32,
    pub channel: i32,
}

#[cfg(feature = "soundfont")]
impl PlaySoundFont {
    pub fn new(source: Handle<crate::soundfont_assets::SoundFontSource>) -> Self {
        Self { source, preset: 0, channel: 0 }
    }

    pub fn preset(mut self, preset: i32) -> Self {
        self.preset = preset;
        self
    }

    pub fn channel(mut self, channel: i32) -> Self {
        self.channel = channel;
        self
    }
}

/// Trigger component: spawn an entity with this to create a neural synth.
///
/// The `neural_synth_playback_system` processes entities with `Added<PlayNeuralSynth>`,
/// loads the model via tutti's neural subsystem, creates a `NeuralSynthNode` in the
/// graph with MIDI routing, and attaches `AudioEmitter`.
///
/// # Examples
///
/// ```rust,ignore
/// let violin = asset_server.load("models/violin.mpk");
/// commands.spawn(PlayNeuralSynth::new(violin));
/// ```
#[cfg(all(feature = "neural", feature = "midi"))]
#[derive(Component)]
pub struct PlayNeuralSynth {
    pub source: Handle<crate::neural_assets::NeuralModelSource>,
}

#[cfg(all(feature = "neural", feature = "midi"))]
impl PlayNeuralSynth {
    pub fn new(source: Handle<crate::neural_assets::NeuralModelSource>) -> Self {
        Self { source }
    }
}

/// Trigger component: spawn an entity with this to create a neural effect.
///
/// The `neural_effect_playback_system` processes entities with `Added<PlayNeuralEffect>`,
/// loads the model via tutti's neural subsystem, creates a `NeuralEffectNode` in the graph,
/// and attaches `AudioEmitter`.
#[cfg(feature = "neural")]
#[derive(Component)]
pub struct PlayNeuralEffect {
    pub source: Handle<crate::neural_assets::NeuralModelSource>,
}

#[cfg(feature = "neural")]
impl PlayNeuralEffect {
    pub fn new(source: Handle<crate::neural_assets::NeuralModelSource>) -> Self {
        Self { source }
    }
}

/// Trigger component: spawn an entity with this to load an audio plugin.
///
/// The `plugin_load_system` processes entities with `Added<LoadPlugin>`,
/// calls the appropriate `engine.vst3/vst2/clap()` builder, creates the
/// plugin in tutti's graph, and replaces this component with
/// `AudioEmitter` + `PluginEmitter`.
///
/// The plugin format (VST3/VST2/CLAP) is auto-detected from the file extension.
///
/// # Examples
///
/// ```rust,ignore
/// // Load a VST3 reverb plugin
/// commands.spawn(LoadPlugin::new("path/to/Reverb.vst3"));
///
/// // Load with initial parameters
/// commands.spawn(LoadPlugin::new("path/to/Synth.clap").param("cutoff", 0.7));
/// ```
#[cfg(feature = "plugin")]
#[derive(Component)]
pub struct LoadPlugin {
    pub path: std::path::PathBuf,
    pub(crate) params: std::collections::HashMap<String, f32>,
}

#[cfg(feature = "plugin")]
impl LoadPlugin {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            params: std::collections::HashMap::new(),
        }
    }

    pub fn param(mut self, name: impl Into<String>, value: f32) -> Self {
        self.params.insert(name.into(), value);
        self
    }
}

/// Marks an entity as a loaded plugin with a control handle.
///
/// Added automatically by `plugin_load_system`. Use the `handle` to
/// control parameters, open/close the editor, save/load state, etc.
///
/// The audio node is tracked separately via `AudioEmitter`.
#[cfg(feature = "plugin")]
#[derive(Component)]
pub struct PluginEmitter {
    pub handle: tutti::PluginHandle,
}

/// Marker component: present while a plugin's GUI editor is open.
///
/// `plugin_editor_idle_system` calls `handle.editor_idle()` every frame
/// for entities that have this marker. Add it after calling
/// `handle.open_editor()`, remove it after `handle.close_editor()`.
#[cfg(feature = "plugin")]
#[derive(Component)]
pub struct PluginEditorOpen;

/// Trigger component: spawn an entity with this to start recording on a channel.
///
/// The `recording_start_system` processes entities with `Added<StartRecording>`,
/// calls `engine.sampler().start_recording()`, replaces this component with
/// `RecordingActive`, and emits a `RecordingEvent::Started`.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct StartRecording {
    pub channel_index: usize,
    pub source: tutti::RecordingSource,
    pub mode: tutti::RecordingMode,
}

#[cfg(feature = "sampler")]
impl StartRecording {
    pub fn new(channel_index: usize, source: tutti::RecordingSource) -> Self {
        Self {
            channel_index,
            source,
            mode: tutti::RecordingMode::Replace,
        }
    }

    pub fn mode(mut self, mode: tutti::RecordingMode) -> Self {
        self.mode = mode;
        self
    }
}

/// Trigger component: spawn or insert on an entity to stop recording on a channel.
///
/// The `recording_stop_system` processes entities with `Added<StopRecording>`,
/// calls `engine.sampler().stop_recording()`, removes `RecordingActive`,
/// and emits a `RecordingEvent::Stopped` with the recorded data.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct StopRecording {
    pub channel_index: usize,
}

/// Marks an entity as having an active recording session.
///
/// Added automatically by `recording_start_system`. Removed when
/// `StopRecording` is processed or recording stops.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct RecordingActive {
    pub channel_index: usize,
    pub source: tutti::RecordingSource,
    pub mode: tutti::RecordingMode,
}

/// Trigger component: spawn an entity with this to enable live analysis.
///
/// Processed by `live_analysis_control_system`, calls `engine.enable_live_analysis()`.
#[cfg(feature = "analysis")]
#[derive(Component)]
pub struct EnableLiveAnalysis;

/// Trigger component: spawn an entity with this to disable live analysis.
///
/// Processed by `live_analysis_control_system`, calls `engine.disable_live_analysis()`.
#[cfg(feature = "analysis")]
#[derive(Component)]
pub struct DisableLiveAnalysis;

/// Trigger component: spawn an entity with this to start an offline export.
///
/// The `export_start_system` processes entities with `Added<StartExport>`,
/// builds an `ExportBuilder`, calls `.start(path)`, and replaces this
/// component with `ExportInProgress`.
#[cfg(feature = "export")]
#[derive(Component)]
pub struct StartExport {
    pub path: std::path::PathBuf,
    pub duration_seconds: Option<f64>,
    pub duration_beats: Option<(f64, f64)>,
    pub format: Option<tutti::AudioFormat>,
    pub normalization: Option<tutti::NormalizationMode>,
}

#[cfg(feature = "export")]
impl StartExport {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            duration_seconds: None,
            duration_beats: None,
            format: None,
            normalization: None,
        }
    }

    pub fn duration_seconds(mut self, seconds: f64) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    pub fn duration_beats(mut self, beats: f64, tempo: f64) -> Self {
        self.duration_beats = Some((beats, tempo));
        self
    }

    pub fn format(mut self, format: tutti::AudioFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn normalization(mut self, mode: tutti::NormalizationMode) -> Self {
        self.normalization = Some(mode);
        self
    }
}

/// Trigger component: spawn an entity with this to enable audio input capture.
///
/// Processed by `audio_input_control_system`. Selects device, sets gain/monitoring,
/// and requests capture start.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct EnableAudioInput {
    pub device_index: Option<usize>,
    pub monitoring: bool,
    pub gain: f32,
}

#[cfg(feature = "sampler")]
impl Default for EnableAudioInput {
    fn default() -> Self {
        Self {
            device_index: None,
            monitoring: false,
            gain: 1.0,
        }
    }
}

#[cfg(feature = "sampler")]
impl EnableAudioInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn device(mut self, index: usize) -> Self {
        self.device_index = Some(index);
        self
    }

    pub fn monitoring(mut self, enabled: bool) -> Self {
        self.monitoring = enabled;
        self
    }

    pub fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }
}

/// Trigger component: spawn an entity with this to disable audio input capture.
///
/// Processed by `audio_input_control_system`. Stops capture and disables monitoring.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct DisableAudioInput;

/// Trigger component: spawn an entity with this to create an automation lane.
///
/// The `automation_lane_system` processes entities with `Added<AddAutomationLane>`,
/// calls `engine.automation_lane(envelope)`, adds the lane to the graph, and
/// replaces this component with `AutomationLaneEmitter`.
///
/// # Examples
///
/// ```rust,ignore
/// use tutti::{AutomationEnvelope, AutomationPoint, CurveType};
///
/// let mut envelope = AutomationEnvelope::new("volume");
/// envelope.add_point(AutomationPoint::new(0.0, 0.0))
///         .add_point(AutomationPoint::with_curve(4.0, 1.0, CurveType::SCurve));
///
/// commands.spawn(AddAutomationLane { envelope });
/// ```
#[cfg(feature = "automation")]
#[derive(Component)]
pub struct AddAutomationLane {
    pub envelope: tutti::AutomationEnvelope<String>,
}

#[cfg(feature = "automation")]
impl AddAutomationLane {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            envelope: tutti::AutomationEnvelope::new(target.into()),
        }
    }

    pub fn with_envelope(envelope: tutti::AutomationEnvelope<String>) -> Self {
        Self { envelope }
    }
}

/// Marks an entity as having an automation lane in the graph.
///
/// Added automatically by `automation_lane_system`. Use `node_id` to
/// connect the lane's output to other graph nodes (e.g., a multiply node
/// for volume automation).
#[cfg(feature = "automation")]
#[derive(Component)]
pub struct AutomationLaneEmitter {
    pub node_id: tutti::NodeId,
}

/// Companion component for `PlayAudio` entities that enables time stretching.
///
/// When present alongside `PlayAudio`, the `audio_playback_system` wraps the
/// `SamplerUnit` in a `TimeStretchUnit` before adding it to the graph.
/// After playback starts, a `TimeStretchControl` component is inserted
/// for lock-free parameter updates.
///
/// # Examples
///
/// ```rust,ignore
/// commands.spawn((
///     PlayAudio::once(asset_server.load("drums.wav")),
///     TimeStretch { stretch_factor: 0.5, pitch_cents: 0.0 },
/// ));
/// ```
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct TimeStretch {
    pub stretch_factor: f32,
    pub pitch_cents: f32,
}

/// Lock-free control handles for a time-stretched audio entity.
///
/// Inserted automatically by `audio_playback_system` when `TimeStretch` is
/// present. Holds `Arc<AtomicFloat>` handles for real-time parameter updates.
/// Updated by `time_stretch_sync_system` when `TimeStretch` changes.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct TimeStretchControl {
    pub(crate) stretch_factor: std::sync::Arc<tutti::AtomicFloat>,
    pub(crate) pitch_cents: std::sync::Arc<tutti::AtomicFloat>,
}

/// Trigger component: spawn an entity with this to add a compressor to the graph.
///
/// The `dsp_compressor_system` processes entities with `Added<AddCompressor>`,
/// creates a `SidechainCompressor` or `StereoSidechainCompressor`, adds it to
/// the graph, and inserts `AudioEmitter`.
#[cfg(feature = "dsp")]
#[derive(Component)]
pub struct AddCompressor {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
    pub makeup_db: f32,
    pub stereo: bool,
}

#[cfg(feature = "dsp")]
impl Default for AddCompressor {
    fn default() -> Self {
        Self {
            threshold_db: -20.0,
            ratio: 4.0,
            attack: 0.005,
            release: 0.1,
            makeup_db: 0.0,
            stereo: false,
        }
    }
}

#[cfg(feature = "dsp")]
impl AddCompressor {
    pub fn new(threshold_db: f32, ratio: f32) -> Self {
        Self {
            threshold_db,
            ratio,
            ..Default::default()
        }
    }

    pub fn attack(mut self, seconds: f32) -> Self {
        self.attack = seconds;
        self
    }

    pub fn release(mut self, seconds: f32) -> Self {
        self.release = seconds;
        self
    }

    pub fn makeup(mut self, db: f32) -> Self {
        self.makeup_db = db;
        self
    }

    pub fn stereo(mut self) -> Self {
        self.stereo = true;
        self
    }
}

/// Trigger component: spawn an entity with this to add a gate to the graph.
///
/// The `dsp_gate_system` processes entities with `Added<AddGate>`,
/// creates a `SidechainGate` or `StereoSidechainGate`, adds it to
/// the graph, and inserts `AudioEmitter`.
#[cfg(feature = "dsp")]
#[derive(Component)]
pub struct AddGate {
    pub threshold_db: f32,
    pub attack: f32,
    pub hold: f32,
    pub release: f32,
    pub stereo: bool,
}

#[cfg(feature = "dsp")]
impl Default for AddGate {
    fn default() -> Self {
        Self {
            threshold_db: -30.0,
            attack: 0.001,
            hold: 0.01,
            release: 0.1,
            stereo: false,
        }
    }
}

#[cfg(feature = "dsp")]
impl AddGate {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            threshold_db,
            ..Default::default()
        }
    }

    pub fn attack(mut self, seconds: f32) -> Self {
        self.attack = seconds;
        self
    }

    pub fn hold(mut self, seconds: f32) -> Self {
        self.hold = seconds;
        self
    }

    pub fn release(mut self, seconds: f32) -> Self {
        self.release = seconds;
        self
    }

    pub fn stereo(mut self) -> Self {
        self.stereo = true;
        self
    }
}

/// Trigger component: spawn an entity with this to add an LFO to the graph.
///
/// The `dsp_lfo_system` processes entities with `Added<AddLfo>`,
/// creates an `LfoNode`, adds it to the graph, and inserts `AudioEmitter`.
/// If `beat_synced` is true, the LFO is wired to the engine's transport.
#[derive(Component)]
pub struct AddLfo {
    pub shape: tutti::LfoShape,
    pub frequency: f32,
    pub depth: f32,
    pub beat_synced: bool,
}

impl Default for AddLfo {
    fn default() -> Self {
        Self {
            shape: tutti::LfoShape::Sine,
            frequency: 1.0,
            depth: 1.0,
            beat_synced: false,
        }
    }
}

impl AddLfo {
    /// Free-running LFO with the given shape and frequency in Hz.
    pub fn new(shape: tutti::LfoShape, frequency: f32) -> Self {
        Self {
            shape,
            frequency,
            ..Default::default()
        }
    }

    /// Beat-synced LFO with the given shape and beats per cycle.
    pub fn beat_synced(shape: tutti::LfoShape, beats_per_cycle: f32) -> Self {
        Self {
            shape,
            frequency: beats_per_cycle,
            beat_synced: true,
            ..Default::default()
        }
    }

    pub fn depth(mut self, depth: f32) -> Self {
        self.depth = depth;
        self
    }
}
