//! ScreenCaptureKit Audio Buffer Processing and PCM Extraction.
//!
//! Extracts audio PCM samples from macOS `CMSampleBuffer`, normalizes
//! sample format (float32 / int16, non-interleaved / interleaved) and
//! sample rate, and forwards to the `AudioEncoder`.

#[cfg(all(target_os = "macos", feature = "screenkit"))]
use super::audio_encoder::{resample_and_interleave, AudioEncoder, AudioEncoderError};

#[cfg(all(target_os = "macos", feature = "screenkit"))]
pub fn extract_pcm_from_sample_buffer(
    sample: &screencapturekit::cm::CMSampleBuffer,
) -> Option<Vec<f32>> {
    use screencapturekit::cm::CMSampleBufferExt;

    let buffer_list = sample.audio_buffer_list()?;
    let num_buffers = buffer_list.num_buffers();
    if num_buffers == 0 {
        return None;
    }

    let format_desc = sample.format_description();
    let is_float = format_desc
        .as_ref()
        .map(|f| f.audio_is_float())
        .unwrap_or(true);
    let sample_rate = format_desc
        .as_ref()
        .and_then(|f| f.audio_sample_rate())
        .unwrap_or(48000.0) as u32;

    if num_buffers >= 2 {
        // Non-interleaved stereo: buffer 0 is Left, buffer 1 is Right
        let b0 = buffer_list.get(0)?;
        let b1 = buffer_list.get(1)?;

        let (left_f32, right_f32) = if is_float {
            let left = parse_f32_slice(b0.data());
            let right = parse_f32_slice(b1.data());
            (left, right)
        } else {
            let left = parse_i16_to_f32_vec(b0.data());
            let right = parse_i16_to_f32_vec(b1.data());
            (left, right)
        };

        Some(resample_and_interleave(
            &left_f32,
            Some(&right_f32),
            sample_rate,
        ))
    } else {
        // Single buffer: either interleaved stereo or mono
        let b0 = buffer_list.get(0)?;
        let channels = b0.number_channels.max(1);

        if is_float {
            let samples = parse_f32_slice(b0.data());
            if channels >= 2 {
                // Interleaved stereo
                if sample_rate == 48000 {
                    Some(samples)
                } else {
                    let (l, r) = deinterleave(&samples);
                    Some(resample_and_interleave(&l, Some(&r), sample_rate))
                }
            } else {
                // Mono
                Some(resample_and_interleave(&samples, None, sample_rate))
            }
        } else {
            let samples = parse_i16_to_f32_vec(b0.data());
            if channels >= 2 {
                if sample_rate == 48000 {
                    Some(samples)
                } else {
                    let (l, r) = deinterleave(&samples);
                    Some(resample_and_interleave(&l, Some(&r), sample_rate))
                }
            } else {
                Some(resample_and_interleave(&samples, None, sample_rate))
            }
        }
    }
}

/// Helper to parse a byte slice into an f32 Vec safely.
pub fn parse_f32_slice(bytes: &[u8]) -> Vec<f32> {
    let sample_count = bytes.len() / 4;
    let mut out = Vec::with_capacity(sample_count);
    for chunk in bytes.chunks_exact(4) {
        let val = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        out.push(val);
    }
    out
}

/// Helper to convert signed 16-bit integer PCM bytes to normalized f32 Vec.
pub fn parse_i16_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    let sample_count = bytes.len() / 2;
    let mut out = Vec::with_capacity(sample_count);
    for chunk in bytes.chunks_exact(2) {
        let val = i16::from_ne_bytes([chunk[0], chunk[1]]);
        out.push((val as f32) / 32768.0);
    }
    out
}

/// Helper to deinterleave stereo samples into separate (left, right) vectors.
pub fn deinterleave(interleaved: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let frame_count = interleaved.len() / 2;
    let mut left = Vec::with_capacity(frame_count);
    let mut right = Vec::with_capacity(frame_count);
    for chunk in interleaved.chunks_exact(2) {
        left.push(chunk[0]);
        right.push(chunk[1]);
    }
    (left, right)
}

#[cfg(all(target_os = "macos", feature = "screenkit"))]
pub fn process_audio_sample<F>(
    sample: &screencapturekit::cm::CMSampleBuffer,
    encoder: &mut AudioEncoder,
    on_frame: F,
) -> Result<(), AudioEncoderError>
where
    F: FnMut(super::audio_encoder::AudioEncodedFrame),
{
    if let Some(pcm) = extract_pcm_from_sample_buffer(sample) {
        if !pcm.is_empty() {
            encoder.push_pcm_and_encode(&pcm, on_frame)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_f32_slice_round_trips() {
        let original = vec![0.0f32, 0.5f32, -0.5f32, 1.0f32];
        let mut bytes = Vec::new();
        for &val in &original {
            bytes.extend_from_slice(&val.to_ne_bytes());
        }

        let parsed = parse_f32_slice(&bytes);
        assert_eq!(parsed, original);
    }

    #[test]
    fn parse_i16_to_f32_scales_correctly() {
        let samples: Vec<i16> = vec![0, 16384, -16384, 32767, -32768];
        let mut bytes = Vec::new();
        for &val in &samples {
            bytes.extend_from_slice(&val.to_ne_bytes());
        }

        let parsed = parse_i16_to_f32_vec(&bytes);
        assert_eq!(parsed.len(), 5);
        assert!((parsed[0] - 0.0).abs() < 1e-4);
        assert!((parsed[1] - 0.5).abs() < 1e-3);
        assert!((parsed[2] - (-0.5)).abs() < 1e-3);
        assert!((parsed[3] - 1.0).abs() < 1e-3);
        assert!((parsed[4] - (-1.0)).abs() < 1e-3);
    }

    #[test]
    fn deinterleave_separates_channels() {
        let interleaved = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let (l, r) = deinterleave(&interleaved);
        assert_eq!(l, vec![1.0, 3.0, 5.0]);
        assert_eq!(r, vec![2.0, 4.0, 6.0]);
    }
}
