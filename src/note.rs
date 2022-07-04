use sdl2::keyboard::Keycode;

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

pub fn key_to_freq(kc: Keycode) -> Option<f32> {
    macro_rules! keys {
        ($(($a:ident, $b:ident, $oct:expr));* $(;)*) => {
            match kc {
                $(Keycode::$a => Some((Note::$b).freq($oct)),)*
                _ => None,
            }
        };
    }

    keys! {
        (Z, A, 4);
        (X, B, 4);
        (C, C, 4);
        (V, D, 4);
        (B, E, 4);
        (N, F, 4);
        (M, G, 4);
        (Comma, A, 5);
        (Period, B, 5);
        (Slash, C, 5);
    }
}
