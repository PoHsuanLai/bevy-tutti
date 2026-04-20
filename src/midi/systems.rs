#[cfg(feature = "midi")]
use bevy_ecs::message::MessageWriter;
#[cfg(feature = "midi")]
use bevy_ecs::prelude::*;
#[cfg(feature = "midi-hardware")]
use bevy_log::warn;

#[cfg(feature = "midi")]
use super::events::MidiInputEvent;

#[cfg(feature = "midi")]
use super::components::MidiReceiver;

#[cfg(feature = "mpe")]
use super::components::MpeReceiver;

#[cfg(feature = "midi-hardware")]
use super::components::{ConnectMidiDevice, DisconnectMidiDevice};
#[cfg(feature = "midi-hardware")]
use super::events::MidiDeviceEvent;

#[cfg(feature = "midi")]
#[derive(Resource)]
pub struct MidiInputObserver {
    pub(crate) receiver: crossbeam_channel::Receiver<tutti::midi::MidiEvent>,
}

#[cfg(feature = "midi")]
#[derive(Resource)]
pub(crate) struct MidiObserverSender {
    pub(crate) sender: Option<crossbeam_channel::Sender<tutti::midi::MidiEvent>>,
}

#[cfg(feature = "midi-hardware")]
#[derive(Resource, Default)]
pub struct MidiDeviceState {
    pub(crate) connected: Option<String>,
    pub(crate) last_check: Option<std::time::Instant>,
}

/// Sets up the UI observer on the hardware MIDI input port, funneling events
/// into `MidiInputObserver`'s channel. No-op when `midi-hardware` is disabled
/// (there's no hardware port to observe).
#[cfg(feature = "midi")]
pub(crate) fn midi_observer_setup_system(
    #[cfg(feature = "midi-hardware")] midi_io: Option<Res<crate::MidiIoRes>>,
    mut sender_res: ResMut<MidiObserverSender>,
) {
    let Some(sender) = sender_res.sender.take() else {
        return;
    };

    #[cfg(feature = "midi-hardware")]
    {
        let Some(midi_io) = midi_io else { return };
        midi_io.0.set_input_observer(sender);
    }

    #[cfg(not(feature = "midi-hardware"))]
    {
        let _ = sender;
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
    graph: Option<ResMut<crate::TuttiGraphRes>>,
    changed: Query<&MidiReceiver, Changed<MidiReceiver>>,
    all_receivers: Query<&MidiReceiver>,
    mut removed: RemovedComponents<MidiReceiver>,
    #[cfg(feature = "mpe")] mpe_changed: Query<&MpeReceiver, Changed<MpeReceiver>>,
    #[cfg(feature = "mpe")] all_mpe_receivers: Query<&MpeReceiver>,
    #[cfg(feature = "mpe")] mut mpe_removed: RemovedComponents<MpeReceiver>,
) {
    let Some(mut graph) = graph else { return };

    #[allow(unused_mut)]
    let mut has_changes = !changed.is_empty() || removed.read().next().is_some();

    #[cfg(feature = "mpe")]
    {
        has_changes = has_changes || !mpe_changed.is_empty() || mpe_removed.read().next().is_some();
    }

    if !has_changes {
        return;
    }

    let table = graph.0.midi_route_mut();
    table.clear();
    for receiver in all_receivers.iter() {
        let unit_id = tutti::core::MidiUnitId::new(receiver.node_id.value());
        if let Some(ch) = receiver.channel {
            table.channel(ch, unit_id);
        } else {
            table.fallback(unit_id);
        }
    }

    // MPE receivers route all channels to one synth via fallback
    #[cfg(feature = "mpe")]
    for mpe_recv in all_mpe_receivers.iter() {
        table.fallback(tutti::core::MidiUnitId::new(mpe_recv.node_id.value()));
    }

    // `commit()` on TuttiGraph publishes both graph edits and the MIDI
    // routing table snapshot in one step.
    graph.0.commit();
}

#[cfg(feature = "midi-hardware")]
pub fn midi_device_connect_system(
    mut commands: Commands,
    midi_io: Option<Res<crate::MidiIoRes>>,
    connect_query: Query<(Entity, &ConnectMidiDevice), Added<ConnectMidiDevice>>,
    disconnect_query: Query<Entity, Added<DisconnectMidiDevice>>,
    mut device_events: MessageWriter<MidiDeviceEvent>,
    mut state: ResMut<MidiDeviceState>,
) {
    let Some(midi_io) = midi_io else { return };

    for (entity, connect) in connect_query.iter() {
        match midi_io.0.connect_input_by_name(&connect.name) {
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
        midi_io.0.disconnect_input();
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
    midi_io: Option<Res<crate::MidiIoRes>>,
    mut state: ResMut<MidiDeviceState>,
    mut device_events: MessageWriter<MidiDeviceEvent>,
) {
    let Some(midi_io) = midi_io else { return };
    let now = std::time::Instant::now();

    if let Some(last) = state.last_check {
        if now.duration_since(last).as_secs() < 2 {
            return;
        }
    }
    state.last_check = Some(now);

    let currently_connected = midi_io.0.input_device_name();

    if state.connected.is_some() && currently_connected.is_none() {
        state.connected = None;
        device_events.write(MidiDeviceEvent::Disconnected);
    } else if let Some(name) = &currently_connected {
        state.connected = Some(name.clone());
    }
}

// =========================================================================
// MIDI Sequence playback
// =========================================================================

#[cfg(feature = "midi")]
use super::components::MidiSequence;

/// Tracks which notes are currently sounding for a [`MidiSequence`].
#[cfg(feature = "midi")]
#[derive(Component, Default)]
pub struct MidiSequenceState {
    active_notes: std::collections::HashSet<u8>,
}

/// Auto-inserts [`MidiSequenceState`] on entities that have [`MidiSequence`]
/// but not yet a state component.
#[cfg(feature = "midi")]
pub fn midi_sequence_setup_system(
    mut commands: Commands,
    query: Query<Entity, (With<MidiSequence>, Without<MidiSequenceState>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).insert(MidiSequenceState::default());
    }
}

/// Ticks all [`MidiSequence`] entities, firing note_on/note_off based on
/// the transport's current beat position.
#[cfg(feature = "midi")]
pub fn midi_sequence_tick_system(
    transport: Option<Res<crate::TransportRes>>,
    midi: Option<Res<crate::MidiBusRes>>,
    mut query: Query<(&MidiSequence, &mut MidiSequenceState)>,
) {
    let Some(transport) = transport else { return };
    let Some(midi) = midi else { return };

    if !transport.0.is_playing() {
        // All-notes-off when transport is not rolling
        for (seq, mut state) in query.iter_mut() {
            let unit_id = tutti::core::MidiUnitId::new(seq.target.value());
            for note in state.active_notes.drain() {
                let event = note_off_event(note);
                midi.0.queue(unit_id, &[event]);
            }
        }
        return;
    }

    let beat = transport.0.current_beat();

    for (seq, mut state) in query.iter_mut() {
        let unit_id = tutti::core::MidiUnitId::new(seq.target.value());
        let local_beat = if seq.loop_enabled && seq.duration_beats > 0.0 {
            let offset = beat - seq.start_beat;
            ((offset % seq.duration_beats) + seq.duration_beats) % seq.duration_beats
        } else {
            beat - seq.start_beat
        };

        // Outside range (non-looped)
        if !seq.loop_enabled && (local_beat < 0.0 || local_beat > seq.duration_beats) {
            for note in state.active_notes.drain() {
                let event = note_off_event(note);
                midi.0.queue(unit_id, &[event]);
            }
            continue;
        }

        // Determine which notes should be active at this beat
        let mut should_be_active = std::collections::HashSet::new();
        for n in &seq.notes {
            if local_beat >= n.start && local_beat < n.start + n.duration {
                should_be_active.insert(n.note);
            }
        }

        // Note-off for notes that ended
        for &note in &state.active_notes {
            if !should_be_active.contains(&note) {
                let event = note_off_event(note);
                midi.0.queue(unit_id, &[event]);
            }
        }

        // Note-on for newly active notes
        for n in &seq.notes {
            if should_be_active.contains(&n.note) && !state.active_notes.contains(&n.note) {
                let event = note_on_event(n.note, n.velocity);
                midi.0.queue(unit_id, &[event]);
            }
        }

        state.active_notes = should_be_active;
    }
}

/// Channel-0 MIDI 2.0 note-on event with a 7-bit MIDI 1 velocity
/// (upconverted to the 16-bit MIDI 2 velocity range).
#[cfg(feature = "midi")]
fn note_on_event(note: u8, velocity_midi1: u8) -> tutti::midi::MidiEvent {
    tutti::midi::MidiEvent::note_on(0, 0, note, (velocity_midi1 as u16) << 9)
}

#[cfg(feature = "midi")]
fn note_off_event(note: u8) -> tutti::midi::MidiEvent {
    tutti::midi::MidiEvent::note_off(0, 0, note, 0)
}

/// Placeholder MPE resource.
///
/// In the flat-bundle engine, MPE is wired into the `MidiProcessor`
/// pipeline at build time via `TuttiEngineBuilder::mpe(mode)`; there is no
/// external hook to read per-note expression back out yet. This resource
/// exposes zeroed defaults so consumers compile against the same shape
/// they used before, and will be replaced with a real `Arc<PerNoteExpression>`
/// handle once the engine surfaces one.
#[cfg(feature = "mpe")]
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MpeExpressionResource;

#[cfg(feature = "mpe")]
#[allow(dead_code)] // stub — real impl lands when the bundle exposes an MPE observer hook
impl MpeExpressionResource {
    pub fn pitch_bend(&self, _note: u8) -> f32 {
        0.0
    }

    pub fn pressure(&self, _note: u8) -> f32 {
        0.0
    }

    pub fn slide(&self, _note: u8) -> f32 {
        0.5
    }

    pub fn is_note_active(&self, _note: u8) -> bool {
        false
    }

    pub fn is_enabled(&self) -> bool {
        false
    }
}

/// MPE setup is a stub pending engine-side observer plumbing.
#[cfg(feature = "mpe")]
pub(crate) fn mpe_setup_system(mut commands: Commands) {
    commands.insert_resource(MpeExpressionResource);
}
