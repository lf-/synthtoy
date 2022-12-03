use std::str::FromStr;
use std::sync::mpsc;

mod filters;
mod note;
mod wavetable;

use filters::*;
use midir::MidiInputConnection;
use wavetable::*;

use clap::{builder::ValueParser, Parser};
use note::key_to_freq;
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::{Event, EventType},
    keyboard::Keycode,
};

type Error = Box<dyn std::error::Error + 'static>;

struct SDLShim<T: Filter>(T);

impl<T: Filter + Send> AudioCallback for SDLShim<T> {
    type Channel = f32;

    fn callback(&mut self, samples: &mut [Self::Channel]) {
        self.0.process(samples);
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum MidiDevice {
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

#[derive(Clone, Debug, clap::Parser)]
struct Args {
    /// MIDI device to get input from. Can be "virtual" to create a virtual
    /// midi input device that can be sent to from other software.
    #[clap(long, value_parser = ValueParser::new(MidiDevice::from_str))]
    midi_device: Option<MidiDevice>,

    /// Lists midi devices then exits.
    #[clap(long)]
    midi_list: bool,
}

#[derive(Clone, Copy, Debug)]
struct MidiEvent {
    timestamp: u64,
    channel: u8,
    inner: MidiEventInner,
}

#[derive(Clone, Copy, Debug)]
enum MidiEventInner {
    Down { velocity: u8, note: u8 },
    Up { velocity: u8, note: u8 },
    KeyPressure { key: u8, pressure: u8 },
}

fn parse_midi(timestamp: u64, midi: &[u8]) -> Option<MidiEvent> {
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

fn initialize_midi(
    dev: MidiDevice,
    send_midi: mpsc::Sender<MidiEvent>,
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
                            send_midi.send(ev).unwrap();
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

fn main() -> Result<(), Error> {
    let args = Args::parse();

    if args.midi_list {
        let input = midir::MidiInput::new("synthtoy")?;
        for port in input.ports() {
            let name = input.port_name(&port)?;
            println!("port: {:?}", name);
        }
        return Ok(());
    }
    run(args)
}

fn run(args: Args) -> Result<(), Error> {
    let ctx = sdl2::init().unwrap();
    let audio = ctx.audio().unwrap();
    let video = ctx.video().unwrap();
    let event = ctx.event()?;
    event.register_custom_event::<MidiEvent>()?;
    let mut pump = ctx.event_pump().unwrap();
    pump.enable_event(EventType::KeyDown);

    let (send_midi, recv_midi) = mpsc::channel();

    let spec = AudioSpecDesired {
        freq: Some(SAMPLING_FREQ as i32),
        channels: Some(1),
        samples: None,
    };

    let freq_curve = move |x: f32| {
        if x <= 1000. {
            1.
        } else {
            0.
        }
    };
    let synth = SynthBuilder::new(StringSynth::new(500))
        // .chain(NoopFilter)
        .chain(FIR::new(25, freq_curve))
        .build();

    let mut dev = audio
        .open_playback(None, &spec, |_spec| SDLShim(synth))
        .unwrap();

    dev.resume();

    let win = video.window("synthtoy", 200, 200);
    let mut win = win.build().unwrap();
    win.show();

    let _midi = args.midi_device.map(|d| initialize_midi(d, send_midi));

    loop {
        let ev = pump.wait_event();
        match &ev {
            Event::Quit { .. } => {
                break;
            }
            Event::KeyDown {
                keycode: Some(keycode),
                ..
            } => match keycode {
                Keycode::O => {
                    let mut lock = dev.lock();
                    let delay = &mut lock.0.synth.delay;
                    delay.set_len(delay.len() + 5);
                    println!("len: {}", delay.len());
                }
                Keycode::I => {
                    let mut lock = dev.lock();
                    let delay = &mut lock.0.synth.delay;
                    delay.set_len((delay.len() - 5).max(1));
                    println!("len: {}", delay.len());
                }
                Keycode::Q => {
                    break;
                }
                Keycode::G => {
                    let mut lock = dev.lock();
                    // todo fix this by setting trigger on Filter::process
                    lock.0.synth.trigger_count = 50;
                }
                Keycode::S => {
                    let lock = dev.lock();
                    lock.0.synth.snoop.save().unwrap();
                    // lock.0.snoop.save().unwrap();
                }
                &k => {
                    if let Some(n) = key_to_freq(k) {
                        let mut lock = dev.lock();
                        let synth = &mut lock.0.synth;
                        synth.tune(n);
                        synth.trigger_count = 50;
                    }
                }
            },
            _ => {}
        }
    }
    Ok(())
}
