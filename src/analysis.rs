use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Live analysis state synced from Tutti via lock-free ArcSwap reads.
///
/// Fields are `Arc` pointers -- cheap to clone for UI consumption.
#[derive(Resource)]
pub struct LiveAnalysisData {
    pub pitch: Arc<tutti::PitchResult>,
    pub transients: Arc<Vec<tutti::Transient>>,
    pub waveform: Arc<tutti::WaveformSummary>,
    pub is_live: bool,
}

impl Default for LiveAnalysisData {
    fn default() -> Self {
        Self {
            pitch: Arc::new(tutti::PitchResult::default()),
            transients: Arc::new(Vec::new()),
            waveform: Arc::new(tutti::WaveformSummary::default()),
            is_live: false,
        }
    }
}

pub fn live_analysis_control_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    mut analysis: ResMut<LiveAnalysisData>,
    enable_query: Query<Entity, Added<crate::components::EnableLiveAnalysis>>,
    disable_query: Query<Entity, Added<crate::components::DisableLiveAnalysis>>,
) {
    let Some(engine) = engine else { return };

    for entity in enable_query.iter() {
        engine.enable_live_analysis();
        analysis.is_live = true;
        commands
            .entity(entity)
            .remove::<crate::components::EnableLiveAnalysis>();
        bevy_log::info!("Live analysis enabled");
    }

    for entity in disable_query.iter() {
        engine.disable_live_analysis();
        analysis.is_live = false;
        commands
            .entity(entity)
            .remove::<crate::components::DisableLiveAnalysis>();
        bevy_log::info!("Live analysis disabled");
    }
}

pub fn live_analysis_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut analysis: ResMut<LiveAnalysisData>,
) {
    if !analysis.is_live {
        return;
    }
    let Some(engine) = engine else { return };

    let handle = engine.analysis();
    analysis.pitch = handle.live_pitch();
    analysis.transients = handle.live_transients();
    analysis.waveform = handle.live_waveform();
}
