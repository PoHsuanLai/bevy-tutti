use bevy_ecs::prelude::*;
use tutti::NodeId;

/// A single note within a [`MidiSequence`].
#[cfg(feature = "midi")]
#[derive(Clone, Debug)]
pub struct MidiSequenceNote {
    pub note: u8,
    pub velocity: u8,
    /// Start time in beats, relative to the sequence start.
    pub start: f64,
    /// Duration in beats.
    pub duration: f64,
}

/// Persistent MIDI sequence that fires note_on/note_off based on transport beat.
///
/// Ticked every frame by [`super::systems::midi_sequence_tick_system`].
#[cfg(feature = "midi")]
#[derive(Component)]
pub struct MidiSequence {
    pub target: NodeId,
    pub notes: Vec<MidiSequenceNote>,
    pub start_beat: f64,
    pub duration_beats: f64,
    pub loop_enabled: bool,
}

/// Routes hardware MIDI input to an audio graph node via `MidiRoutingTable`.
/// The routing table is rebuilt automatically when these components change.
#[cfg(feature = "midi")]
#[derive(Component)]
pub struct MidiReceiver {
    pub node_id: NodeId,
    /// MIDI channel filter. `None` = receive all channels.
    pub channel: Option<u8>,
}

/// One-shot trigger: connect to a MIDI input device by name (partial match).
#[cfg(feature = "midi-hardware")]
#[derive(Component)]
pub struct ConnectMidiDevice {
    pub name: String,
}

/// One-shot trigger: disconnect from the current MIDI input device.
#[cfg(feature = "midi-hardware")]
#[derive(Component)]
pub struct DisconnectMidiDevice;

/// Unlike `MidiReceiver`, routes all MIDI channels to one synth via
/// `table.fallback()` (standard MPE pattern).
#[cfg(feature = "mpe")]
#[derive(Component)]
pub struct MpeReceiver {
    pub node_id: NodeId,
}

/// One-shot trigger for MIDI 2.0 events (16-bit velocity, 32-bit CC,
/// per-note pitch bend). Removed after `midi2_send_system` processes it.
#[cfg(feature = "midi2")]
#[derive(Component)]
pub struct SendMidi2 {
    pub port: usize,
    pub events: Vec<tutti::Midi2Event>,
}

#[cfg(feature = "midi2")]
impl SendMidi2 {
    pub fn note_on(note: u8, velocity: f32, channel: u8) -> Self {
        let handle = tutti::Midi2Handle;
        Self {
            port: 0,
            events: vec![handle.note_on(note, velocity, channel)],
        }
    }

    pub fn note_off(note: u8, velocity: f32, channel: u8) -> Self {
        let handle = tutti::Midi2Handle;
        Self {
            port: 0,
            events: vec![handle.note_off(note, velocity, channel)],
        }
    }

    /// `bend` is normalized -1.0..1.0 (0.0 = center).
    pub fn per_note_pitch_bend(note: u8, bend: f32, channel: u8) -> Self {
        let handle = tutti::Midi2Handle;
        Self {
            port: 0,
            events: vec![handle.per_note_pitch_bend(note, bend, channel)],
        }
    }

    /// `value` is normalized 0.0..1.0 (mapped to 32-bit).
    pub fn cc(cc: u8, value: f32, channel: u8) -> Self {
        let handle = tutti::Midi2Handle;
        Self {
            port: 0,
            events: vec![handle.control_change(cc, value, channel)],
        }
    }

    pub fn events(port: usize, events: Vec<tutti::Midi2Event>) -> Self {
        Self { port, events }
    }
}
