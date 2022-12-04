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

impl TryFrom<u8> for Note {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Note::A),
            1 => Ok(Note::As),
            2 => Ok(Note::B),
            3 => Ok(Note::C),
            4 => Ok(Note::Cs),
            5 => Ok(Note::D),
            6 => Ok(Note::Ds),
            7 => Ok(Note::E),
            8 => Ok(Note::F),
            9 => Ok(Note::Fs),
            10 => Ok(Note::G),
            11 => Ok(Note::Gs),
            _ => Err("note outside range 0..=11"),
        }
    }
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

pub fn midi_note_to_freq(note: u8) -> f32 {
    match note {
        0..=21 => {
            println!("buggy midi device: extremely low note {note}");
            Note::A.freq(0)
        }
        n => {
            let relative_to_a0 = n - 21;
            let note = Note::try_from(relative_to_a0 % 12).unwrap();
            let octave = relative_to_a0 / 12;
            note.freq(octave as u32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(note: u8, expect_freq: f32) {
        let got = midi_note_to_freq(note);
        assert!(
            got - expect_freq < 1.,
            "Note {note} has wrong frequency, got {got}"
        );
    }
    #[test]
    fn test_midi_note_freq() {
        check(21, 27.5);
        check(22, 29.14);
        check(69, 440.);
    }
}
