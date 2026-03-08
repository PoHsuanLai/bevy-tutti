use bevy_asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;

/// Raw model bytes. Registration is deferred to playback time because
/// it requires a running `TuttiEngineResource`.
#[derive(Asset, TypePath, Clone)]
pub struct NeuralModelSource {
    bytes: Vec<u8>,
    pub name: String,
    pub path: std::path::PathBuf,
}

impl NeuralModelSource {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NeuralModelLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Default)]
pub struct NeuralModelLoader;

impl AssetLoader for NeuralModelLoader {
    type Asset = NeuralModelSource;
    type Settings = ();
    type Error = NeuralModelLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let name = load_context
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let path = load_context.path().to_path_buf();
        Ok(NeuralModelSource { bytes, name, path })
    }

    fn extensions(&self) -> &[&str] {
        &["mpk", "onnx"]
    }
}

/// Resource that exposes neural subsystem health to the UI.
#[derive(Resource, Default, Debug)]
pub struct NeuralStatusResource {
    pub is_enabled: bool,
    pub has_gpu: bool,
    pub is_healthy: bool,
    pub utilization: f32,
    pub inference_avg_us: f32,
    pub inference_peak_us: f32,
    pub queue_depth: u32,
    pub model_count: u32,
    pub overload_count: u64,
}

pub fn neural_status_sync_system(
    engine: Option<Res<crate::TuttiEngineResource>>,
    mut status: ResMut<NeuralStatusResource>,
) {
    let Some(engine) = engine else { return };
    let handle = engine.neural_status();
    if !handle.is_enabled() {
        status.is_enabled = false;
        return;
    }
    status.is_enabled = true;
    status.has_gpu = handle.has_gpu();
    status.is_healthy = handle.is_healthy();
    let metrics = handle.gpu_metrics();
    status.utilization = metrics.utilization;
    status.inference_avg_us = metrics.inference_average_us;
    status.inference_peak_us = metrics.inference_peak_us;
    status.queue_depth = metrics.queue_depth;
    status.model_count = metrics.model_count;
    status.overload_count = metrics.overload_count;
}
