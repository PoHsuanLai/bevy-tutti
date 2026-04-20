#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural"))]
use bevy_asset::Assets;
#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural", feature = "plugin"))]
use bevy_ecs::prelude::*;
#[cfg(any(feature = "sampler", feature = "plugin"))]
use bevy_log::warn;

#[cfg(feature = "sampler")]
use tutti::core::WaveAsset;
#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural", feature = "plugin"))]
use crate::components::*;

#[cfg(feature = "sampler")]
use tutti::sampler::SamplerUnit;
#[cfg(feature = "sampler")]
use crate::components::{TimeStretch, TimeStretchControl};

/// Processes `PlayAudio` trigger components, creates `SamplerUnit` nodes in
/// tutti's graph, and attaches `AudioEmitter` to the entity.
///
/// If a `TimeStretch` component is present on the same entity, the sampler
/// is wrapped in a `TimeStretchUnit` and a `TimeStretchControl` component
/// is inserted for lock-free parameter updates.
#[cfg(feature = "sampler")]
pub fn audio_playback_system(
    mut commands: Commands,
    audio_assets: Res<Assets<WaveAsset>>,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    config: Option<Res<crate::AudioConfig>>,
    query: Query<(Entity, &PlayAudio), Added<PlayAudio>>,
    ts_query: Query<&TimeStretch>,
) {
    let Some(mut graph) = graph else { return };
    let Some(config) = config else { return };

    let mut edited = false;

    for (entity, play) in query.iter() {
        let Some(source) = audio_assets.get(&play.source) else {
            warn!("WaveAsset not loaded yet for entity {entity:?}, will retry next frame");
            continue;
        };

        let wave = source.0.clone();
        let gain = play.gain;
        let speed = play.speed;
        let looping = play.looping;

        let ts = ts_query.get(entity).ok();
        let sample_rate = config.sample_rate;

        let sampler = SamplerUnit::with_settings(wave, gain, speed, looping);

        let (node_id, ts_control) = if let Some(ts) = ts {
            let wrapped =
                tutti::sampler::stretch::Unit::new(Box::new(sampler), sample_rate);
            wrapped.set_stretch_factor(ts.stretch_factor);
            wrapped.set_pitch_cents(ts.pitch_cents);
            let control = TimeStretchControl {
                stretch_factor: wrapped.stretch_factor_arc(),
                pitch_cents: wrapped.pitch_cents_arc(),
            };
            let id = graph.0.add(Box::new(wrapped));
            graph.0.pipe_output(id);
            (id, Some(control))
        } else {
            let id = graph.0.add(Box::new(sampler));
            graph.0.pipe_output(id);
            (id, None)
        };
        edited = true;

        let mut entity_commands = commands.entity(entity);
        entity_commands
            .remove::<PlayAudio>()
            .insert((AudioEmitter { node_id }, AudioPlaybackState::Playing));

        if let Some(control) = ts_control {
            entity_commands.insert(control);
        }

        if play.auto_despawn {
            entity_commands.insert(DespawnOnFinish);
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Syncs `AudioVolume` component changes to the tutti graph node's gain.
#[cfg(feature = "sampler")]
pub fn audio_parameter_sync_system(
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    query: Query<(&AudioEmitter, &AudioVolume), Changed<AudioVolume>>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;
    for (emitter, volume) in query.iter() {
        if let Some(sampler) = graph.0.node_mut::<SamplerUnit>(emitter.node_id) {
            sampler.set_gain(volume.0);
            edited = true;
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Polls tutti graph for finished (non-looping) samples and updates
/// `AudioPlaybackState`. Removes graph nodes and optionally despawns entities.
#[cfg(feature = "sampler")]
pub fn audio_cleanup_system(
    mut commands: Commands,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    mut query: Query<(
        Entity,
        &AudioEmitter,
        &mut AudioPlaybackState,
        Option<&DespawnOnFinish>,
    )>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;

    for (entity, emitter, mut state, despawn) in query.iter_mut() {
        if *state != AudioPlaybackState::Playing {
            continue;
        }

        let is_playing = graph
            .0
            .node::<SamplerUnit>(emitter.node_id)
            .map(|s| s.is_playing())
            .unwrap_or(false);

        if !is_playing {
            *state = AudioPlaybackState::Finished;

            if graph.0.contains(emitter.node_id) {
                graph.0.remove(emitter.node_id);
                edited = true;
            }

            if despawn.is_some() {
                commands.entity(entity).despawn();
            }
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Syncs `TimeStretch` component changes to the lock-free `TimeStretchControl` atomics.
///
/// When `TimeStretch` is mutated, this system writes the new values to the
/// `Arc<AtomicF32>` handles, which the audio thread reads lock-free.
#[cfg(feature = "sampler")]
pub fn time_stretch_sync_system(
    query: Query<(&TimeStretch, &TimeStretchControl), Changed<TimeStretch>>,
) {
    for (ts, control) in query.iter() {
        control
            .stretch_factor
            .store(ts.stretch_factor, tutti::core::Ordering::Release);
        control
            .pitch_cents
            .store(ts.pitch_cents, tutti::core::Ordering::Release);
    }
}

/// Syncs entity `GlobalTransform` to tutti's spatial panner nodes.
///
/// Lazily creates a `SpatialPannerNode` for each emitter with `SpatialAudio`.
/// Computes listener-relative azimuth/elevation and applies distance attenuation.
#[cfg(all(feature = "spatial", feature = "sampler"))]
pub fn spatial_audio_sync_system(
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    listener_query: Query<&bevy_transform::components::GlobalTransform, With<AudioListener>>,
    mut emitter_query: Query<(
        &bevy_transform::components::GlobalTransform,
        &AudioEmitter,
        &mut SpatialAudio,
    )>,
) {
    let Some(mut graph) = graph else { return };
    let listener_tf = listener_query.single().ok();

    let mut edited = false;

    for (emitter_tf, emitter, mut spatial) in emitter_query.iter_mut() {
        if spatial.panner_node_id.is_none() {
            let emitter_node = emitter.node_id;
            let Ok(panner) = tutti::dsp_nodes::SpatialPannerNode::stereo() else {
                warn!("Failed to create SpatialPannerNode");
                continue;
            };
            let panner_id = graph.0.add(Box::new(panner));
            // Route: emitter → panner → master
            graph.0.connect(emitter_node, 0, panner_id, 0);
            graph.0.pipe_output(panner_id);
            edited = true;
            spatial.panner_node_id = Some(panner_id);
        }

        let Some(panner_id) = spatial.panner_node_id else {
            continue;
        };

        let (azimuth, elevation, distance) = if let Some(listener) = listener_tf {
            let relative = listener
                .affine()
                .inverse()
                .transform_point3(emitter_tf.translation());
            let az = (-relative.x).atan2(-relative.z).to_degrees();
            let el = relative
                .y
                .atan2((relative.x * relative.x + relative.z * relative.z).sqrt())
                .to_degrees();
            (az, el, relative.length())
        } else {
            let pos = emitter_tf.translation();
            let az = pos.x.atan2(pos.z).to_degrees();
            let el = pos
                .y
                .atan2((pos.x * pos.x + pos.z * pos.z).sqrt())
                .to_degrees();
            (az, el, pos.length())
        };

        if let Some(panner) = graph
            .0
            .node::<tutti::dsp_nodes::SpatialPannerNode>(panner_id)
        {
            panner.set_position(azimuth, elevation);
        }

        let gain = compute_attenuation(
            distance,
            spatial.attenuation,
            spatial.ref_distance,
            spatial.max_distance,
        );
        if let Some(sampler) = graph.0.node_mut::<SamplerUnit>(emitter.node_id) {
            sampler.set_gain(gain);
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Processes `PlaySoundFont` trigger components, creates `SoundFontUnit` nodes
/// in tutti's graph with MIDI routing, and attaches `AudioEmitter` to the entity.
#[cfg(feature = "soundfont")]
pub fn soundfont_playback_system(
    mut commands: Commands,
    sf_assets: Res<Assets<tutti::synth::SoundFontAsset>>,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    config: Option<Res<crate::AudioConfig>>,
    #[cfg(feature = "midi")] midi: Option<Res<crate::MidiBusRes>>,
    query: Query<(Entity, &crate::components::PlaySoundFont), Added<crate::components::PlaySoundFont>>,
) {
    let Some(mut graph) = graph else { return };
    let Some(config) = config else { return };

    let mut edited = false;

    for (entity, play) in query.iter() {
        let Some(source) = sf_assets.get(&play.source) else {
            continue;
        };

        let settings = tutti::synth::SynthesizerSettings::new(config.sample_rate as i32);
        let mut unit = match tutti::synth::SoundFontUnit::new(source.0.clone(), &settings) {
            Ok(unit) => unit,
            Err(e) => {
                bevy_log::error!("Failed to create SoundFontUnit: {}", e);
                commands
                    .entity(entity)
                    .remove::<crate::components::PlaySoundFont>();
                continue;
            }
        };
        unit.program_change(play.channel, play.preset);

        // Register the unit's MIDI sender with the bus so the routing table
        // can dispatch events to it by MidiUnitId.
        #[cfg(feature = "midi")]
        if let Some(midi) = &midi {
            midi.0.insert(unit.midi_sender());
        }

        let id = graph.0.add(Box::new(unit));
        graph.0.pipe_output(id);
        edited = true;

        commands
            .entity(entity)
            .remove::<crate::components::PlaySoundFont>()
            .insert(AudioEmitter { node_id: id });
    }

    if edited {
        graph.0.commit();
    }
}

/// Processes `PlayNeuralSynth` trigger components, loads the neural model,
/// creates a `NeuralSynthNode` in tutti's graph with MIDI routing, and
/// attaches `AudioEmitter` to the entity.
#[cfg(all(feature = "neural", feature = "midi"))]
pub fn neural_synth_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<tutti::neural::NeuralModel>>,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    neural: Option<Res<crate::NeuralRes>>,
    query: Query<(Entity, &crate::components::PlayNeuralSynth), Added<crate::components::PlayNeuralSynth>>,
) {
    let Some(mut graph) = graph else { return };
    let Some(neural) = neural else { return };

    let mut edited = false;

    for (entity, play) in query.iter() {
        let Some(source) = model_assets.get(&play.source) else {
            continue;
        };

        match load_neural_model(&neural.0, source) {
            Ok(unit) => {
                let id = graph.0.master(unit);
                edited = true;
                commands
                    .entity(entity)
                    .remove::<crate::components::PlayNeuralSynth>()
                    .insert(AudioEmitter { node_id: id });
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to create neural synth '{}': {}",
                    source.path.display(),
                    e
                );
                commands
                    .entity(entity)
                    .remove::<crate::components::PlayNeuralSynth>();
            }
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Processes `PlayNeuralEffect` trigger components, loads the neural model,
/// creates a `NeuralEffectNode` in tutti's graph, and attaches `AudioEmitter`.
#[cfg(feature = "neural")]
pub fn neural_effect_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<tutti::neural::NeuralModel>>,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    neural: Option<Res<crate::NeuralRes>>,
    query: Query<(Entity, &crate::components::PlayNeuralEffect), Added<crate::components::PlayNeuralEffect>>,
) {
    let Some(mut graph) = graph else { return };
    let Some(neural) = neural else { return };

    let mut edited = false;

    for (entity, play) in query.iter() {
        let Some(source) = model_assets.get(&play.source) else {
            continue;
        };

        match load_neural_model(&neural.0, source) {
            Ok(unit) => {
                let id = graph.0.master(unit);
                edited = true;
                commands
                    .entity(entity)
                    .remove::<crate::components::PlayNeuralEffect>()
                    .insert(AudioEmitter { node_id: id });
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to create neural effect '{}': {}",
                    source.path.display(),
                    e
                );
                commands
                    .entity(entity)
                    .remove::<crate::components::PlayNeuralEffect>();
            }
        }
    }

    if edited {
        graph.0.commit();
    }
}

#[cfg(feature = "neural")]
fn load_neural_model(
    engine: &std::sync::Arc<tutti::neural::Engine>,
    source: &tutti::neural::NeuralModel,
) -> Result<Box<dyn tutti::AudioUnit>, tutti::Error> {
    #[cfg(feature = "ort")]
    if source.path.extension().and_then(|e| e.to_str()) == Some("onnx") {
        let (unit, _id) = tutti::onnx(engine, &source.path).build()?;
        return Ok(unit);
    }

    #[cfg(not(feature = "ort"))]
    let _ = engine;

    Err(tutti::Error::Core(tutti::core::Error::InvalidConfig(
        format!("Unsupported neural model format: {}", source.path.display()),
    )))
}

#[cfg(all(feature = "spatial", feature = "sampler"))]
fn compute_attenuation(
    distance: f32,
    model: crate::components::AttenuationModel,
    ref_distance: f32,
    max_distance: f32,
) -> f32 {
    use crate::components::AttenuationModel;
    if distance >= max_distance {
        return 0.0;
    }

    match model {
        AttenuationModel::InverseDistance => {
            ref_distance / (ref_distance + (distance - ref_distance).max(0.0))
        }
        AttenuationModel::Linear => 1.0 - (distance / max_distance).clamp(0.0, 1.0),
        AttenuationModel::Exponential => (distance / ref_distance).powf(-2.0).clamp(0.0, 1.0),
    }
}

/// Ticks `editor_idle()` on all plugins that have `PluginEditorOpen`.
///
/// Call this in Bevy's `Update` schedule. Plugin GUIs require periodic
/// idle ticks to handle redraws and event processing.
#[cfg(feature = "plugin")]
pub fn plugin_editor_idle_system(
    _main_thread: NonSend<crate::PluginEditorMainThread>,
    query: Query<(&PluginEmitter, &PluginEditorOpen)>,
) {
    for (emitter, _) in query.iter() {
        emitter.handle.editor_idle();
    }
}

/// Phase 1 of plugin editor opening: spawn a Bevy Window for the editor.
///
/// The native handle won't be available until the next frame, so we insert
/// `PendingPluginEditor` and let `plugin_editor_attach_system` finish the job.
#[cfg(feature = "plugin")]
pub fn plugin_editor_open_system(
    mut commands: Commands,
    query: Query<(Entity, &PluginEmitter), Added<OpenPluginEditor>>,
) {
    use bevy_window::{Window, WindowResolution};

    for (entity, emitter) in query.iter() {
        commands.entity(entity).remove::<OpenPluginEditor>();

        let window_entity = commands
            .spawn(Window {
                title: emitter.handle.name().to_string(),
                resolution: WindowResolution::new(800, 600),
                decorations: true,
                visible: false,
                ..Default::default()
            })
            .id();

        bevy_log::info!(
            "Spawning editor window for '{}' (window={window_entity:?})",
            emitter.handle.name(),
        );

        commands
            .entity(entity)
            .insert(PendingPluginEditor { window_entity });
    }
}

/// Phase 2: once the native handle is available, call `open_editor` on the plugin.
#[cfg(feature = "plugin")]
pub fn plugin_editor_attach_system(
    _main_thread: NonSend<crate::PluginEditorMainThread>,
    mut commands: Commands,
    pending: Query<(Entity, &PluginEmitter, &PendingPluginEditor)>,
    mut windows: Query<&mut bevy_window::Window>,
    handles: Query<&bevy_window::RawHandleWrapper>,
    primary: Query<&bevy_window::RawHandleWrapper, With<bevy_window::PrimaryWindow>>,
) {
    for (entity, emitter, pend) in pending.iter() {
        let Ok(raw_handle) = handles.get(pend.window_entity) else {
            continue; // handle not ready yet
        };
        // SAFETY: plugin editor systems are pinned to the main thread via
        // `PluginEditorMainThread` non-send marker; `get_handle` is safe to
        // call on the main thread.
        let thread_locked = unsafe { raw_handle.get_handle() };

        match emitter.handle.open_editor(&thread_locked) {
            Ok(size) => {
                let w = size.width;
                let h = size.height;
                bevy_log::info!(
                    "Plugin '{}' editor opened ({w}x{h})",
                    emitter.handle.name()
                );

                // Resize and show the window.
                if let Ok(mut win) = windows.get_mut(pend.window_entity) {
                    win.resolution.set(w as f32, h as f32);
                    win.visible = true;
                }

                // Attach as child of primary window so they move together.
                if let Ok(parent_handle) = primary.single() {
                    attach_child_window(raw_handle, parent_handle);
                }

                // Remove RawHandleWrapper so Bevy's renderer doesn't create a
                // wgpu surface on this window (the plugin owns the rendering).
                commands
                    .entity(pend.window_entity)
                    .remove::<bevy_window::RawHandleWrapper>();

                commands
                    .entity(entity)
                    .remove::<PendingPluginEditor>()
                    .insert(PluginEditorOpen {
                        editor_window: pend.window_entity,
                        width: w,
                        height: h,
                    });
            }
            Err(e) => {
                warn!(
                    "Plugin '{}' editor failed to open: {}",
                    emitter.handle.name(),
                    e,
                );
                commands.entity(pend.window_entity).despawn();
                commands.entity(entity).remove::<PendingPluginEditor>();
            }
        }
    }
}

// Platform helpers re-exported for use in this module.
#[cfg(feature = "plugin")]
use crate::native_window::attach_child_window;

/// Closes plugin editors for entities with `ClosePluginEditor` trigger.
#[cfg(feature = "plugin")]
pub fn plugin_editor_close_system(
    _main_thread: NonSend<crate::PluginEditorMainThread>,
    mut commands: Commands,
    query: Query<(Entity, &PluginEmitter, &PluginEditorOpen), Added<ClosePluginEditor>>,
) {
    for (entity, emitter, editor) in query.iter() {
        emitter.handle.close_editor();
        commands.entity(editor.editor_window).try_despawn();
        bevy_log::info!(
            "Plugin '{}' editor closed (entity {entity:?})",
            emitter.handle.name()
        );
        commands
            .entity(entity)
            .remove::<ClosePluginEditor>()
            .remove::<PluginEditorOpen>();
    }
}

/// Detects crashed plugins and removes them from the graph.
///
/// Polls `handle.is_crashed()` for all plugin entities. If a plugin has
/// crashed, removes the graph node and despawns `PluginEmitter` + `PluginEditorOpen`.
#[cfg(feature = "plugin")]
pub fn plugin_crash_detect_system(
    mut commands: Commands,
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    query: Query<(Entity, &AudioEmitter, &PluginEmitter)>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;

    for (entity, audio, plugin) in query.iter() {
        if plugin.handle.is_crashed() {
            bevy_log::error!(
                "Plugin '{}' crashed (entity {entity:?}), removing from graph",
                plugin.handle.name()
            );

            if graph.0.contains(audio.node_id) {
                graph.0.remove(audio.node_id);
                edited = true;
            }

            commands
                .entity(entity)
                .remove::<PluginEmitter>()
                .remove::<PluginEditorOpen>()
                .remove::<AudioEmitter>();
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Processes `StartRecording` trigger components.
///
/// Calls `sampler.recording().start_recording()` with the current transport beat,
/// replaces the trigger with `RecordingActive`.
#[cfg(feature = "sampler")]
pub fn recording_start_system(
    mut commands: Commands,
    sampler: Option<Res<crate::SamplerRes>>,
    transport: Res<crate::TransportState>,
    query: Query<(Entity, &StartRecording), Added<StartRecording>>,
) {
    let Some(sampler) = sampler else { return };

    for (entity, start) in query.iter() {
        match sampler.0.recording().start_recording(
            start.channel_index,
            start.source,
            start.mode,
            transport.beat,
        ) {
            Ok(()) => {
                commands
                    .entity(entity)
                    .remove::<StartRecording>()
                    .insert(RecordingActive {
                        channel_index: start.channel_index,
                        source: start.source,
                        mode: start.mode,
                    });
                bevy_log::info!(
                    "Recording started on channel {} ({:?}, {:?})",
                    start.channel_index,
                    start.source,
                    start.mode
                );
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to start recording on channel {}: {}",
                    start.channel_index,
                    e
                );
                commands.entity(entity).remove::<StartRecording>();
            }
        }
    }
}

/// Processes `StopRecording` trigger components.
///
/// Calls `sampler.recording().stop_recording()`, removes `RecordingActive`,
/// and logs the result. The `RecordedData` is available in the log;
/// for programmatic access, use the direct sampler API.
#[cfg(feature = "sampler")]
pub fn recording_stop_system(
    mut commands: Commands,
    sampler: Option<Res<crate::SamplerRes>>,
    query: Query<(Entity, &StopRecording), Added<StopRecording>>,
    active_query: Query<(Entity, &RecordingActive)>,
) {
    let Some(sampler) = sampler else { return };

    for (entity, stop) in query.iter() {
        match sampler.0.recording().stop_recording(stop.channel_index) {
            Ok(data) => {
                bevy_log::info!(
                    "Recording stopped on channel {}, data captured",
                    stop.channel_index
                );
                for (active_entity, active) in active_query.iter() {
                    if active.channel_index == stop.channel_index {
                        commands.entity(active_entity).remove::<RecordingActive>();
                    }
                }
                commands
                    .entity(entity)
                    .remove::<StopRecording>()
                    .insert(RecordingResult(data));
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to stop recording on channel {}: {}",
                    stop.channel_index,
                    e
                );
                commands.entity(entity).remove::<StopRecording>();
            }
        }
    }
}

/// Holds the recorded data after a recording session completes.
///
/// Inserted by `recording_stop_system` on the entity that had `StopRecording`.
/// Consume and remove this component to process the recorded data.
#[cfg(feature = "sampler")]
#[derive(Component)]
pub struct RecordingResult(pub tutti::sampler::capture::Recorded);
