//! Neural subsystem health mirror as a Bevy resource.

use bevy_ecs::prelude::*;

/// Exposes neural subsystem health / performance to the UI.
#[derive(Resource, Default, Debug)]
pub struct NeuralStatusResource {
    pub is_enabled: bool,
    pub has_gpu: bool,
    pub is_healthy: bool,
    pub inference_avg_us: f32,
    pub inference_peak_us: f32,
    pub model_count: u32,
}

pub fn neural_status_sync_system(
    neural: Option<Res<crate::NeuralRes>>,
    mut status: ResMut<NeuralStatusResource>,
) {
    let Some(neural) = neural else {
        status.is_enabled = false;
        return;
    };
    let metrics = neural.0.meter().snapshot();
    status.is_enabled = true;
    // has_gpu is a property of the Backend, which lives on the engine
    // thread. Track through a dedicated command if needed; for now the
    // status panel doesn't surface this.
    status.has_gpu = false;
    status.is_healthy = neural.0.is_healthy();
    status.inference_avg_us = metrics.inference.average.as_micros() as f32;
    status.inference_peak_us = metrics.inference.peak.as_micros() as f32;
    status.model_count = metrics.batch.model_count;
}
