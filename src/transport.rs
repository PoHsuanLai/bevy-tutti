use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Transport state synced from Tutti every frame via lock-free atomics.
#[derive(Resource, Debug, Clone)]
pub struct TransportState {
    pub tempo: f64,
    pub beat: f64,
    pub is_playing: bool,
    pub is_paused: bool,
    pub is_recording: bool,
    pub is_looping: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub time_sig_numerator: u8,
    pub time_sig_denominator: u8,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            beat: 0.0,
            is_playing: false,
            is_paused: true,
            is_recording: false,
            is_looping: false,
            loop_start: 0.0,
            loop_end: 16.0,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
        }
    }
}

impl TransportState {
    pub fn beat(&self) -> f64 {
        self.beat
    }

    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping
    }

    pub fn loop_range(&self) -> (f64, f64) {
        (self.loop_start, self.loop_end)
    }
}

pub fn transport_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut state: ResMut<TransportState>,
) {
    let Some(engine) = engine else { return };
    let t = engine.transport();
    state.beat = t.current_beat();
    state.is_playing = t.is_playing();
    state.is_recording = t.is_recording();
    state.tempo = t.get_tempo() as f64;
    state.is_looping = t.is_loop_enabled();
    if let Some((start, end)) = t.get_loop_range() {
        state.loop_start = start;
        state.loop_end = end;
    }
}
