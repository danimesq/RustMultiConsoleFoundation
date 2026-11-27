//! Canonical WAV decoder and integer PCM mixer shared by native SDL and browser AudioWorklet.
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


/// Decode an integer/float RIFF-WAVE into the shared canonical 44.1 kHz S16
/// stereo format. Native SDL and the browser AudioWorklet consume these exact
/// samples, so their integer mixing stage is bit-for-bit equivalent.
pub fn decode_wav_pcm(bytes: &[u8], volume_percent: i32) -> Result<Vec<i16>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("expected a little-endian RIFF/WAVE file".to_owned());
    }
    let riff_size = read_u32(bytes, 4)? as usize;
    let riff_end = riff_size.checked_add(8)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| "RIFF length exceeds the available WAV data".to_owned())?;

    let mut format: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut offset = 12_usize;
    while offset.checked_add(8).is_some_and(|end| end <= riff_end) {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_len = read_u32(bytes, offset + 4)? as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start.checked_add(chunk_len)
            .filter(|end| *end <= riff_end)
            .ok_or_else(|| "WAV chunk length exceeds the RIFF container".to_owned())?;
        let chunk = &bytes[chunk_start..chunk_end];

        if chunk_id == b"fmt " {
            if chunk.len() < 16 {
                return Err("WAV fmt chunk is shorter than 16 bytes".to_owned());
            }
            let mut format_tag = read_u16(chunk, 0)?;
            let channels = read_u16(chunk, 2)?;
            let sample_rate = read_u32(chunk, 4)?;
            let block_align = read_u16(chunk, 12)?;
            let bits_per_sample = read_u16(chunk, 14)?;
            if format_tag == 0xfffe {
                if chunk.len() < 40 {
                    return Err("WAVE_FORMAT_EXTENSIBLE fmt chunk is incomplete".to_owned());
                }
                let valid_bits = read_u16(chunk, 18)?;
                if valid_bits != 0 && valid_bits != bits_per_sample {
                    return Err("WAVE_FORMAT_EXTENSIBLE valid bits must match the sample container".to_owned());
                }
                // The first WORD of the SubFormat GUID is the PCM/IEEE-float tag.
                format_tag = read_u16(chunk, 24)?;
            }
            format = Some((format_tag, channels, sample_rate, block_align, bits_per_sample));
        } else if chunk_id == b"data" && data.is_none() {
            data = Some(chunk);
        }

        offset = chunk_end.saturating_add(chunk_len & 1);
    }

    let (format_tag, channels, sample_rate, block_align, bits_per_sample) =
        format.ok_or_else(|| "WAV fmt chunk is missing".to_owned())?;
    let data = data.ok_or_else(|| "WAV data chunk is missing".to_owned())?;
    if channels == 0 || sample_rate == 0 {
        return Err("WAV channel count and sample rate must be non-zero".to_owned());
    }
    if !matches!(format_tag, 1 | 3) {
        return Err(format!("unsupported WAV encoding tag {format_tag} (expected PCM or IEEE float)"));
    }
    if format_tag == 1 && !matches!(bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(format!("unsupported PCM sample width: {bits_per_sample} bits"));
    }
    if format_tag == 3 && bits_per_sample != 32 {
        return Err(format!("unsupported IEEE-float sample width: {bits_per_sample} bits"));
    }

    let bytes_per_sample = usize::from(bits_per_sample).div_ceil(8);
    let minimum_align = usize::from(channels).checked_mul(bytes_per_sample)
        .ok_or_else(|| "WAV block alignment overflow".to_owned())?;
    let align = usize::from(block_align);
    if align < minimum_align || align == 0 {
        return Err("WAV block alignment is smaller than its channel data".to_owned());
    }

    let source_frames = data.len() / align;
    if source_frames == 0 {
        return Err("WAV contains no complete audio frames".to_owned());
    }
    let source_frames_u64 = u64::try_from(source_frames)
        .map_err(|_| "WAV frame count exceeds the supported range".to_owned())?;
    let output_frames = source_frames_u64
        .checked_mul(u64::from(MIXER_RATE_HZ))
        .ok_or_else(|| "WAV resampled frame count overflow".to_owned())?
        / u64::from(sample_rate);
    let output_frames = usize::try_from(output_frames.max(1))
        .map_err(|_| "WAV resampled frame count exceeds the supported range".to_owned())?;

    let mut source = Vec::with_capacity(source_frames.saturating_mul(MIXER_CHANNELS));
    for frame in 0..source_frames {
        let base = frame * align;
        let left = decode_wav_sample(data, base, format_tag, bits_per_sample)?;
        let right = if channels == 1 {
            left
        } else {
            decode_wav_sample(data, base + bytes_per_sample, format_tag, bits_per_sample)?
        };
        source.extend_from_slice(&[left, right]);
    }

    let mut output = Vec::with_capacity(output_frames.saturating_mul(MIXER_CHANNELS));
    for frame in 0..output_frames {
        if sample_rate == MIXER_RATE_HZ {
            let source_offset = frame.min(source_frames - 1) * MIXER_CHANNELS;
            output.extend_from_slice(&source[source_offset..source_offset + MIXER_CHANNELS]);
            continue;
        }

        // Rational linear resampling avoids floating-point rounding differences
        // between SDL and WebAudio for the same source WAV.
        let numerator = (frame as u64)
            .checked_mul(u64::from(sample_rate))
            .ok_or_else(|| "WAV resampling position overflow".to_owned())?;
        let source_frame = (numerator / u64::from(MIXER_RATE_HZ)) as usize;
        let remainder = (numerator % u64::from(MIXER_RATE_HZ)) as i64;
        let next_frame = (source_frame + 1).min(source_frames - 1);
        let first = source_frame.min(source_frames - 1) * MIXER_CHANNELS;
        let second = next_frame * MIXER_CHANNELS;
        for channel in 0..MIXER_CHANNELS {
            let a = i64::from(source[first + channel]);
            let b = i64::from(source[second + channel]);
            let weighted = a * (i64::from(MIXER_RATE_HZ) - remainder) + b * remainder;
            output.push((weighted / i64::from(MIXER_RATE_HZ)) as i16);
        }
    }

    let volume = volume_percent.clamp(0, 400);
    if volume != 100 {
        for sample in &mut output {
            let scaled = (i32::from(*sample) * volume) / 100;
            *sample = scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
    }
    Ok(output)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes.get(offset..offset + 2)
        .ok_or_else(|| "truncated WAV integer field".to_owned())?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes.get(offset..offset + 4)
        .ok_or_else(|| "truncated WAV integer field".to_owned())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn decode_wav_sample(
    data: &[u8],
    offset: usize,
    format_tag: u16,
    bits_per_sample: u16,
) -> Result<i16, String> {
    let value = match (format_tag, bits_per_sample) {
        (1, 8) => i32::from(*data.get(offset).ok_or_else(|| "truncated 8-bit PCM sample".to_owned())?) - 128,
        (1, 16) => i32::from(i16::from_le_bytes([
            *data.get(offset).ok_or_else(|| "truncated 16-bit PCM sample".to_owned())?,
            *data.get(offset + 1).ok_or_else(|| "truncated 16-bit PCM sample".to_owned())?,
        ])),
        (1, 24) => {
            let a = i32::from(*data.get(offset).ok_or_else(|| "truncated 24-bit PCM sample".to_owned())?);
            let b = i32::from(*data.get(offset + 1).ok_or_else(|| "truncated 24-bit PCM sample".to_owned())?);
            let c = i32::from(*data.get(offset + 2).ok_or_else(|| "truncated 24-bit PCM sample".to_owned())?);
            let raw = a | (b << 8) | (c << 16);
            let signed = if raw & 0x0080_0000 != 0 {
                raw | !0x00ff_ffff
            } else {
                raw
            };
            signed >> 8
        }
        (1, 32) => {
            let sample = read_i32(data, offset)?;
            sample >> 16
        }
        (3, 32) => {
            let sample = f32::from_le_bytes([
                *data.get(offset).ok_or_else(|| "truncated float WAV sample".to_owned())?,
                *data.get(offset + 1).ok_or_else(|| "truncated float WAV sample".to_owned())?,
                *data.get(offset + 2).ok_or_else(|| "truncated float WAV sample".to_owned())?,
                *data.get(offset + 3).ok_or_else(|| "truncated float WAV sample".to_owned())?,
            ]);
            if !sample.is_finite() { 0 }
            else if sample <= -1.0 { i32::from(i16::MIN) }
            else if sample >= 1.0 { i32::from(i16::MAX) }
            else { (sample * 32768.0).round() as i32 }
        }
        _ => return Err("unsupported WAV sample encoding".to_owned()),
    };
    let value = if format_tag == 1 && bits_per_sample == 8 {
        value << 8
    } else if format_tag == 1 && bits_per_sample == 24 {
        value
    } else if format_tag == 1 && bits_per_sample == 32 {
        value
    } else {
        value
    };
    Ok(value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, String> {
    let slice = bytes.get(offset..offset + 4)
        .ok_or_else(|| "truncated 32-bit PCM sample".to_owned())?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}


/// Convert browser-decoded channel data into the same canonical S16 stereo stream.
/// This is used only for formats that the Rust RIFF/WAVE decoder does not accept
/// (for example OGG/MP3/AAC); WAV remains on the exact shared RIFF path above.
pub fn canonicalize_float_stereo(
    left: &[f32],
    right: &[f32],
    source_rate_hz: u32,
    volume_percent: i32,
) -> Result<Vec<i16>, String> {
    if source_rate_hz == 0 {
        return Err("decoded audio sample rate must be non-zero".to_owned());
    }
    let source_frames = left.len().min(right.len());
    if source_frames == 0 {
        return Err("decoded audio contains no sample frames".to_owned());
    }
    let mut source = Vec::with_capacity(source_frames.saturating_mul(MIXER_CHANNELS));
    for frame in 0..source_frames {
        source.push(float_to_s16(left[frame]));
        source.push(float_to_s16(right[frame]));
    }
    let frames_u64 = u64::try_from(source_frames)
        .map_err(|_| "decoded audio frame count exceeds the supported range".to_owned())?;
    let output_frames = frames_u64
        .checked_mul(u64::from(MIXER_RATE_HZ))
        .ok_or_else(|| "decoded audio resampled frame count overflow".to_owned())?
        / u64::from(source_rate_hz);
    let output_frames = usize::try_from(output_frames.max(1))
        .map_err(|_| "decoded audio resampled frame count exceeds the supported range".to_owned())?;

    let mut output = Vec::with_capacity(output_frames.saturating_mul(MIXER_CHANNELS));
    for frame in 0..output_frames {
        if source_rate_hz == MIXER_RATE_HZ {
            let offset = frame.min(source_frames - 1) * MIXER_CHANNELS;
            output.extend_from_slice(&source[offset..offset + MIXER_CHANNELS]);
            continue;
        }
        let numerator = (frame as u64)
            .checked_mul(u64::from(source_rate_hz))
            .ok_or_else(|| "decoded audio resampling position overflow".to_owned())?;
        let first_frame = (numerator / u64::from(MIXER_RATE_HZ)) as usize;
        let remainder = (numerator % u64::from(MIXER_RATE_HZ)) as i64;
        let next_frame = (first_frame + 1).min(source_frames - 1);
        let first = first_frame.min(source_frames - 1) * MIXER_CHANNELS;
        let next = next_frame * MIXER_CHANNELS;
        for channel in 0..MIXER_CHANNELS {
            let a = i64::from(source[first + channel]);
            let b = i64::from(source[next + channel]);
            let weighted = a * (i64::from(MIXER_RATE_HZ) - remainder) + b * remainder;
            output.push((weighted / i64::from(MIXER_RATE_HZ)) as i16);
        }
    }

    let volume = volume_percent.clamp(0, 400);
    if volume != 100 {
        for sample in &mut output {
            let scaled = (i32::from(*sample) * volume) / 100;
            *sample = scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
    }
    Ok(output)
}

fn float_to_s16(sample: f32) -> i16 {
    let value = if !sample.is_finite() {
        0
    } else if sample <= -1.0 {
        i32::from(i16::MIN)
    } else if sample >= 1.0 {
        i32::from(i16::MAX)
    } else {
        (sample * 32768.0).round() as i32
    };
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// Produce the same diagnostic tone PCM on native SDL and the browser worklet.
pub fn generate_beep_pcm(frequency_hz: u32, duration_ms: u32) -> Vec<i16> {
    let frequency = f64::from(frequency_hz.clamp(40, 12_000));
    let frames = ((u64::from(MIXER_RATE_HZ) * u64::from(duration_ms.min(5_000)) / 1_000)
        .max(1)) as usize;
    let mut pcm = Vec::with_capacity(frames * MIXER_CHANNELS);
    for frame in 0..frames {
        let phase = std::f64::consts::TAU * frequency * frame as f64 / f64::from(MIXER_RATE_HZ);
        let sample = (phase.sin() * 7_000.0) as i16;
        pcm.extend_from_slice(&[sample, sample]);
    }
    pcm
}


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


    fn make_pcm16_wav(sample_rate: u32, channels: u16, frames: &[[i16; 2]]) -> Vec<u8> {
        let mut data = Vec::new();
        for frame in frames {
            for channel in 0..usize::from(channels) {
                data.extend_from_slice(&frame[channel].to_le_bytes());
            }
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        wav
    }

    #[test]
    fn wav_decoder_preserves_44100_hz_stereo_pcm_and_applies_pascal_volume() {
        let wav = make_pcm16_wav(44_100, 2, &[[1001, -1001], [2000, -2000]]);
        assert_eq!(decode_wav_pcm(&wav, 100).unwrap(), vec![1001, -1001, 2000, -2000]);
        assert_eq!(decode_wav_pcm(&wav, 50).unwrap(), vec![500, -500, 1000, -1000]);
    }

    #[test]
    fn wav_decoder_duplicates_mono_and_resamples_with_integer_rational_interpolation() {
        let wav = make_pcm16_wav(22_050, 1, &[[1000, 0], [3000, 0]]);
        assert_eq!(
            decode_wav_pcm(&wav, 100).unwrap(),
            vec![1000, 1000, 2000, 2000, 3000, 3000, 3000, 3000]
        );
    }

    #[test]
    fn decoder_converts_unsigned_eight_bit_pcm_to_signed_sixteen_bit() {
        assert_eq!(decode_wav_sample(&[0], 0, 1, 8).unwrap(), i16::MIN);
        assert_eq!(decode_wav_sample(&[128], 0, 1, 8).unwrap(), 0);
        assert_eq!(decode_wav_sample(&[255], 0, 1, 8).unwrap(), 32_512);
    }

    #[test]
    fn decoder_sign_extends_twenty_four_bit_pcm() {
        assert_eq!(decode_wav_sample(&[0xff, 0xff, 0x7f], 0, 1, 24).unwrap(), i16::MAX);
        assert_eq!(decode_wav_sample(&[0x00, 0x00, 0x80], 0, 1, 24).unwrap(), i16::MIN);
        assert_eq!(decode_wav_sample(&[0x00, 0x00, 0x01], 0, 1, 24).unwrap(), 256);
    }

    #[test]
    fn decoder_clamps_ieee_float_pcm_and_rounds_to_signed_sixteen_bit() {
        assert_eq!(decode_wav_sample(&(-1.0_f32).to_le_bytes(), 0, 3, 32).unwrap(), i16::MIN);
        assert_eq!(decode_wav_sample(&(0.5_f32).to_le_bytes(), 0, 3, 32).unwrap(), 16_384);
        assert_eq!(decode_wav_sample(&(1.0_f32).to_le_bytes(), 0, 3, 32).unwrap(), i16::MAX);
        assert_eq!(decode_wav_sample(&(f32::NAN).to_le_bytes(), 0, 3, 32).unwrap(), 0);
    }

    #[test]
    fn wav_decoder_rejects_truncated_or_non_wav_data() {
        assert!(decode_wav_pcm(b"not a wave file", 100).is_err());
        let mut wav = make_pcm16_wav(44_100, 2, &[[1, 2]]);
        wav.truncate(30);
        assert!(decode_wav_pcm(&wav, 100).is_err());
    }

    #[test]
    fn browser_decoded_float_stereo_is_quantized_and_resampled_to_canonical_pcm() {
        let left = [0.0_f32, 0.25, 0.5];
        let right = [0.0_f32, -0.25, -0.5];
        assert_eq!(
            canonicalize_float_stereo(&left, &right, 44_100, 100).unwrap(),
            vec![0, 0, 8192, -8192, 16_384, -16_384]
        );
        assert_eq!(
            canonicalize_float_stereo(&left, &right, 22_050, 50).unwrap(),
            vec![0, 0, 2048, -2048, 4096, -4096, 6144, -6144, 8192, -8192, 8192, -8192]
        );
        assert!(canonicalize_float_stereo(&left, &right, 0, 100).is_err());
    }

    #[test]
    fn shared_beep_generator_is_stereo_and_uses_the_native_sample_count() {
        let pcm = generate_beep_pcm(440, 10);
        assert_eq!(pcm.len(), 441 * 2);
        assert!(pcm.chunks_exact(2).all(|frame| frame[0] == frame[1]));
        assert_eq!(generate_beep_pcm(0, 0).len(), 2);
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
