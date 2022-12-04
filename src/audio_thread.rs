use std::sync::mpsc;

use sdl2::audio::{AudioCallback, AudioSpecDesired};

use crate::filters::{Filter, StringSynth, SynthBuilder, FIR, SAMPLING_FREQ};
use crate::midi::{MidiEvent, MidiEventInner};
use crate::note;

#[derive(Clone, Copy, Debug)]
pub enum AudioEvent {
    // FIXME: timestamping?
    Midi(MidiEvent),
    PlayNote(f32),
    Terminate,
}

struct SDLShim<T: Filter>(T);

impl<T: Filter + Send> AudioCallback for SDLShim<T> {
    type Channel = f32;

    fn callback(&mut self, samples: &mut [Self::Channel]) {
        self.0.process(samples);
    }
}

pub struct AudioSubsystemCrimesWrapper(pub sdl2::AudioSubsystem);

// SAFETY: crimes!
unsafe impl Send for AudioSubsystemCrimesWrapper {}

pub fn audio_thread(audio: AudioSubsystemCrimesWrapper, audio_recv: mpsc::Receiver<AudioEvent>) {
    let audio = audio.0;

    let freq_curve = move |x: f32| {
        if x <= 1000. {
            1.
        } else {
            0.
        }
    };

    let spec = AudioSpecDesired {
        freq: Some(SAMPLING_FREQ as i32),
        channels: Some(1),
        samples: None,
    };

    let synth = SynthBuilder::new(StringSynth::new(500))
        // .chain(NoopFilter)
        .chain(FIR::new(25, freq_curve))
        .build();

    let mut dev = audio
        .open_playback(None, &spec, |_spec| SDLShim(synth))
        .unwrap();

    dev.resume();

    loop {
        match audio_recv.recv().unwrap() {
            AudioEvent::Midi(MidiEvent { inner, .. }) => match inner {
                MidiEventInner::Down { velocity: _, note } => {
                    let freq = note::midi_note_to_freq(note);
                    let mut lock = dev.lock();
                    let synth = &mut lock.0.synth;
                    synth.tune(freq);
                    synth.trigger_count = 50;
                }
                _ => {}
            },
            AudioEvent::PlayNote(freq) => {
                let mut lock = dev.lock();
                let synth = &mut lock.0.synth;
                synth.tune(freq);
                synth.trigger_count = 50;
            }
            AudioEvent::Terminate => break,
        }
    }
}
