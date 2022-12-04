use std::str::FromStr;
use std::sync::mpsc;

pub mod audio_thread;
pub mod filters;
pub mod midi;
pub mod note;
pub mod wavetable;

use audio_thread::{AudioEvent, AudioSubsystemCrimesWrapper};
use midi::{initialize_midi, MidiDevice, MidiEvent};

use clap::{builder::ValueParser, Parser};
use note::key_to_freq;
use sdl2::{
    event::{Event, EventType},
    keyboard::Keycode,
};

type Error = Box<dyn std::error::Error + 'static>;

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

    let (send_audio, recv_audio) = mpsc::channel();

    let win = video.window("synthtoy", 200, 200);
    let mut win = win.build().unwrap();
    win.show();

    let _audio_thread = {
        let crime = AudioSubsystemCrimesWrapper(audio);
        std::thread::spawn(move || {
            audio_thread::audio_thread(crime, recv_audio);
        });
    };

    let _midi = args.midi_device.map({
        let send_audio = send_audio.clone();
        move |d| initialize_midi(d, send_audio)
    });

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
                Keycode::O => {}
                Keycode::I => {}
                Keycode::Q => {
                    send_audio.send(AudioEvent::Terminate).unwrap();
                    break;
                }
                Keycode::G => {}
                Keycode::S => {
                    // let lock = dev.lock();
                    // lock.0.synth.snoop.save().unwrap();
                    // lock.0.snoop.save().unwrap();
                }
                &k => {
                    if let Some(n) = key_to_freq(k) {
                        send_audio.send(AudioEvent::PlayNote(n))?;
                    }
                }
            },
            _ => {}
        }
    }
    Ok(())
}
