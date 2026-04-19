//! Neural subsystem health mirror as a Bevy resource.

use bevy_ecs::prelude::*;

/// Exposes neural subsystem health / performance to the UI.
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
