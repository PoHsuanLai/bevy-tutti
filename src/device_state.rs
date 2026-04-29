use bevy_ecs::prelude::*;

use crate::{AudioConfig, TuttiDriverRes};

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
    driver: Option<Res<TuttiDriverRes>>,
    config: Option<Res<AudioConfig>>,
    mut state: ResMut<AudioDeviceState>,
) {
    let Some(driver) = driver else { return };
    if let Ok(d) = driver.0.lock() {
        state.is_running = d.is_running();
    }
    if let Some(cfg) = config {
        state.channels = cfg.channels;
    }
}

/// One-shot startup system: enumerate devices once.
pub fn device_state_init_system(
    driver: Option<Res<TuttiDriverRes>>,
    mut state: ResMut<AudioDeviceState>,
) {
    let Some(driver) = driver else { return };

    if let Ok(d) = driver.0.lock() {
        if let Ok(name) = d.device_name() {
            state.current_device = name;
        }
    }
    if let Ok(devices) = tutti::TuttiDriver::devices() {
        state.output_devices = devices.map(|d| d.name).collect();
    }
}
