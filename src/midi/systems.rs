#[cfg(feature = "midi")]
use bevy_ecs::prelude::*;
#[cfg(feature = "midi")]
use bevy_ecs::message::MessageWriter;
#[cfg(feature = "midi-hardware")]
use bevy_log::warn;

#[cfg(feature = "midi")]
use crate::TuttiEngineResource;

#[cfg(feature = "midi")]
use super::events::MidiInputEvent;

#[cfg(feature = "midi")]
use super::components::{MidiReceiver, SendMidi};

#[cfg(feature = "mpe")]
use super::components::MpeReceiver;

#[cfg(feature = "midi2")]
use super::components::SendMidi2;

#[cfg(feature = "midi-hardware")]
use super::components::{ConnectMidiDevice, DisconnectMidiDevice};
#[cfg(feature = "midi-hardware")]
use super::events::MidiDeviceEvent;

#[cfg(feature = "midi")]
#[derive(Resource)]
pub struct MidiInputObserver {
    pub(crate) receiver: crossbeam_channel::Receiver<tutti::MidiEvent>,
}

#[cfg(feature = "midi")]
#[derive(Resource)]
pub(crate) struct MidiObserverSender {
    pub(crate) sender: Option<crossbeam_channel::Sender<tutti::MidiEvent>>,
}

#[cfg(feature = "midi-hardware")]
#[derive(Resource, Default)]
pub struct MidiDeviceState {
    pub(crate) connected: Option<String>,
    pub(crate) last_check: Option<std::time::Instant>,
}

/// Must run after `TuttiEngineResource` is inserted.
#[cfg(feature = "midi")]
pub(crate) fn midi_observer_setup_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut sender_res: ResMut<MidiObserverSender>,
) {
    let Some(engine) = engine else { return };
    let Some(sender) = sender_res.sender.take() else {
        return;
    };

    #[cfg(feature = "midi-hardware")]
    {
        let midi_handle = engine.midi();
        if let Some(midi_system) = midi_handle.inner() {
            midi_system.set_ui_observer(sender);
        }
    }

    #[cfg(not(feature = "midi-hardware"))]
    {
        let _ = (engine, sender);
    }
}

#[cfg(feature = "midi")]
pub fn midi_input_event_system(
    observer: Option<Res<MidiInputObserver>>,
    mut writer: MessageWriter<MidiInputEvent>,
) {
    let Some(observer) = observer else { return };
    while let Ok(event) = observer.receiver.try_recv() {
        writer.write(MidiInputEvent(event));
    }
}

#[cfg(feature = "midi")]
pub fn midi_routing_sync_system(
    engine: Option<Res<TuttiEngineResource>>,
    changed: Query<&MidiReceiver, Changed<MidiReceiver>>,
    all_receivers: Query<&MidiReceiver>,
    mut removed: RemovedComponents<MidiReceiver>,
    #[cfg(feature = "mpe")] mpe_changed: Query<&MpeReceiver, Changed<MpeReceiver>>,
    #[cfg(feature = "mpe")] all_mpe_receivers: Query<&MpeReceiver>,
    #[cfg(feature = "mpe")] mut mpe_removed: RemovedComponents<MpeReceiver>,
) {
    let Some(engine) = engine else { return };

    #[allow(unused_mut)]
    let mut has_changes = !changed.is_empty() || removed.read().next().is_some();

    #[cfg(feature = "mpe")]
    {
        has_changes =
            has_changes || !mpe_changed.is_empty() || mpe_removed.read().next().is_some();
    }

    if !has_changes {
        return;
    }

    engine.midi_routing(|table| {
        table.clear();
        for receiver in all_receivers.iter() {
            let unit_id = receiver.node_id.value();
            if let Some(ch) = receiver.channel {
                table.channel(ch, unit_id);
            } else {
                table.fallback(unit_id);
            }
        }

        // MPE receivers route all channels to one synth via fallback
        #[cfg(feature = "mpe")]
        for mpe_recv in all_mpe_receivers.iter() {
            table.fallback(mpe_recv.node_id.value());
        }

        table.commit();
    });
}

#[cfg(feature = "midi")]
pub fn midi_send_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &SendMidi), Added<SendMidi>>,
) {
    let Some(engine) = engine else { return };

    for (entity, send) in query.iter() {
        engine.queue_midi(send.target, &send.events);
        commands.entity(entity).remove::<SendMidi>();
    }
}

#[cfg(feature = "midi-hardware")]
pub fn midi_device_connect_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    connect_query: Query<(Entity, &ConnectMidiDevice), Added<ConnectMidiDevice>>,
    disconnect_query: Query<Entity, Added<DisconnectMidiDevice>>,
    mut device_events: MessageWriter<MidiDeviceEvent>,
    mut state: ResMut<MidiDeviceState>,
) {
    let Some(engine) = engine else { return };
    let midi = engine.midi();

    for (entity, connect) in connect_query.iter() {
        match midi.connect_device_by_name(&connect.name) {
            Ok(()) => {
                state.connected = Some(connect.name.clone());
                device_events.write(MidiDeviceEvent::Connected {
                    name: connect.name.clone(),
                });
            }
            Err(e) => {
                warn!("Failed to connect MIDI device '{}': {}", connect.name, e);
            }
        }
        commands.entity(entity).remove::<ConnectMidiDevice>();
    }

    for entity in disconnect_query.iter() {
        midi.disconnect_device();
        if state.connected.is_some() {
            state.connected = None;
            device_events.write(MidiDeviceEvent::Disconnected);
        }
        commands.entity(entity).remove::<DisconnectMidiDevice>();
    }
}

/// Polls every 2s; fires `MidiDeviceEvent::Disconnected` if device disappears.
#[cfg(feature = "midi-hardware")]
pub fn midi_device_poll_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut state: ResMut<MidiDeviceState>,
    mut device_events: MessageWriter<MidiDeviceEvent>,
) {
    let Some(engine) = engine else { return };
    let now = std::time::Instant::now();

    if let Some(last) = state.last_check {
        if now.duration_since(last).as_secs() < 2 {
            return;
        }
    }
    state.last_check = Some(now);

    let midi = engine.midi();
    let currently_connected = midi
        .inner()
        .and_then(|s| s.connected_device_name());

    if state.connected.is_some() && currently_connected.is_none() {
        state.connected = None;
        device_events.write(MidiDeviceEvent::Disconnected);
    } else if let Some(name) = &currently_connected {
        state.connected = Some(name.clone());
    }
}

/// All reads are lock-free (atomic internally).
#[cfg(feature = "mpe")]
#[derive(Resource)]
pub struct MpeExpressionResource {
    pub(crate) handle: tutti::MpeHandle,
}

#[cfg(feature = "mpe")]
impl MpeExpressionResource {
    /// Combined per-note + global pitch bend. Normalized to -1.0..1.0.
    #[inline]
    pub fn pitch_bend(&self, note: u8) -> f32 {
        self.handle.pitch_bend(note)
    }

    /// Max of per-note and global pressure. Normalized to 0.0..1.0.
    #[inline]
    pub fn pressure(&self, note: u8) -> f32 {
        self.handle.pressure(note)
    }

    /// CC74 slide, normalized to 0.0..1.0 (defaults to 0.5).
    #[inline]
    pub fn slide(&self, note: u8) -> f32 {
        self.handle.slide(note)
    }

    #[inline]
    pub fn is_note_active(&self, note: u8) -> bool {
        self.handle.is_note_active(note)
    }

    pub fn mode(&self) -> tutti::MpeMode {
        self.handle.mode()
    }

    pub fn is_enabled(&self) -> bool {
        self.handle.is_enabled()
    }
}

#[cfg(feature = "mpe")]
pub(crate) fn mpe_setup_system(
    engine: Option<Res<TuttiEngineResource>>,
    mut commands: Commands,
) {
    if let Some(engine) = engine {
        commands.insert_resource(MpeExpressionResource {
            handle: engine.midi().mpe(),
        });
    }
}

#[cfg(feature = "midi2")]
pub fn midi2_send_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &SendMidi2), Added<SendMidi2>>,
) {
    let Some(engine) = engine else { return };
    let midi_handle = engine.midi();
    let Some(midi_system) = midi_handle.inner() else {
        return;
    };

    for (entity, send) in query.iter() {
        for event in &send.events {
            midi_system.push_midi2_event(send.port, *event);
        }
        commands.entity(entity).remove::<SendMidi2>();
    }
}
