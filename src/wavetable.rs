const PERIOD_SAMPLE_SIZE: usize = 4096;

pub type WaveLookupTable = [f32; PERIOD_SAMPLE_SIZE];

pub trait WavetableSource {
    fn sample(&self, index: usize) -> f32;
}
static SIN_VALUES: WaveLookupTable = include!("../include/sin_table.txt");
static TRIANGLE_VALUES: WaveLookupTable = include!("../include/triangle_table.txt");

struct SquareWave;

impl WavetableSource for SquareWave {
    // period = 4096 steps = 2 pi
    fn sample(&self, index: usize) -> f32 {
        if index % PERIOD_SAMPLE_SIZE < (PERIOD_SAMPLE_SIZE / 2) {
            -1.
        } else {
            1.
        }
    }
}

macro_rules! impl_lookup {
    ($(($name:ident, $table:ident)),* $(,)*) => {
        $(
            struct $name;

            impl WavetableSource for $name {
                fn sample(&self, index: usize) -> f32 {
                    $table[index % PERIOD_SAMPLE_SIZE]
                }
            }
        )*
    }
}

impl_lookup! {
    (TriangleWave, TRIANGLE_VALUES),
    (SineWave, SIN_VALUES)
}

pub struct WaveTable<Wave: WavetableSource>(Wave);

impl<W: WavetableSource> WaveTable<W> {
    pub fn new(w: W) -> Self {
        Self(w)
    }
}

// todo implement
// impl<W: WavetableSource> Filter for WaveTable<W> {}
