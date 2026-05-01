use bevy_ecs::prelude::*;

/// Trigger component: spawn an entity with this to start recording on a channel.
///
/// The `recording_start_system` processes entities with `Added<StartRecording>`,
/// calls `engine.sampler().start_recording()`, replaces this component with
/// `RecordingActive`, and emits a `RecordingEvent::Started`.
#[derive(Component)]
pub struct StartRecording {
    pub channel_index: usize,
    pub source: tutti::sampler::capture::Source,
    pub mode: tutti::sampler::capture::Mode,
}

impl StartRecording {
    pub fn new(channel_index: usize, source: tutti::sampler::capture::Source) -> Self {
        Self {
            channel_index,
            source,
            mode: tutti::sampler::capture::Mode::Replace,
        }
    }

    pub fn mode(mut self, mode: tutti::sampler::capture::Mode) -> Self {
        self.mode = mode;
        self
    }
}

/// Trigger component: spawn or insert on an entity to stop recording on a channel.
///
/// The `recording_stop_system` processes entities with `Added<StopRecording>`,
/// calls `engine.sampler().stop_recording()`, removes `RecordingActive`,
/// and emits a `RecordingEvent::Stopped` with the recorded data.
#[derive(Component)]
pub struct StopRecording {
    pub channel_index: usize,
}

/// Marks an entity as having an active recording session.
///
/// Added automatically by `recording_start_system`. Removed when
/// `StopRecording` is processed or recording stops.
#[derive(Component)]
pub struct RecordingActive {
    pub channel_index: usize,
    pub source: tutti::sampler::capture::Source,
    pub mode: tutti::sampler::capture::Mode,
}
