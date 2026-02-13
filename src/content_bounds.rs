use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Content duration bounds synced from Tutti every frame.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ContentBounds {
    pub end_beat: f64,
    /// Computed from end_beat and current tempo.
    pub duration_seconds: f64,
}

pub fn content_bounds_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut bounds: ResMut<ContentBounds>,
) {
    let Some(engine) = engine else { return };

    bounds.end_beat = engine.content_end_beat();
    bounds.duration_seconds = engine.content_duration();
}
