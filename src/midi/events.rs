#[cfg(feature = "midi")]
use bevy_ecs::prelude::*;
#[cfg(feature = "midi")]
use bevy_ecs::message::Message;

#[cfg(feature = "midi")]
use tutti::MidiEvent;

/// Fired every frame for each MIDI event received from hardware input.
#[cfg(feature = "midi")]
#[derive(Event, Message, Clone, Debug)]
pub struct MidiInputEvent(pub MidiEvent);

#[cfg(feature = "midi")]
impl MidiInputEvent {
    #[inline]
    pub fn is_note_on(&self) -> bool {
        self.0.is_note_on()
    }

    #[inline]
    pub fn is_note_off(&self) -> bool {
        self.0.is_note_off()
    }

    #[inline]
    pub fn note(&self) -> Option<u8> {
        self.0.note()
    }

    #[inline]
    pub fn velocity(&self) -> Option<u8> {
        self.0.velocity()
    }

    #[inline]
    pub fn channel(&self) -> u8 {
        self.0.channel_num()
    }

    #[inline]
    pub fn event(&self) -> &MidiEvent {
        &self.0
    }
}

#[cfg(feature = "midi-hardware")]
#[derive(Event, Message, Clone, Debug)]
pub enum MidiDeviceEvent {
    Connected { name: String },
    Disconnected,
}
