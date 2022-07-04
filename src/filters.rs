use std::{
    cell::Cell,
    f32::consts::PI,
    fs::OpenOptions,
    io::{self, BufWriter},
};

use wav::BitDepth;

pub struct SynthBuilder<S: 'static + Filter + Send, T: Filter>(S, T);

impl<S: 'static + Filter + Send> SynthBuilder<S, NoopFilter> {
    pub fn new(synth: S) -> Self {
        SynthBuilder(synth, NoopFilter)
    }

}
impl<S: 'static + Filter + Send, T: Filter> SynthBuilder<S, T> {
    pub fn chain<F: Filter>(self, filter: F) -> SynthBuilder<S, Chain<F, T>> {
        SynthBuilder(self.0, Chain(filter, self.1))
    }

    pub fn build(self) -> Synth<S, T> {
        Synth {synth: self.0, filter: self.1}
    }
}

pub const SAMPLING_FREQ: usize = 44100;

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
        self.samples.resize(new_len, 0.);
        self.pos %= new_len.min(self.samples.len());
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
    pub gain: f32,
}

impl LowPass {
    fn new(gain: f32) -> Self {
        Self {
            last: 0.,
            // stop you from making it have >unity gain
            gain: gain.min(0.499),
        }
    }
}

impl Default for LowPass {
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl Filter for LowPass {
    fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            let s2 = *s;
            *s = (self.last + s2) * self.gain;
            self.last = s2;
        }
    }
}

// group samples into a window of size n
// y(n) = c1x(n) + c2x(n - 1) + ...
// sum(cx | x <- [1..n]) <= 1
// can express as a dot product of two matrices of coeffs to inputs
// to achieve an FIR, take in a desired frequency response curve, perform an inverse FFT to get the individual coeffs
// sample frequency response at given rate, e.g.
// issue: how to implement phase delay at the fundamental frequency

// frequency sampling implementation of FIR
pub struct FIR {
    omegas: Vec<(f32, f32)>, // (initial freq resp(omega_k), omega_k)
    freq0: f32,
    coeff: f32,
}

impl FIR {
    pub fn new(taps: usize, freq_resp_curve: impl Fn(f32) -> f32) -> Self {
        debug_assert!(taps > 0);

        // since freq response is symmetrical around the origin
        let m = taps * 2 + 1;

        // freq_resp_curve(omegak) = H(omegak)

        let omegas = (1..=taps)
            .map(|n| {
                let omega_k = n as f32 * PI / m as f32;
                let resp_out = freq_resp_curve(omega_k);

                (resp_out, omega_k)
            })
            .collect();
        let freq0 = freq_resp_curve(0.);

        Self {
            omegas,
            freq0,
            coeff: 1.0 / m as f32,
        }
        // todo!()
    }
}

impl Filter for FIR {
    fn process(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            let comp_sum = self
                .omegas
                .iter()
                .map(|&(resp, omega_k)| resp * (omega_k * *sample).cos())
                .sum::<f32>();
            *sample = self.coeff * (self.freq0 + 2. * comp_sum);
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

pub struct NoopFilter;

impl Filter for NoopFilter {
    fn process(&mut self, samples: &mut [f32]) {}
}

pub struct Synth<S: 'static + Filter + Send, F: Filter = NoopFilter> {
    pub synth: S,
    pub filter: F
}

impl<S: 'static + Filter + Send, F: Filter> Filter for Synth<S, F> {
    fn process(&mut self, samples: &mut [f32]) {
        self.synth.process(samples);
        self.filter.process(samples);
    }
}

pub struct Chain<H: Filter, T: Filter>(pub H, pub T);

impl<H: Filter, T: Filter> Filter for Chain<H, T> {
    fn process(&mut self, samples: &mut [f32]) {
        self.1.process(samples);
        self.0.process(samples);
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
    pub fn tune(&mut self, freq: f32) {
        // FIXME: not perfect; will be slightly out of tune until we implement
        // fractional delays
        self.delay
            .set_len((SAMPLING_FREQ as f32 / freq).round() as usize);
    }

    pub fn new(depth: usize) -> StringSynth {
        StringSynth {
            delay: DelayLine::new(depth),
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
