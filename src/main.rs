use std::{
    cell::{Cell, RefCell},
    fs::OpenOptions,
    io::{self, BufWriter},
    thread,
    time::Duration,
};

use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::{Event, EventType},
    keyboard::Keycode,
};
use wav::BitDepth;

trait Filter: 'static + Send {
    fn process(&mut self, samples: &mut [f32]);
}

struct DelayLine {
    samples: Vec<f32>,
    pos: usize,
}

impl DelayLine {
    fn new(len: usize) -> DelayLine {
        let mut samples = Vec::new();
        samples.resize(len, 0.);
        DelayLine { samples, pos: 0 }
    }
}

impl Filter for DelayLine {
    fn process(&mut self, inout_samples: &mut [f32]) {
        for s in inout_samples.iter_mut() {
            self.samples[self.pos] = *s;
            *s = self.samples[(self.pos + 1) % self.samples.len()];
            self.pos = (self.pos + 1) % self.samples.len();
        }
    }
}

struct LowPass {
    last: f32,
}

impl Default for LowPass {
    fn default() -> Self {
        Self { last: 0. }
    }
}

impl Filter for LowPass {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            let s2 = *s;
            // so that gain is less than unity
            *s = (self.last + s2) * 0.4;
            self.last = s2;
        }
    }
}

struct Pipe {
    components: Vec<Box<dyn Filter>>,
}

impl Pipe {
    fn new(components: Vec<Box<dyn Filter>>) -> Pipe {
        Self { components }
    }
}

impl Default for Pipe {
    fn default() -> Self {
        Pipe {
            components: Vec::new(),
        }
    }
}

impl Pipe {
    fn add<T>(&mut self, comp: T)
    where
        T: Filter,
    {
        self.components.push(Box::new(comp));
    }
}

impl Filter for Pipe {
    fn process(&mut self, samples: &mut [f32]) {
        for comp in self.components.iter_mut() {
            comp.process(samples);
        }
    }
}

struct Snoop {
    name: String,
    samples: Cell<Vec<f32>>,
}

impl Snoop {
    fn new(name: String) -> Snoop {
        Snoop {
            name,
            samples: Default::default(),
        }
    }

    fn save(&self) -> io::Result<()> {
        let header = wav::Header::new(wav::header::WAV_FORMAT_IEEE_FLOAT, 1, 44100, 32);
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.name)?;
        let mut writer = BufWriter::new(file);

        let fmt = BitDepth::ThirtyTwoFloat(self.samples.take());
        wav::write(header, &fmt, &mut writer)
    }
}

impl Filter for Snoop {
    fn process(&mut self, samples: &mut [f32]) {
        self.samples.get_mut().extend(samples.iter());
    }
}

/// Splits an incoming stream into N pieces and then joins them back after
/// running the components over the input samples provided.
struct SplitJoin(Vec<Box<dyn Filter>>);

impl Filter for SplitJoin {
    fn process(&mut self, samples: &mut [f32]) {
        let mut copies = Vec::new();
        copies.resize_with(self.0.len(), || samples.to_vec());

        for (comp, inputs) in self.0.iter_mut().zip(copies.iter_mut()) {
            comp.process(inputs);
        }

        for (idx, s) in samples.iter_mut().enumerate() {
            *s = 0.;
            for outp in copies.iter() {
                *s += outp[idx];
            }
        }
    }
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

const STRING_SYNTH_DEPTH: usize = 2000;

impl Filter for SquareWave {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = if self.phase > 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

struct Rng {
    v: u32,
}

impl Rng {
    fn next(&mut self) -> f32 {
        self.v ^= self.v << 13;
        self.v ^= self.v >> 17;
        self.v ^= self.v << 5;

        // convert to full scale
        self.v as i32 as f32 / i32::MAX as f32
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng { v: 0xdeadbeef }
    }
}

struct Scale(f32);

impl Filter for Scale {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s *= self.0;
        }
    }
}

struct StringSynth {
    delay: DelayLine,
    lpf: LowPass,
    snoop: Snoop,

    last: f32,

    /// number of samples of noise burst remaining
    trigger_count: u32,

    rng: Rng,
}

impl StringSynth {
    fn new() -> StringSynth {
        StringSynth {
            delay: DelayLine::new(STRING_SYNTH_DEPTH),
            lpf: LowPass::default(),
            rng: Rng::default(),
            snoop: Snoop::new("string.wav".to_string()),
            last: 0.,
            trigger_count: 0,
        }
    }
}

impl Filter for StringSynth {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            let loop_in = if self.trigger_count > 0 {
                self.trigger_count -= 1;
                self.rng.next() + self.last
            } else {
                self.last
            };

            let mut samp = [loop_in];
            self.delay.process(&mut samp);
            self.lpf.process(&mut samp);
            self.snoop.process(&mut samp);
            self.last = samp[0];
            *s = self.last;
        }
    }
}

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
        freq: Some(44100),
        channels: Some(1),
        samples: None,
    };

    let mut dev = audio
        .open_playback(None, &spec, |_spec| SDLShim(StringSynth::new()))
        .unwrap();

    dev.resume();

    let win = video.window("synthtoy", 200, 200);
    let mut win = win.build().unwrap();
    win.show();

    loop {
        let ev = pump.wait_event();
        dbg!(&ev);
        match &ev {
            Event::Quit { .. } => {
                break;
            }
            Event::KeyDown {
                keycode: Some(keycode),
                ..
            } => match keycode {
                Keycode::Q => {
                    break;
                }
                Keycode::G => {
                    let mut lock = dev.lock();
                    lock.0.trigger_count = 5000;
                }
                Keycode::S => {
                    let lock = dev.lock();
                    lock.0.snoop.save().unwrap();
                }
                _ => {}
            },
            _ => {}
        }
    }
}
