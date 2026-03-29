#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural"))]
use bevy_asset::Assets;
#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural", feature = "plugin"))]
use bevy_ecs::prelude::*;
#[cfg(any(feature = "sampler", feature = "plugin"))]
use bevy_log::warn;

#[cfg(feature = "sampler")]
use crate::assets::TuttiAudioSource;
#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural", feature = "plugin"))]
use crate::components::*;
#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural", feature = "plugin"))]
use crate::TuttiEngineResource;

#[cfg(feature = "sampler")]
use tutti::SamplerUnit;
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
    audio_assets: Res<Assets<TuttiAudioSource>>,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &PlayAudio), Added<PlayAudio>>,
    ts_query: Query<&TimeStretch>,
) {
    let Some(engine) = engine else { return };

    for (entity, play) in query.iter() {
        let Some(source) = audio_assets.get(&play.source) else {
            warn!("TuttiAudioSource not loaded yet for entity {entity:?}, will retry next frame");
            continue;
        };

        let wave = source.wave().clone();
        let gain = play.gain;
        let speed = play.speed;
        let looping = play.looping;

        let ts = ts_query.get(entity).ok();
        let sample_rate = engine.sample_rate();

        let (node_id, ts_control) = engine.graph_mut(|net| {
            let sampler = SamplerUnit::with_settings(wave, gain, speed, looping);

            if let Some(ts) = ts {
                let wrapped =
                    tutti::TimeStretchUnit::new(Box::new(sampler), sample_rate);
                wrapped.set_stretch_factor(ts.stretch_factor);
                wrapped.set_pitch_cents(ts.pitch_cents);
                let control = TimeStretchControl {
                    stretch_factor: wrapped.stretch_factor_arc(),
                    pitch_cents: wrapped.pitch_cents_arc(),
                };
                let id = net.add(wrapped).master();
                (id, Some(control))
            } else {
                let id = net.add(sampler).master();
                (id, None)
            }
        });

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
}

/// Syncs `AudioVolume` component changes to the tutti graph node's gain.
#[cfg(feature = "sampler")]
pub fn audio_parameter_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(&AudioEmitter, &AudioVolume), Changed<AudioVolume>>,
) {
    let Some(engine) = engine else { return };

    for (emitter, volume) in query.iter() {
        let node_id = emitter.node_id;
        let gain = volume.0;
        engine.graph_mut(|net| {
            if let Some(sampler) = net.node_mut_typed::<SamplerUnit>(node_id) {
                sampler.set_gain(gain);
            }
        });
    }
}

/// Polls tutti graph for finished (non-looping) samples and updates
/// `AudioPlaybackState`. Removes graph nodes and optionally despawns entities.
#[cfg(feature = "sampler")]
pub fn audio_cleanup_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    mut query: Query<(
        Entity,
        &AudioEmitter,
        &mut AudioPlaybackState,
        Option<&DespawnOnFinish>,
    )>,
) {
    let Some(engine) = engine else { return };

    for (entity, emitter, mut state, despawn) in query.iter_mut() {
        if *state != AudioPlaybackState::Playing {
            continue;
        }

        let is_playing = engine.graph(|net| {
            net.node_ref_typed::<SamplerUnit>(emitter.node_id)
                .map(|s| s.is_playing())
                .unwrap_or(false)
        });

        if !is_playing {
            *state = AudioPlaybackState::Finished;

            engine.graph_mut(|net| {
                if net.contains(emitter.node_id) {
                    net.remove(emitter.node_id);
                }
            });

            if despawn.is_some() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Syncs `TimeStretch` component changes to the lock-free `TimeStretchControl` atomics.
///
/// When `TimeStretch` is mutated, this system writes the new values to the
/// `Arc<AtomicFloat>` handles, which the audio thread reads lock-free.
#[cfg(feature = "sampler")]
pub fn time_stretch_sync_system(
    query: Query<(&TimeStretch, &TimeStretchControl), Changed<TimeStretch>>,
) {
    for (ts, control) in query.iter() {
        control.stretch_factor.set(ts.stretch_factor);
        control.pitch_cents.set(ts.pitch_cents);
    }
}

/// Syncs entity `GlobalTransform` to tutti's spatial panner nodes.
///
/// Lazily creates a `SpatialPannerNode` for each emitter with `SpatialAudio`.
/// Computes listener-relative azimuth/elevation and applies distance attenuation.
#[cfg(all(feature = "spatial", feature = "sampler"))]
pub fn spatial_audio_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    listener_query: Query<&bevy_transform::components::GlobalTransform, With<AudioListener>>,
    mut emitter_query: Query<(
        &bevy_transform::components::GlobalTransform,
        &AudioEmitter,
        &mut SpatialAudio,
    )>,
) {
    let Some(engine) = engine else { return };
    let listener_tf = listener_query.single().ok();

    for (emitter_tf, emitter, mut spatial) in emitter_query.iter_mut() {
        if spatial.panner_node_id.is_none() {
            let emitter_node = emitter.node_id;
            let panner_id = engine.graph_mut(|net| {
                let Ok(panner) = tutti::SpatialPannerNode::stereo() else {
                    warn!("Failed to create SpatialPannerNode");
                    return None;
                };
                let panner_id = net.add(panner).id();
                // Route: emitter → panner → master
                net.connect_ports(emitter_node, 0, panner_id, 0);
                net.pipe_output(panner_id);
                Some(panner_id)
            });
            spatial.panner_node_id = panner_id;
            if panner_id.is_none() {
                continue;
            }
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

        engine.graph(|net| {
            if let Some(panner) =
                net.node_ref_typed::<tutti::SpatialPannerNode>(panner_id)
            {
                panner.set_position(azimuth, elevation);
            }
        });

        let gain = compute_attenuation(
            distance,
            spatial.attenuation,
            spatial.ref_distance,
            spatial.max_distance,
        );
        engine.graph_mut(|net| {
            if let Some(sampler) = net.node_mut_typed::<SamplerUnit>(emitter.node_id) {
                sampler.set_gain(gain);
            }
        });
    }
}

/// Processes `PlaySoundFont` trigger components, creates `SoundFontUnit` nodes
/// in tutti's graph with MIDI routing, and attaches `AudioEmitter` to the entity.
#[cfg(feature = "soundfont")]
pub fn soundfont_playback_system(
    mut commands: Commands,
    sf_assets: Res<Assets<crate::soundfont_assets::SoundFontSource>>,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &crate::components::PlaySoundFont), Added<crate::components::PlaySoundFont>>,
) {
    let Some(engine) = engine else { return };

    for (entity, play) in query.iter() {
        let Some(source) = sf_assets.get(&play.source) else {
            continue;
        };

        let settings = tutti::SynthesizerSettings::new(engine.sample_rate() as i32);
        let midi_registry = engine.graph_mut(|net| net.midi_registry().clone());
        let mut unit = tutti::SoundFontUnit::with_midi(
            source.soundfont().clone(),
            &settings,
            midi_registry,
        );
        unit.program_change(play.channel, play.preset);

        let node_id = engine.graph_mut(|net| net.add(unit).master());

        commands.entity(entity)
            .remove::<crate::components::PlaySoundFont>()
            .insert(AudioEmitter { node_id });
    }
}

/// Processes `PlayNeuralSynth` trigger components, loads the neural model,
/// creates a `NeuralSynthNode` in tutti's graph with MIDI routing, and
/// attaches `AudioEmitter` to the entity.
#[cfg(all(feature = "neural", feature = "midi"))]
pub fn neural_synth_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<crate::neural_assets::NeuralModelSource>>,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &crate::components::PlayNeuralSynth), Added<crate::components::PlayNeuralSynth>>,
) {
    let Some(engine) = engine else { return };

    for (entity, play) in query.iter() {
        let Some(source) = model_assets.get(&play.source) else {
            continue;
        };

        let result = load_neural_model(&engine, source);
        match result {
            Ok((unit, model_id)) => {
                let node_id = engine.graph_mut(|net| {
                    net.add_neural_boxed(unit, model_id).master()
                });
                commands.entity(entity)
                    .remove::<crate::components::PlayNeuralSynth>()
                    .insert(AudioEmitter { node_id });
            }
            Err(e) => {
                bevy_log::error!("Failed to create neural synth '{}': {}", source.name, e);
                commands.entity(entity).remove::<crate::components::PlayNeuralSynth>();
            }
        }
    }
}

/// Processes `PlayNeuralEffect` trigger components, loads the neural model,
/// creates a `NeuralEffectNode` in tutti's graph, and attaches `AudioEmitter`.
#[cfg(feature = "neural")]
pub fn neural_effect_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<crate::neural_assets::NeuralModelSource>>,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &crate::components::PlayNeuralEffect), Added<crate::components::PlayNeuralEffect>>,
) {
    let Some(engine) = engine else { return };

    for (entity, play) in query.iter() {
        let Some(source) = model_assets.get(&play.source) else {
            continue;
        };

        let result = load_neural_model(&engine, source);
        match result {
            Ok((unit, model_id)) => {
                let node_id = engine.graph_mut(|net| {
                    net.add_neural_boxed(unit, model_id).master()
                });
                commands.entity(entity)
                    .remove::<crate::components::PlayNeuralEffect>()
                    .insert(AudioEmitter { node_id });
            }
            Err(e) => {
                bevy_log::error!("Failed to create neural effect '{}': {}", source.name, e);
                commands.entity(entity).remove::<crate::components::PlayNeuralEffect>();
            }
        }
    }
}

#[cfg(feature = "neural")]
fn load_neural_model(
    engine: &crate::TuttiEngineResource,
    source: &crate::neural_assets::NeuralModelSource,
) -> Result<(Box<dyn tutti::AudioUnit>, tutti::NeuralModelId), tutti::Error> {
    #[cfg(feature = "ort")]
    if source.path.extension().and_then(|e| e.to_str()) == Some("onnx") {
        return engine.onnx(&source.path).build();
    }

    Err(tutti::Error::Core(tutti::core::Error::InvalidConfig(
        format!("Unsupported neural model format: {}", source.path.display()),
    )))
}

#[cfg(feature = "spatial")]
fn compute_attenuation(
    distance: f32,
    model: AttenuationModel,
    ref_distance: f32,
    max_distance: f32,
) -> f32 {
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

/// Opens a plugin editor embedded in the primary window.
///
/// Creates a plain child NSView inside the main window's NSView and passes
/// it to the plugin's `open_editor()`. The child view has no layer-backing,
/// following the baseview/nih-plug pattern that JUCE and VSTGUI expect.
#[cfg(feature = "plugin")]
pub fn plugin_editor_open_system(
    _main_thread: NonSend<crate::PluginEditorMainThread>,
    mut commands: Commands,
    query: Query<(Entity, &PluginEmitter), Added<OpenPluginEditor>>,
    primary_window: Query<&bevy_window::RawHandleWrapper, With<bevy_window::PrimaryWindow>>,
) {
    let Ok(raw_handle) = primary_window.single() else {
        return;
    };
    let Some(parent_ptr) = raw_handle_to_u64(raw_handle.get_window_handle()) else {
        return;
    };

    for (entity, emitter) in query.iter() {
        commands.entity(entity).remove::<OpenPluginEditor>();

        let child_ptr = create_plain_child_view(parent_ptr);

        bevy_log::info!(
            "Opening editor for '{}' in primary window (parent={parent_ptr:#x}, child={child_ptr:#x})",
            emitter.handle.name(),
        );

        if let Some((w, h)) = emitter.handle.open_editor(child_ptr) {
            bevy_log::info!(
                "Plugin '{}' editor opened ({w}x{h})",
                emitter.handle.name()
            );
            resize_child_view(child_ptr, w, h);
            commands.entity(entity).insert(PluginEditorOpen {
                nsview_ptr: child_ptr,
                width: w,
                height: h,
            });
        } else {
            warn!(
                "Plugin '{}' editor failed to open",
                emitter.handle.name()
            );
            remove_child_view(child_ptr);
        }
    }
}

/// Extract a u64 native handle pointer from a `RawWindowHandle`.
#[cfg(feature = "plugin")]
fn raw_handle_to_u64(raw: raw_window_handle::RawWindowHandle) -> Option<u64> {
    use raw_window_handle::RawWindowHandle;
    match raw {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as u64),
        RawWindowHandle::Win32(h) => Some(isize::from(h.hwnd) as u64),
        RawWindowHandle::Xlib(h) => Some(h.window as u64),
        RawWindowHandle::Xcb(h) => Some(h.window.get() as u64),
        RawWindowHandle::Wayland(h) => Some(h.surface.as_ptr() as u64),
        _ => None,
    }
}

/// Creates a plain child NSView inside the given parent NSView.
///
/// On macOS, winit marks its NSView as layer-backed (`setWantsLayer(true)`).
/// Plugin editors (VSTGUI, JUCE) expect a plain NSView without layer-backing —
/// they set up their own CALayer internally. This function creates a bare
/// `NSView` via `initWithFrame:` and adds it as a subview.
#[cfg(feature = "plugin")]
fn create_plain_child_view(parent_ptr: u64) -> u64 {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2::MainThreadOnly;
        use objc2_app_kit::NSView;
        use objc2_foundation::NSRect;

        unsafe {
            let parent_view: &NSView = &*(parent_ptr as *const NSView);
            let frame: NSRect = parent_view.frame();

            let mtm = objc2::MainThreadMarker::new_unchecked();

            let child: Retained<NSView> = NSView::initWithFrame(NSView::alloc(mtm), frame);
            parent_view.addSubview(&child);

            let ptr = Retained::into_raw(child) as u64;
            bevy_log::info!(
                "Created plain child NSView at {ptr:#x} (frame={:?})",
                frame
            );
            ptr
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        parent_ptr
    }
}

/// Resizes the plain child NSView to match the plugin editor's size.
#[cfg(feature = "plugin")]
fn resize_child_view(child_ptr: u64, width: u32, height: u32) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        unsafe {
            let child: &NSView = &*(child_ptr as *const NSView);
            let new_frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(width as f64, height as f64),
            );
            child.setFrame(new_frame);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (child_ptr, width, height);
    }
}

/// Removes the child NSView from its parent (calls `removeFromSuperview`).
#[cfg(feature = "plugin")]
fn remove_child_view(child_ptr: u64) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;

        unsafe {
            let child: &NSView = &*(child_ptr as *const NSView);
            child.removeFromSuperview();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = child_ptr;
    }
}

/// Reposition a plugin editor's NSView within the main window.
///
/// Coordinates use **top-left origin** (matching egui/screen coords).
/// Call this each frame from your layout system after computing the editor's
/// screen rect.
#[cfg(feature = "plugin")]
pub fn set_editor_frame(nsview_ptr: u64, x: f64, y: f64, w: f64, h: f64, _parent_height: f64) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        unsafe {
            let view: &NSView = &*(nsview_ptr as *const NSView);

            // Winit's content view has isFlipped=true (top-left origin),
            // matching egui's coordinate system — no Y-flip needed.
            let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
            view.setFrame(frame);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (nsview_ptr, x, y, w, h, _parent_height);
    }
}

/// Show or hide a plugin editor's NSView.
#[cfg(feature = "plugin")]
pub fn set_editor_visible(nsview_ptr: u64, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;

        unsafe {
            let view: &NSView = &*(nsview_ptr as *const NSView);
            view.setHidden(!visible);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (nsview_ptr, visible);
    }
}

/// Apply a CALayer mask to a plugin editor's NSView so that the given
/// rects (in view-local coordinates) are clipped out, revealing egui
/// content underneath.
///
/// Each rect is `(x, y, w, h)` in the NSView's local coordinate space.
/// If `clip_rects` is empty, any existing mask is removed.
#[cfg(feature = "plugin")]
pub fn set_editor_mask(nsview_ptr: u64, view_w: f64, view_h: f64, clip_rects: &[(f64, f64, f64, f64)]) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;
        use objc2_core_graphics::CGMutablePath;
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        use objc2_quartz_core::CAShapeLayer;

        unsafe {
            let view: &NSView = &*(nsview_ptr as *const NSView);

            // Ensure the view is layer-backed so we can apply a mask.
            view.setWantsLayer(true);

            let Some(layer) = view.layer() else { return };

            if clip_rects.is_empty() {
                // No occlusion — remove mask to show everything.
                layer.setMask(None);
                return;
            }

            // Build a path: full view bounds MINUS each clip rect.
            // Using even-odd fill rule, adding the full rect then the clip rects
            // creates "holes" where the clip rects are.
            let path = CGMutablePath::new();

            // Full view bounds (visible area)
            let bounds = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(view_w, view_h),
            );
            CGMutablePath::add_rect(Some(&path), std::ptr::null(), bounds);

            // Each clip rect becomes a hole (even-odd rule).
            // Clip rects arrive in top-left origin (egui coords) but CALayer
            // uses bottom-left origin — flip Y.
            for &(x, y, w, h) in clip_rects {
                let flipped_y = view_h - y - h;
                let clip = NSRect::new(
                    NSPoint::new(x, flipped_y),
                    NSSize::new(w, h),
                );
                CGMutablePath::add_rect(Some(&path), std::ptr::null(), clip);
            }

            // Create mask layer with even-odd fill
            let mask_layer = CAShapeLayer::new();
            mask_layer.setPath(Some(&path));
            mask_layer.setFillRule(objc2_quartz_core::kCAFillRuleEvenOdd);

            layer.setMask(Some(&mask_layer));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (nsview_ptr, view_w, view_h, clip_rects);
    }
}

/// Closes plugin editors for entities with `ClosePluginEditor` trigger.
/// Detaches the child NSView from the primary window.
#[cfg(feature = "plugin")]
pub fn plugin_editor_close_system(
    _main_thread: NonSend<crate::PluginEditorMainThread>,
    mut commands: Commands,
    query: Query<(Entity, &PluginEmitter, &PluginEditorOpen), Added<ClosePluginEditor>>,
) {
    for (entity, emitter, editor) in query.iter() {
        emitter.handle.close_editor();
        remove_child_view(editor.nsview_ptr);
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
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &AudioEmitter, &PluginEmitter)>,
) {
    let Some(engine) = engine else { return };

    for (entity, audio, plugin) in query.iter() {
        if plugin.handle.is_crashed() {
            bevy_log::error!(
                "Plugin '{}' crashed (entity {entity:?}), removing from graph",
                plugin.handle.name()
            );

            engine.graph_mut(|net| {
                if net.contains(audio.node_id) {
                    net.remove(audio.node_id);
                }
            });

            commands
                .entity(entity)
                .remove::<PluginEmitter>()
                .remove::<PluginEditorOpen>()
                .remove::<AudioEmitter>();
        }
    }
}

/// Processes `StartRecording` trigger components.
///
/// Calls `engine.sampler().start_recording()` with the current transport beat,
/// replaces the trigger with `RecordingActive`.
#[cfg(feature = "sampler")]
pub fn recording_start_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    transport: Res<crate::TransportState>,
    query: Query<(Entity, &StartRecording), Added<StartRecording>>,
) {
    let Some(engine) = engine else { return };

    for (entity, start) in query.iter() {
        match engine
            .sampler()
            .start_recording(start.channel_index, start.source, start.mode, transport.beat)
        {
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
/// Calls `engine.sampler().stop_recording()`, removes `RecordingActive`,
/// and logs the result. The `RecordedData` is available in the log;
/// for programmatic access, use the direct sampler API.
#[cfg(feature = "sampler")]
pub fn recording_stop_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &StopRecording), Added<StopRecording>>,
    active_query: Query<(Entity, &RecordingActive)>,
) {
    let Some(engine) = engine else { return };

    for (entity, stop) in query.iter() {
        match engine.sampler().stop_recording(stop.channel_index) {
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
pub struct RecordingResult(pub tutti::RecordedData);
