//! Opus audio encoding and PCM frame buffering.
//!
//! WebRTC requires 48kHz stereo Opus audio with 20ms frame duration
//! (960 samples per channel = 1920 samples for stereo). This module
//! accumulates incoming PCM samples, slices them into 20ms chunks,
//! encodes them with libopus, and prepares `AudioEncodedFrame`s.

use std::sync::Arc;
use std::time::Instant;

/// Sink callback for encoded audio frames.
pub type AudioFrameSink = Arc<dyn Fn(AudioEncodedFrame) + Send + Sync + 'static>;

/// Duration in samples per channel for a standard 20ms WebRTC Opus frame at 48kHz.
pub const OPUS_FRAME_SAMPLES_PER_CHANNEL: usize = 960;
/// Channel count for stereo audio.
pub const OPUS_CHANNELS: usize = 2;
/// Total number of interleaved samples in a 20ms stereo frame (960 * 2 = 1920).
pub const OPUS_FRAME_SAMPLE_COUNT: usize = OPUS_FRAME_SAMPLES_PER_CHANNEL * OPUS_CHANNELS;
/// Sample rate expected by Opus and WebRTC audio.
pub const OPUS_SAMPLE_RATE: u32 = 48000;
/// Maximum buffered samples (100ms = 5 frames of 1920 samples).
/// Dropping older samples when exceeding this limit prevents latency accumulation.
pub const MAX_ACCUMULATOR_SAMPLES: usize = OPUS_FRAME_SAMPLE_COUNT * 5;

/// An encoded Opus audio frame ready for WebRTC RTP packetization.
#[derive(Debug, Clone)]
pub struct AudioEncodedFrame {
    /// Opus-encoded packet bytes.
    pub data: Vec<u8>,
    /// Number of audio samples per channel represented by this packet (typically 960).
    pub timestamp_samples: u32,
    /// Timestamp when this audio frame was captured.
    pub captured_at: Instant,
}

/// Errors that can occur during audio encoding.
#[derive(Debug, thiserror::Error)]
pub enum AudioEncoderError {
    #[error("Opus encoder initialization failed: {0}")]
    InitFailed(String),
    #[error("Opus encode failed: {0}")]
    EncodeFailed(String),
}

/// Opus encoder and PCM sample buffer.
pub struct AudioEncoder {
    encoder: opus::Encoder,
    /// Accumulator for interleaved 48kHz stereo f32 PCM samples: [L0, R0, L1, R1, ...].
    pcm_accumulator: Vec<f32>,
    /// Temporary scratch buffer for encoded Opus output.
    output_buffer: Vec<u8>,
}

impl AudioEncoder {
    /// Create a new AudioEncoder initialized for 48kHz stereo audio.
    pub fn new() -> Result<Self, AudioEncoderError> {
        let mut encoder = opus::Encoder::new(
            OPUS_SAMPLE_RATE,
            opus::Channels::Stereo,
            opus::Application::Audio,
        )
        .map_err(|e| AudioEncoderError::InitFailed(e.to_string()))?;

        // 96kbps provides pristine audio quality for desktop/media streaming over LAN
        let _ = encoder.set_bitrate(opus::Bitrate::Bits(96_000));

        Ok(Self {
            encoder,
            pcm_accumulator: Vec::with_capacity(OPUS_FRAME_SAMPLE_COUNT * 4),
            output_buffer: vec![0u8; 1275], // RFC 6716 max recommended Opus packet size
        })
    }

    /// Push interleaved 48kHz stereo f32 samples and encode any full 20ms frames.
    pub fn push_pcm_and_encode<F>(
        &mut self,
        samples: &[f32],
        mut on_frame: F,
    ) -> Result<(), AudioEncoderError>
    where
        F: FnMut(AudioEncodedFrame),
    {
        self.pcm_accumulator.extend_from_slice(samples);

        // Keep interactive latency under control: if accumulator has backed up beyond 100ms,
        // drain excess stale samples from the front.
        if self.pcm_accumulator.len() > MAX_ACCUMULATOR_SAMPLES {
            let excess = self.pcm_accumulator.len() - MAX_ACCUMULATOR_SAMPLES;
            // Round excess to full stereo samples (multiple of 2)
            let excess = (excess / OPUS_CHANNELS) * OPUS_CHANNELS;
            self.pcm_accumulator.drain(..excess);
            tracing::debug!(
                "Audio accumulator pruned {} stale samples to protect interactive latency",
                excess
            );
        }

        // Process all complete 20ms frames (1920 samples)
        while self.pcm_accumulator.len() >= OPUS_FRAME_SAMPLE_COUNT {
            let frame_slice = &self.pcm_accumulator[..OPUS_FRAME_SAMPLE_COUNT];
            let encoded_len = self
                .encoder
                .encode_float(frame_slice, &mut self.output_buffer)
                .map_err(|e| AudioEncoderError::EncodeFailed(e.to_string()))?;

            let frame = AudioEncodedFrame {
                data: self.output_buffer[..encoded_len].to_vec(),
                timestamp_samples: OPUS_FRAME_SAMPLES_PER_CHANNEL as u32,
                captured_at: Instant::now(),
            };

            on_frame(frame);
            self.pcm_accumulator.drain(..OPUS_FRAME_SAMPLE_COUNT);
        }

        Ok(())
    }

    /// Reset internal sample accumulator and encoder state.
    pub fn reset(&mut self) {
        self.pcm_accumulator.clear();
        let _ = self.encoder.reset_state();
    }
}

/// Convert arbitrary sample-rate / channel PCM to 48kHz stereo interleaved f32.
pub fn resample_and_interleave(left: &[f32], right: Option<&[f32]>, source_rate: u32) -> Vec<f32> {
    if left.is_empty() {
        return Vec::new();
    }

    let source_frames = left.len();
    let has_right = right.is_some() && right.unwrap().len() >= source_frames;
    let right_channel = right.unwrap_or(left);

    if source_rate == OPUS_SAMPLE_RATE {
        // Direct interleave without resampling
        let mut interleaved = Vec::with_capacity(source_frames * OPUS_CHANNELS);
        for i in 0..source_frames {
            interleaved.push(left[i]);
            interleaved.push(if has_right { right_channel[i] } else { left[i] });
        }
        return interleaved;
    }

    // Linear resampling to 48kHz
    let target_frames =
        ((source_frames as u64 * OPUS_SAMPLE_RATE as u64) / source_rate.max(1) as u64) as usize;
    let mut out = Vec::with_capacity(target_frames * OPUS_CHANNELS);

    let ratio = source_rate as f64 / OPUS_SAMPLE_RATE as f64;
    for out_idx in 0..target_frames {
        let src_pos = out_idx as f64 * ratio;
        let idx0 = src_pos.floor() as usize;
        let frac = (src_pos - idx0 as f64) as f32;
        let idx1 = (idx0 + 1).min(source_frames - 1);

        let l0 = left[idx0.min(source_frames - 1)];
        let l1 = left[idx1];
        let l = l0 + (l1 - l0) * frac;

        let r0 = if has_right {
            right_channel[idx0.min(source_frames - 1)]
        } else {
            l0
        };
        let r1 = if has_right { right_channel[idx1] } else { l1 };
        let r = r0 + (r1 - r0) * frac;

        out.push(l);
        out.push(r);
    }

    out
}

/// Generate a synthetic 48kHz stereo sine wave frame for testing and headless verification.
pub fn generate_synthetic_audio_frame(frequency_hz: f32, phase: &mut f32) -> Vec<f32> {
    let mut samples = Vec::with_capacity(OPUS_FRAME_SAMPLE_COUNT);
    let phase_increment = 2.0 * std::f32::consts::PI * frequency_hz / (OPUS_SAMPLE_RATE as f32);

    for _ in 0..OPUS_FRAME_SAMPLES_PER_CHANNEL {
        let sample = (phase.sin()) * 0.25; // -12dB level to avoid clipping / loudness
        samples.push(sample);
        samples.push(sample);
        *phase = (*phase + phase_increment) % (2.0 * std::f32::consts::PI);
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_encoder_initializes_and_encodes_frame() {
        let mut encoder = AudioEncoder::new().expect("AudioEncoder should initialize");
        let mut frames = Vec::new();
        let mut phase = 0.0;
        let pcm = generate_synthetic_audio_frame(440.0, &mut phase);

        assert_eq!(pcm.len(), OPUS_FRAME_SAMPLE_COUNT);
        encoder
            .push_pcm_and_encode(&pcm, |frame| frames.push(frame))
            .expect("encode succeeds");

        assert_eq!(frames.len(), 1, "exactly one 20ms frame should be emitted");
        assert_eq!(frames[0].timestamp_samples, 960);
        assert!(!frames[0].data.is_empty(), "Opus packet must not be empty");
        assert!(
            frames[0].data.len() <= 1275,
            "Opus packet size within bounds"
        );
    }

    #[test]
    fn sample_accumulator_chunks_variable_inputs() {
        let mut encoder = AudioEncoder::new().expect("AudioEncoder should initialize");
        let mut frames = Vec::new();

        // Feed small 500-sample chunks (250 stereo samples)
        let chunk = vec![0.0f32; 500];
        for _ in 0..10 {
            encoder
                .push_pcm_and_encode(&chunk, |frame| frames.push(frame))
                .expect("encode succeeds");
        }

        // 10 * 500 = 5000 samples. 5000 / 1920 = 2 full frames (3840 samples), with 1160 remaining.
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn accumulator_drops_excess_when_stale() {
        let mut encoder = AudioEncoder::new().expect("AudioEncoder should initialize");
        let huge_chunk = vec![0.0f32; MAX_ACCUMULATOR_SAMPLES * 2];
        let mut frames = Vec::new();

        encoder
            .push_pcm_and_encode(&huge_chunk, |frame| frames.push(frame))
            .expect("encode succeeds");

        // Should produce frames and not exceed MAX_ACCUMULATOR_SAMPLES in storage
        assert!(!frames.is_empty());
        assert!(encoder.pcm_accumulator.len() < OPUS_FRAME_SAMPLE_COUNT);
    }

    #[test]
    fn resample_and_interleave_handles_44100_to_48000() {
        let left_44100 = vec![0.5f32; 4410]; // 100ms at 44100
        let right_44100 = vec![-0.5f32; 4410];

        let out = resample_and_interleave(&left_44100, Some(&right_44100), 44100);
        // 100ms at 48000 should be ~4800 stereo frames = ~9600 samples
        assert_eq!(out.len(), 4800 * 2);
        assert!((out[0] - 0.5).abs() < 1e-4);
        assert!((out[1] - (-0.5)).abs() < 1e-4);
    }

    #[test]
    fn resample_and_interleave_mono_duplicates_to_stereo() {
        let mono = vec![0.8f32; 960];
        let out = resample_and_interleave(&mono, None, 48000);
        assert_eq!(out.len(), 960 * 2);
        for i in (0..out.len()).step_by(2) {
            assert_eq!(out[i], 0.8f32);
            assert_eq!(out[i + 1], 0.8f32);
        }
    }
}
