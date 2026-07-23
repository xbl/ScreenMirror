//! H.264 helpers and the re-exported `NativeVideoEncoder`.
//!
//! `VideoEncoder` is now a thin alias for the bindgen-driven
//! [`NativeVideoEncoder`](crate::webrtc::video_toolbox_native::NativeVideoEncoder)
//! which wraps a persistent `AVCodecContext` instead of spawning per-frame
//! `ffmpeg` processes. The Annex B / AVCC helpers and their unit tests
//! remain here because `NativeVideoEncoder` (and the rest of the WebRTC
//! pipeline) depends on them.

#[derive(Debug, Clone)]
pub struct H264EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
}

pub fn split_annex_b_nalus(input: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= input.len() {
        let len = if input[i..].starts_with(&[0, 0, 1]) {
            3
        } else if i + 4 <= input.len() && input[i..].starts_with(&[0, 0, 0, 1]) {
            4
        } else {
            i += 1;
            continue;
        };
        starts.push((i, len));
        i += len;
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, &(start, prefix))| {
            let end = starts
                .get(index + 1)
                .map(|(next, _)| *next)
                .unwrap_or(input.len());
            let nal = &input[start + prefix..end];
            (!nal.is_empty()).then_some(nal)
        })
        .collect()
}

pub fn is_keyframe(sample: &[u8]) -> bool {
    split_annex_b_nalus(sample)
        .iter()
        .any(|nal| matches!(nal.first().map(|v| v & 0x1f), Some(5)))
}

pub fn annex_b_to_avcc(sample: &[u8]) -> Result<Vec<u8>, String> {
    let nalus = split_annex_b_nalus(sample);
    if nalus.is_empty() {
        return Err("H.264 sample contains no NAL units".into());
    }
    let size = nalus.iter().map(|n| 4 + n.len()).sum();
    let mut out = Vec::with_capacity(size);
    for nal in nalus {
        let len = u32::try_from(nal.len()).map_err(|_| "NAL unit is too large")?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(nal);
    }
    Ok(out)
}

pub use crate::webrtc::video_toolbox_native::NativeVideoEncoder as VideoEncoder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mixed_annex_b_start_codes() {
        let input = [0, 0, 1, 0x67, 1, 0, 0, 0, 1, 0x65, 2];
        let nalus = split_annex_b_nalus(&input);
        assert_eq!(nalus, vec![&[0x67, 1][..], &[0x65, 2][..]]);
    }

    #[test]
    fn detects_idr_keyframe() {
        assert!(is_keyframe(&[0, 0, 1, 0x65, 1]));
        assert!(!is_keyframe(&[0, 0, 1, 0x41, 1]));
    }

    #[test]
    fn converts_annex_b_to_avcc() {
        let avcc = annex_b_to_avcc(&[0, 0, 1, 0x67, 9, 0, 0, 1, 0x65, 1]).unwrap();
        assert_eq!(avcc, vec![0, 0, 0, 2, 0x67, 9, 0, 0, 0, 2, 0x65, 1]);
    }
}