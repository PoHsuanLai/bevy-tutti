//! Bevy `Resource` wrappers around the flat `TuttiEngine` bundle.
//!
//! Each subsystem of the engine is surfaced as its own resource so systems
//! can take only the ones they need. The wrappers are thin newtypes; most
//! provide `Deref` to the inner handle.

use bevy_ecs::prelude::*;

#[cfg(any(feature = "sampler", feature = "soundfont", feature = "neural"))]
use std::sync::Arc;

#[cfg(feature = "midi")]
use tutti::midi_runtime::MidiBus;
#[cfg(feature = "midi-hardware")]
use tutti::midi::MidiIo;
#[cfg(feature = "neural")]
use tutti::neural::Engine as NeuralEngine;
use tutti::{TuttiDriver, TuttiGraph};

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
