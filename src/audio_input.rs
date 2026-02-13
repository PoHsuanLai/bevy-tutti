use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

/// Audio input state synced from Tutti's sampler subsystem every frame.
#[derive(Resource)]
pub struct AudioInputState {
    pub peak_level: f32,
    pub devices: Vec<AudioInputDeviceInfo>,
}

impl Default for AudioInputState {
    fn default() -> Self {
        Self {
            peak_level: 0.0,
            devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioInputDeviceInfo {
    pub index: usize,
    pub name: String,
}

pub fn audio_input_control_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    enable_query: Query<
        (Entity, &crate::components::EnableAudioInput),
        Added<crate::components::EnableAudioInput>,
    >,
    disable_query: Query<Entity, Added<crate::components::DisableAudioInput>>,
) {
    let Some(engine) = engine else { return };

    for (entity, enable) in enable_query.iter() {
        let sampler = engine.sampler();

        if let Some(device_index) = enable.device_index {
            if let Err(e) = sampler.select_input_device(device_index) {
                bevy_log::error!("Failed to select audio input device {}: {}", device_index, e);
            }
        }

        sampler.set_input_gain(enable.gain);
        sampler.set_input_monitoring(enable.monitoring);

        commands
            .entity(entity)
            .remove::<crate::components::EnableAudioInput>();
        bevy_log::info!(
            "Audio input configured (device={:?}, gain={}, monitoring={})",
            enable.device_index,
            enable.gain,
            enable.monitoring
        );
    }

    for entity in disable_query.iter() {
        engine.sampler().set_input_monitoring(false);

        commands
            .entity(entity)
            .remove::<crate::components::DisableAudioInput>();
        bevy_log::info!("Audio input monitoring disabled");
    }
}

pub fn audio_input_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut state: ResMut<AudioInputState>,
) {
    let Some(engine) = engine else { return };
    let sampler = engine.sampler();

    state.peak_level = sampler.input_peak_level();

    let devices = sampler.list_input_devices();
    if state.devices.len() != devices.len() {
        state.devices = devices
            .into_iter()
            .map(|d| AudioInputDeviceInfo {
                index: d.index,
                name: d.name,
            })
            .collect();
    }
}
