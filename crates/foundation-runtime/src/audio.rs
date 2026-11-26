//! Platform-neutral PCM mixer shared by native Foundation hosts.
//!
//! The mixer is intentionally independent from SDL and operating-system APIs.
//! It preserves the audio contract needed by Street Chaves: 44.1 kHz S16 stereo,
//! 48 voices, one active voice per sample, oldest-voice stealing, looping and
//! saturating summation.

use std::sync::Arc;

pub const MIXER_RATE_HZ: u32 = 44_100;
pub const MIXER_CHANNELS: usize = 2;
pub const MAX_AUDIO_VOICES: usize = 48;
pub const MAX_MIXED_SAMPLES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleId(pub u64);

#[derive(Debug)]
struct Voice {
    sample_id: SampleId,
    pcm: Arc<[i16]>,
    position_frames: usize,
    looped: bool,
    sequence: u32,
}

/// Deterministic PCM mixer. Hosts own the device callback and call mix_into.
#[derive(Debug)]
pub struct AudioMixer {
    voices: [Option<Voice>; MAX_AUDIO_VOICES],
    next_sequence: u32,
    callback_buffer: Vec<i16>,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self {
            voices: std::array::from_fn(|_| None),
            next_sequence: 0,
            callback_buffer: vec![0; MAX_MIXED_SAMPLES],
        }
    }
}

impl AudioMixer {
    /// Starts a sample at frame zero. Replaying the same sample cuts the old voice.
    /// When the pool is full, the oldest sequence is stolen using Pascal's signed
    /// Cardinal-difference comparison, including its rollover behavior.
    pub fn play(&mut self, sample_id: SampleId, pcm: Arc<[i16]>, looped: bool) -> bool {
        if pcm.len() < MIXER_CHANNELS {
            return false;
        }

        for voice in &mut self.voices {
            if voice
                .as_ref()
                .map(|active| active.sample_id == sample_id)
                .unwrap_or(false)
            {
                *voice = None;
            }
        }

        let slot = self.voices.iter().position(Option::is_none).unwrap_or_else(|| {
            let mut oldest_index = 0;
            for candidate_index in 1..MAX_AUDIO_VOICES {
                let candidate = self.voices[candidate_index]
                    .as_ref()
                    .expect("the voice pool is full");
                let oldest = self.voices[oldest_index]
                    .as_ref()
                    .expect("the voice pool is full");
                if (candidate.sequence.wrapping_sub(oldest.sequence) as i32) < 0 {
                    oldest_index = candidate_index;
                }
            }
            oldest_index
        });

        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.voices[slot] = Some(Voice {
            sample_id,
            pcm,
            position_frames: 0,
            looped,
            sequence: self.next_sequence,
        });
        true
    }

    /// Stops every active voice belonging to a sample.
    pub fn stop(&mut self, sample_id: SampleId) {
        for voice in &mut self.voices {
            if voice
                .as_ref()
                .map(|active| active.sample_id == sample_id)
                .unwrap_or(false)
            {
                *voice = None;
            }
        }
    }

    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|voice| voice.is_some()).count()
    }

    /// Mixes interleaved stereo samples into a caller-owned buffer.
    pub fn mix_into(&mut self, output: &mut [i16]) {
        mix_voices_into(&mut self.voices, output);
    }

    /// Reuses an allocated buffer from the SDL real-time callback.
    pub(crate) fn render_callback_buffer(&mut self, sample_count: usize) -> &[i16] {
        self.callback_buffer.resize(sample_count, 0);
        let mixed_samples = sample_count.min(MAX_MIXED_SAMPLES);
        mix_voices_into(
            &mut self.voices,
            &mut self.callback_buffer[..mixed_samples],
        );
        self.callback_buffer[mixed_samples..].fill(0);
        &self.callback_buffer
    }
}

fn mix_voices_into(voices: &mut [Option<Voice>; MAX_AUDIO_VOICES], output: &mut [i16]) {
    output.fill(0);
    let frame_count = output.len() / MIXER_CHANNELS;

    for frame in 0..frame_count {
        let mut left = 0_i32;
        let mut right = 0_i32;

        for voice_slot in voices.iter_mut() {
            let mut ended = false;

            if let Some(voice) = voice_slot.as_mut() {
                let sample_frames = voice.pcm.len() / MIXER_CHANNELS;
                if sample_frames == 0 {
                    ended = true;
                } else {
                    if voice.position_frames >= sample_frames {
                        if voice.looped {
                            voice.position_frames = 0;
                        } else {
                            ended = true;
                        }
                    }

                    if !ended {
                        let offset = voice.position_frames * MIXER_CHANNELS;
                        left += i32::from(voice.pcm[offset]);
                        right += i32::from(voice.pcm[offset + 1]);
                        voice.position_frames += 1;
                        if voice.position_frames >= sample_frames {
                            if voice.looped {
                                voice.position_frames = 0;
                            } else {
                                ended = true;
                            }
                        }
                    }
                }
            }

            if ended {
                *voice_slot = None;
            }
        }

        output[frame * MIXER_CHANNELS] = left.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        output[frame * MIXER_CHANNELS + 1] = right.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    if output.len() % MIXER_CHANNELS != 0 {
        let last = output.len() - 1;
        output[last] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(frames: &[(i16, i16)]) -> Arc<[i16]> {
        Arc::from(
            frames
                .iter()
                .flat_map(|(left, right)| [*left, *right])
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn voices_mix_concurrently_and_saturate() {
        let mut mixer = AudioMixer::default();
        mixer.play(SampleId(1), pcm(&[(20_000, -20_000)]), false);
        mixer.play(SampleId(2), pcm(&[(20_000, -20_000)]), false);
        let mut output = [0; 2];
        mixer.mix_into(&mut output);
        assert_eq!(output, [i16::MAX, i16::MIN]);
        assert_eq!(mixer.active_voice_count(), 0);
    }

    #[test]
    fn replay_cuts_same_sample_and_restarts_at_zero() {
        let mut mixer = AudioMixer::default();
        let sample = pcm(&[(1, 10), (2, 20), (3, 30)]);
        mixer.play(SampleId(7), sample.clone(), false);
        let mut first = [0; 2];
        mixer.mix_into(&mut first);
        assert_eq!(first, [1, 10]);

        mixer.play(SampleId(7), sample, false);
        let mut restarted = [0; 4];
        mixer.mix_into(&mut restarted);
        assert_eq!(restarted, [1, 10, 2, 20]);
    }

    #[test]
    fn loop_repeats_without_a_silent_frame() {
        let mut mixer = AudioMixer::default();
        mixer.play(SampleId(3), pcm(&[(1, 10), (2, 20)]), true);
        let mut output = [0; 6];
        mixer.mix_into(&mut output);
        assert_eq!(output, [1, 10, 2, 20, 1, 10]);
    }

    #[test]
    fn oldest_voice_is_stolen_after_all_48_slots_are_used() {
        let mut mixer = AudioMixer::default();
        for id in 0..MAX_AUDIO_VOICES as u64 {
            let value = (id + 1) as i16;
            assert!(mixer.play(SampleId(id), pcm(&[(value, value)]), false));
        }
        assert_eq!(mixer.active_voice_count(), MAX_AUDIO_VOICES);
        mixer.play(
            SampleId(MAX_AUDIO_VOICES as u64),
            pcm(&[((MAX_AUDIO_VOICES + 1) as i16, (MAX_AUDIO_VOICES + 1) as i16)]),
            false,
        );
        let mut output = [0; 2];
        mixer.mix_into(&mut output);
        assert_eq!(output, [1_224, 1_224]);
    }

    #[test]
    fn zero_length_samples_do_not_allocate_a_voice() {
        let mut mixer = AudioMixer::default();
        assert!(!mixer.play(SampleId(1), Arc::from(Vec::<i16>::new()), false));
        assert_eq!(mixer.active_voice_count(), 0);
    }
}
