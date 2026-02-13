use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Audio device state synced from Tutti every frame.
#[derive(Resource, Debug, Clone)]
pub struct AudioDeviceState {
    pub output_devices: Vec<String>,
    pub current_device: String,
    pub is_running: bool,
    pub channels: usize,
}

impl Default for AudioDeviceState {
    fn default() -> Self {
        Self {
            output_devices: Vec::new(),
            current_device: String::new(),
            is_running: false,
            channels: 2,
        }
    }
}

pub fn device_state_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut state: ResMut<AudioDeviceState>,
) {
    let Some(engine) = engine else { return };

    state.is_running = engine.is_running();
    state.channels = engine.channels();

    if let Ok(name) = engine.current_output_device_name() {
        state.current_device = name;
    }

    if let Ok(devices) = tutti::TuttiEngine::list_output_devices() {
        if devices.len() != state.output_devices.len() {
            state.output_devices = devices;
        }
    }
}
