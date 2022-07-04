#[derive(Clone, Copy, Debug)]
#[repr(u32)]
#[allow(unused)]
pub enum Note {
    A = 0,
    As,
    B,
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
}

impl Note {
    pub fn ratio(self) -> f32 {
        let semitones = self as u32;

        let ratio = 2.0f32.powf(1f32 / 12.);
        ratio.powf(semitones as f32)
    }

    pub fn freq(self, octave: u32) -> f32 {
        440f32 * 2f32.powf(octave as f32 - 4.) * self.ratio()
    }
}
