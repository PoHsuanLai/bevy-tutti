use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Master output peak/RMS levels, synced from Tutti every frame via lock-free atomics.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MasterMeterLevels {
    pub peak_left: f32,
    pub peak_right: f32,
    pub rms_left: f32,
    pub rms_right: f32,
}

pub fn metering_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut levels: ResMut<MasterMeterLevels>,
) {
    let Some(engine) = engine else { return };
    let (l_peak, r_peak, l_rms, r_rms) = engine.metering().amplitude();
    levels.peak_left = l_peak;
    levels.peak_right = r_peak;
    levels.rms_left = l_rms;
    levels.rms_right = r_rms;
}
