use bevy_ecs::prelude::*;
use tutti::NodeId;

#[cfg(feature = "midi")]
use tutti::MidiEvent;

/// One-shot trigger component: removed after `midi_send_system` processes it.
#[cfg(feature = "midi")]
#[derive(Component)]
pub struct SendMidi {
    pub target: NodeId,
    pub events: Vec<MidiEvent>,
}

#[cfg(feature = "midi")]
impl SendMidi {
    pub fn note_on(target: NodeId, note: u8, velocity: u8) -> Self {
        Self {
            target,
            events: vec![MidiEvent::note_on(0, 0, note, velocity)],
        }
    }

    pub fn note_off(target: NodeId, note: u8) -> Self {
        Self {
            target,
            events: vec![MidiEvent::note_off(0, 0, note, 0)],
        }
    }

    pub fn cc(target: NodeId, channel: u8, cc: u8, value: u8) -> Self {
        Self {
            target,
            events: vec![MidiEvent::control_change(0, channel, cc, value)],
        }
    }

    pub fn pitch_bend(target: NodeId, channel: u8, value: u16) -> Self {
        Self {
            target,
            events: vec![MidiEvent::pitch_bend(0, channel, value)],
        }
    }

    pub fn events(target: NodeId, events: Vec<MidiEvent>) -> Self {
        Self { target, events }
    }
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
