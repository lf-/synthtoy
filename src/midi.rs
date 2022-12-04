use std::sync::mpsc;

use midir::MidiInputConnection;

use crate::{audio_thread::AudioEvent, Error};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum MidiDevice {
    Named(String),
    Virtual,
}

impl std::str::FromStr for MidiDevice {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Ok(match value {
            "virtual" => todo!("virtual midi devices not yet supported"), //MidiDevice::Virtual,
            _ => MidiDevice::Named(value.to_string()),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MidiEvent {
    pub timestamp: u64,
    pub channel: u8,
    pub inner: MidiEventInner,
}

#[derive(Clone, Copy, Debug)]
pub enum MidiEventInner {
    Down { velocity: u8, note: u8 },
    Up { velocity: u8, note: u8 },
    KeyPressure { key: u8, pressure: u8 },
}

pub fn parse_midi(timestamp: u64, midi: &[u8]) -> Option<MidiEvent> {
    let byte0 = midi[0];
    let cmd = (byte0 & 0xf0) >> 4;
    let channel = byte0 & 0xf;

    Some(MidiEvent {
        timestamp,
        channel,
        inner: match cmd {
            0x8 => MidiEventInner::Up {
                velocity: midi[2],
                note: midi[1],
            },
            0x9 => MidiEventInner::Down {
                velocity: midi[2],
                note: midi[1],
            },
            0xa => MidiEventInner::KeyPressure {
                key: midi[1],
                pressure: midi[2],
            },
            _ => {
                // println!("unk command: {}", cmd);
                return None;
            }
        },
    })
}

pub fn initialize_midi(
    dev: MidiDevice,
    send_midi: mpsc::Sender<AudioEvent>,
) -> Result<Option<MidiInputConnection<()>>, Error> {
    if let MidiDevice::Named(n) = dev {
        let mut the_port = None;
        let input = midir::MidiInput::new("synthtoy")?;
        for port in input.ports() {
            let name = input.port_name(&port)?;
            if name.starts_with(&n) {
                the_port = Some(port);
            }
        }

        if let Some(p) = the_port {
            Ok(Some(input.connect(
                &p,
                "synthtoy-in",
                {
                    let send_midi = send_midi;
                    move |ts, data, _| {
                        if let Some(ev) = parse_midi(ts, data) {
                            println!("{:?}", &ev);
                            send_midi.send(AudioEvent::Midi(ev)).unwrap();
                        }
                    }
                },
                (),
            )?))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}
