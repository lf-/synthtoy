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

pub trait Filter: 'static + Send {
    fn process(&mut self, samples: &mut [f32]);
}

pub struct DelayLine {
    pub samples: Vec<f32>,
    pub pos: usize,
}

impl DelayLine {
    pub fn new(len: usize) -> DelayLine {
        let samples = vec![0.; len];
        DelayLine { samples, pos: 0 }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn set_len(&mut self, new_len: usize) {
        self.samples = vec![0.; new_len];
        self.pos = new_len.min(self.samples.len()) - 1;
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

pub struct LowPass {
    pub last: f32,
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
            *s = (self.last + s2) * 0.499;
            self.last = s2;
        }
    }
}

#[derive(Default)]
pub struct Pipe {
    pub components: Vec<Box<dyn Filter>>,
}

impl Pipe {
    fn new(components: Vec<Box<dyn Filter>>) -> Pipe {
        Self { components }
    }

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

pub struct Snoop {
    pub name: String,
    pub samples: Cell<Vec<f32>>,
}

impl Snoop {
    pub fn new(name: String) -> Snoop {
        Snoop {
            name,
            samples: Default::default(),
        }
    }

    pub fn save(&self) -> io::Result<()> {
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
pub struct SplitJoin(Vec<Box<dyn Filter>>);

impl Filter for SplitJoin {
    fn process(&mut self, samples: &mut [f32]) {
        let mut copies = Vec::new();
        copies.resize_with(self.0.len(), || samples.to_vec());

        for (comp, inputs) in self.0.iter_mut().zip(copies.iter_mut()) {
            comp.process(inputs);
        }

        for (idx, s) in samples.iter_mut().enumerate() {
            *s = copies.iter().map(|o| o[idx]).sum();
        }
    }
}

pub struct SquareWave {
    pub phase_inc: f32,
    pub phase: f32,
    pub volume: f32,
}

const STRING_SYNTH_DEPTH: usize = 100;

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

pub struct Rng {
    pub v: u32,
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

pub struct Scale(f32);

impl Filter for Scale {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s *= self.0;
        }
    }
}

pub struct StringSynth {
    pub delay: DelayLine,
    pub lpf: LowPass,
    pub snoop: Snoop,

    pub last: f32,

    /// number of samples of noise burst remaining
    pub trigger_count: u32,

    pub rng: Rng,
}

impl StringSynth {
    pub fn new() -> StringSynth {
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
