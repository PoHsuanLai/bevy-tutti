use bevy_ecs::prelude::*;

use crate::SamplerRes;

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
    sampler: Option<Res<SamplerRes>>,
    enable_query: Query<
        (Entity, &crate::components::EnableAudioInput),
        Added<crate::components::EnableAudioInput>,
    >,
    disable_query: Query<Entity, Added<crate::components::DisableAudioInput>>,
) {
    let Some(sampler) = sampler else { return };
    let input = sampler.0.audio_input();

    for (entity, enable) in enable_query.iter() {
        if let Some(device_index) = enable.device_index {
            if let Err(e) = input.select_device(device_index) {
                bevy_log::error!(
                    "Failed to select audio input device {}: {}",
                    device_index,
                    e
                );
            }
        }

        input.set_gain(enable.gain);
        input.set_monitoring(enable.monitoring);

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
        input.set_monitoring(false);

        commands
            .entity(entity)
            .remove::<crate::components::DisableAudioInput>();
        bevy_log::info!("Audio input monitoring disabled");
    }
}

pub fn audio_input_sync_system(
    sampler: Option<Res<SamplerRes>>,
    mut state: ResMut<AudioInputState>,
) {
    let Some(sampler) = sampler else { return };
    state.peak_level = sampler.0.audio_input().peak_level();
}

/// One-shot startup: enumerate input devices once.
pub fn audio_input_init_system(
    sampler: Option<Res<SamplerRes>>,
    mut state: ResMut<AudioInputState>,
) {
    let Some(sampler) = sampler else { return };
    let devices = sampler.0.audio_input().list_input_devices();
    state.devices = devices
        .into_iter()
        .map(|d| AudioInputDeviceInfo {
            index: d.index,
            name: d.name,
        })
        .collect();
}
