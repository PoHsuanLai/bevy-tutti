//! Neural synth + effect playback triggers, plus the status mirror resource.

use bevy_app::{App, Plugin, Update};
use bevy_asset::{AssetApp, Assets, Handle};
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;

use crate::loader::TuttiStreamingLoader;
use crate::playback::AudioEmitter;
use crate::resources::{NeuralRes, TuttiGraphRes};

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
#[cfg(feature = "midi")]
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Clone)]
pub struct PlayNeuralSynth {
    pub source: Handle<tutti::neural::NeuralModel>,
}

#[cfg(feature = "midi")]
impl PlayNeuralSynth {
    pub fn new(source: Handle<tutti::neural::NeuralModel>) -> Self {
        Self { source }
    }
}

/// Trigger component: spawn an entity with this to create a neural effect.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Clone)]
pub struct PlayNeuralEffect {
    pub source: Handle<tutti::neural::NeuralModel>,
}

impl PlayNeuralEffect {
    pub fn new(source: Handle<tutti::neural::NeuralModel>) -> Self {
        Self { source }
    }
}

/// Exposes neural subsystem health / performance to the UI.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource, Default)]
pub struct NeuralStatusResource {
    pub is_enabled: bool,
    pub has_gpu: bool,
    pub is_healthy: bool,
    pub inference_avg_us: f32,
    pub inference_peak_us: f32,
    pub model_count: u32,
}

/// Processes `PlayNeuralSynth` trigger components, loads the neural model,
/// creates a `NeuralSynthNode` in tutti's graph with MIDI routing, and
/// attaches `AudioEmitter` to the entity.
#[cfg(feature = "midi")]
pub fn neural_synth_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<tutti::neural::NeuralModel>>,
    graph: Option<ResMut<TuttiGraphRes>>,
    neural: Option<Res<NeuralRes>>,
    query: Query<(Entity, &PlayNeuralSynth), Added<PlayNeuralSynth>>,
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
                let id = graph.0.master_boxed(unit);
                edited = true;
                commands
                    .entity(entity)
                    .remove::<PlayNeuralSynth>()
                    .insert(AudioEmitter { node_id: id });
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to create neural synth '{}': {}",
                    source.path.display(),
                    e
                );
                commands.entity(entity).remove::<PlayNeuralSynth>();
            }
        }
    }

    if edited {
        graph.0.commit();
    }
}

/// Processes `PlayNeuralEffect` trigger components, loads the neural model,
/// creates a `NeuralEffectNode` in tutti's graph, and attaches `AudioEmitter`.
pub fn neural_effect_playback_system(
    mut commands: Commands,
    model_assets: Res<Assets<tutti::neural::NeuralModel>>,
    graph: Option<ResMut<TuttiGraphRes>>,
    neural: Option<Res<NeuralRes>>,
    query: Query<(Entity, &PlayNeuralEffect), Added<PlayNeuralEffect>>,
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
                let id = graph.0.master_boxed(unit);
                edited = true;
                commands
                    .entity(entity)
                    .remove::<PlayNeuralEffect>()
                    .insert(AudioEmitter { node_id: id });
            }
            Err(e) => {
                bevy_log::error!(
                    "Failed to create neural effect '{}': {}",
                    source.path.display(),
                    e
                );
                commands.entity(entity).remove::<PlayNeuralEffect>();
            }
        }
    }

    if edited {
        graph.0.commit();
    }
}

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
        format!(
            "Unsupported neural model format: {}",
            source.path.display()
        ),
    )))
}

pub fn neural_status_sync_system(
    neural: Option<Res<NeuralRes>>,
    mut status: ResMut<NeuralStatusResource>,
) {
    let Some(neural) = neural else {
        status.is_enabled = false;
        return;
    };
    let metrics = neural.0.meter().snapshot();
    status.is_enabled = true;
    status.has_gpu = false;
    status.is_healthy = neural.0.is_healthy();
    status.inference_avg_us = metrics.inference.average.as_micros() as f32;
    status.inference_peak_us = metrics.inference.peak.as_micros() as f32;
    status.model_count = metrics.batch.model_count;
}

/// Bevy plugin: neural synth/effect playback + status sync.
pub struct TuttiNeuralPlugin;

impl Plugin for TuttiNeuralPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayNeuralEffect>()
            .register_type::<NeuralStatusResource>();
        #[cfg(feature = "midi")]
        app.register_type::<PlayNeuralSynth>();

        app.init_asset::<tutti::neural::NeuralModel>()
            .register_asset_loader(TuttiStreamingLoader::<tutti::neural::NeuralModel>::default())
            .init_resource::<NeuralStatusResource>()
            .add_systems(Update, neural_effect_playback_system)
            .add_systems(Update, neural_status_sync_system);

        #[cfg(feature = "midi")]
        app.add_systems(Update, neural_synth_playback_system);
    }
}
