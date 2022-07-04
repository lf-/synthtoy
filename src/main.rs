mod filters;
mod note;
mod wavetable;

use filters::*;
use wavetable::*;

use note::key_to_freq;
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::{Event, EventType},
    keyboard::Keycode,
};

struct SDLShim<T: Filter>(T);

impl<T: Filter + Send> AudioCallback for SDLShim<T> {
    type Channel = f32;

    fn callback(&mut self, samples: &mut [Self::Channel]) {
        self.0.process(samples);
    }
}

fn main() {
    let ctx = sdl2::init().unwrap();
    let audio = ctx.audio().unwrap();
    let video = ctx.video().unwrap();
    let mut pump = ctx.event_pump().unwrap();
    pump.enable_event(EventType::KeyDown);

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
    let synth = SynthBuilder::new()
        .chain(StringSynth::new(500))
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
                    let delay = &mut lock.0 .1 .0.delay;
                    delay.set_len(delay.len() + 5);
                    println!("len: {}", delay.len());
                }
                Keycode::I => {
                    let mut lock = dev.lock();
                    let delay = &mut lock.0 .1 .0.delay;
                    delay.set_len((delay.len() - 5).max(1));
                    println!("len: {}", delay.len());
                }
                Keycode::Q => {
                    break;
                }
                Keycode::G => {
                    let mut lock = dev.lock();
                    // todo fix this by setting trigger on Filter::process
                    lock.0 .1 .0.trigger_count = 50;
                }
                Keycode::S => {
                    let lock = dev.lock();
                    lock.0 .1 .0.snoop.save().unwrap();
                    // lock.0.snoop.save().unwrap();
                }
                &k => {
                    if let Some(n) = key_to_freq(k) {
                        let mut lock = dev.lock();
                        lock.0 .1 .0.tune(n);
                        lock.0 .1 .0.trigger_count = 50;
                    }
                }
            },
            _ => {}
        }
    }
}
